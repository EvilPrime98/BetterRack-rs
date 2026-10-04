//! Root entity: routing, server lifecycle, startup sequence, shell chrome and
//! the global overlays (toasts, confirm dialog, app loader).

use gpui::{
    AnyView, App, AppContext as _, Context, Entity, FocusHandle, Focusable, InteractiveElement,
    IntoElement, ParentElement, Render, Styled, Subscription, Window, actions, div, prelude::*, px,
};

use crate::api::ApiClient;
use crate::platform::server_process::{self, Launch, ServerProcess};
use crate::platform::{close_guard, logging, server_config, update_check};
use crate::route::{GoBack, History, Navigate, Route, window_title};
use crate::runtime;
use crate::state::Stores;
use crate::ui::app_loader::app_loader;
use crate::ui::components::button::{ButtonVariant, button};
use crate::ui::confirm::{self, ConfirmHost, ConfirmOptions};
use crate::ui::modals::{self, ModalHost};
use crate::ui::pages::downloads::DownloadsPage;
use crate::ui::pages::library::LibraryPage;
use crate::ui::pages::lists::{ListPage, Source};
use crate::ui::pages::reader::ReaderPage;
use crate::ui::pages::settings::SettingsPage;
use crate::ui::pages::store::StorePage;
use crate::ui::shell::header::header;
use crate::ui::shell::sidebar::Sidebar;
use crate::ui::theme;
use crate::ui::toast::{self, ToastHost};

actions!(betterrack, [Back, Forward, ToggleFullscreen, RequestClose]);

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        gpui::KeyBinding::new("alt-left", Back, None),
        gpui::KeyBinding::new("alt-right", Forward, None),
        // F11 anywhere; macOS also uses ctrl-cmd-f.
        gpui::KeyBinding::new("f11", ToggleFullscreen, None),
        #[cfg(target_os = "macos")]
        gpui::KeyBinding::new("ctrl-cmd-f", ToggleFullscreen, None),
    ]);
}

pub enum ServerStatus {
    Starting,
    Ready,
    Failed(String),
}

/// What the content area shows. Pages arrive phase by phase; the rest render a placeholder.
enum Page {
    Library(Entity<LibraryPage>),
    Reader(Entity<ReaderPage>),
    List(Entity<ListPage>),
    Settings(Entity<SettingsPage>),
    Store(Entity<StorePage>),
    Downloads(Entity<DownloadsPage>),
    Placeholder,
}

pub struct AppRoot {
    history: History,
    pub server_status: ServerStatus,
    /// `Some` once the server is up. Pages clone this and call it through [`runtime::run`].
    pub client: Option<ApiClient>,
    /// Keeps the sidecar alive; dropping it kills the child.
    _server: Option<ServerProcess>,
    stores: Stores,
    sidebar: Entity<Sidebar>,
    toasts: Entity<ToastHost>,
    confirm: Entity<ConfirmHost>,
    modals: Entity<ModalHost>,
    page: Page,
    _page_subs: Vec<Subscription>,
    _subs: Vec<Subscription>,
    /// The startup sequence (§5.4) is still running: show the AppLoader.
    loading: bool,
    /// Dev aid: `BETTERRACK_OPEN` (see [`dev_route`]) opens a page once startup finishes.
    pending_route: Option<Route>,
    dev_identify_opened: Option<bool>,
    focus: FocusHandle,
    /// The close guard has been answered: the next close request goes through.
    close_confirmed: bool,
    close_check_pending: bool,
}

impl AppRoot {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);

        let stores = Stores::new(cx);
        let toasts = toast::init(cx);
        let confirm = confirm::init(cx);
        let modals = modals::init(cx);
        cx.set_global(stores.clone());

        let sidebar = cx.new(|cx| Sidebar::new(stores.clone(), cx));
        let mut subs = vec![
            cx.subscribe_in(&sidebar, window, |this, _, ev: &Navigate, window, cx| {
                this.navigate(ev.0.clone(), window, cx)
            }),
            // The loader message follows the identify-all progress.
            cx.observe(&stores.library, |_, _, cx| cx.notify()),
            cx.observe(&stores.prefs, |_, _, cx| cx.notify()),
        ];

        // Debounced progress writes must reach the server before the process exits.
        let comics = stores.comics.clone();
        subs.push(cx.on_app_quit(move |_, cx| {
            let flush = comics.update(cx, |s, cx| s.flush_pending(cx));
            async move { flush.await }
        }));

        let mut this = Self {
            history: History::default(),
            server_status: ServerStatus::Starting,
            client: None,
            _server: None,
            stores,
            sidebar,
            toasts,
            confirm,
            modals,
            page: Page::Placeholder,
            _page_subs: Vec::new(),
            _subs: subs,
            loading: true,
            pending_route: dev_route(),
            dev_identify_opened: None,
            focus,
            close_confirmed: false,
            close_check_pending: false,
        };
        // Closing cancels running downloads: ask first (`on_window_should_close` is synchronous, so
        // the check runs in the background and the window closes itself once it is answered).
        let root = cx.entity();
        window.on_window_should_close(cx, move |window, cx| {
            root.update(cx, |root, cx| root.guard_close(window, cx))
        });
        window.set_window_title(&window_title(this.history.current()));
        this.build_page(window, cx);
        this.start_server(cx);
        this
    }

    /// `BETTERRACK_SERVER_URL` attaches to a running server (dev); remote mode (§7) attaches to the
    /// saved URL; otherwise run the backend in-process.
    fn start_server(&self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let outcome = runtime::run(async {
                let remote = server_config::current();
                if let Ok(url) = std::env::var("BETTERRACK_SERVER_URL") {
                    attach(url, std::env::var("BR_API_KEY").ok()).await
                } else if remote.needs_setup() {
                    Err("Remote mode is on but no server address is set.".to_string())
                } else if remote.remote {
                    attach(remote.url.clone(), server_config::api_key()).await
                } else if let Some(launch) = Launch::detect() {
                    let server = ServerProcess::spawn(&launch)
                        .await
                        .map_err(|e| e.to_string())?;
                    let client =
                        ApiClient::new(&server.base_url, None).map_err(|e| e.to_string())?;
                    Ok((server, client))
                } else {
                    let client = server_process::start_in_process()?;
                    Ok((
                        ServerProcess::external(client.base_url().to_string()),
                        client,
                    ))
                }
            })
            .await;
            this.update(cx, |this, cx| match outcome {
                Ok::<_, String>((server, client)) => {
                    tracing::info!("server ready at {}", client.base_url());
                    this._server = Some(server);
                    this.client = Some(client.clone());
                    this.server_status = ServerStatus::Ready;
                    this.begin_startup(client, cx);
                    this.check_for_updates(cx);
                    cx.notify();
                }
                Err(msg) => {
                    tracing::error!("server startup failed: {msg}");
                    this.server_status = ServerStatus::Failed(msg);
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// On a packaged start, offer a newer release (`checkForUpdates`). Runs in the background and
    /// never blocks or fails the app.
    fn check_for_updates(&self, cx: &mut Context<Self>) {
        if !server_process::is_packaged() {
            return;
        }
        cx.spawn(async move |_, cx| {
            let update =
                runtime::run(async { update_check::check(env!("CARGO_PKG_VERSION")).await }).await;
            let Some(update) = update else { return };
            cx.update(|cx| {
                let mut options = ConfirmOptions::new(
                    "BetterRack update",
                    format!(
                        "Version {} is available. You are running version {}. Download the new version now?",
                        update.version,
                        env!("CARGO_PKG_VERSION")
                    ),
                )
                .labels("Download", "Later");
                // "Later" + "Don't ask again" means "Skip this version".
                options.dont_ask_again = true;
                confirm::ask(cx, options, move |answer, skip, cx| match answer {
                    Some(true) => {
                        toast::success(cx, "Downloading the update…");
                        cx.spawn(async move |cx| {
                            let result = runtime::run({
                                let update = update.clone();
                                async move { update_check::download(&update).await }
                            })
                            .await;
                            cx.update(|cx| match result {
                                Ok(path) => {
                                    toast::success(cx, "Update downloaded");
                                    update_check::open_download(&path);
                                }
                                Err(e) => {
                                    tracing::warn!("update download failed: {e}");
                                    toast::error(cx, format!("Update download failed: {e}"));
                                }
                            });
                        })
                        .detach();
                    }
                    _ if skip => update_check::skip(&update.version),
                    _ => {}
                });
            });
        })
        .detach();
    }

    /// §5.4 steps 3–6: library, comic cache and settings load in parallel (failures do not abort),
    /// then an optional identify-all, then the loader goes away.
    fn begin_startup(&mut self, client: ApiClient, cx: &mut Context<Self>) {
        self.stores.set_client(client.clone(), cx);
        let stores = self.stores.clone();
        cx.spawn(async move |this, cx| {
            let (library, comics) = cx.update(|cx| {
                (
                    stores.library.update(cx, |s, cx| s.fetch(cx)),
                    stores.comics.update(cx, |s, cx| s.init(cx)),
                )
            });
            let settings = runtime::run(async move { client.settings().await });
            let (_, _, settings) = futures_util::future::join3(library, comics, settings).await;

            let rescan = match settings {
                Ok(s) => {
                    let rescan = s.rescan_on_startup;
                    cx.update(|cx| {
                        stores.settings.update(cx, |st, cx| {
                            st.settings = s;
                            st.loaded = true;
                            cx.notify();
                        })
                    });
                    rescan
                }
                Err(e) => {
                    tracing::warn!("loading settings failed: {e}");
                    false
                }
            };

            let has_library = cx.update(|cx| !stores.library.read(cx).groups.is_empty());
            if rescan && has_library {
                // Long-running: the loader stays up and shows "Identifying library X/Y".
                let identify =
                    cx.update(|cx| stores.library.update(cx, |s, cx| s.identify_library(cx)));
                identify.await;
            }

            this.update(cx, |this, cx| {
                this.loading = false;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Close-guard decision for a close request: `true` lets the window close now. Otherwise a
    /// background check decides, and closes the window itself if it is allowed.
    pub fn guard_close(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.close_confirmed {
            return true;
        }
        let Some(client) = self.client.clone() else {
            return true;
        };
        if self.close_check_pending {
            return false;
        }
        self.close_check_pending = true;
        let handle = window.window_handle();
        cx.spawn(async move |this, cx| {
            let close = close_guard::confirm_close(client, cx).await;
            this.update(cx, |this, _| {
                this.close_check_pending = false;
                this.close_confirmed = close;
            })
            .ok();
            if close {
                cx.update(|cx| {
                    handle
                        .update(cx, |_, window, _| window.remove_window())
                        .ok()
                });
            }
        })
        .detach();
        false
    }

    pub fn navigate(&mut self, route: Route, window: &mut Window, cx: &mut Context<Self>) {
        self.history.push(route);
        self.route_changed(window, cx);
    }

    pub fn go_back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.history.back_or_home();
        self.route_changed(window, cx);
    }

    pub fn go_forward(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.history.forward();
        self.route_changed(window, cx);
    }

    fn route_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.set_window_title(&window_title(self.history.current()));
        self.build_page(window, cx);
        cx.notify();
    }

    /// Instantiate the view for the current route (a fresh page per visit).
    fn build_page(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let route = self.history.current().clone();
        self.sidebar.update(cx, |s, cx| {
            s.current = route.clone();
            cx.notify();
        });
        // Leaving the reader: progress must reach the server before views that read it.
        if matches!(self.page, Page::Reader(_)) {
            self.stores
                .comics
                .update(cx, |s, cx| s.flush_pending(cx).detach());
        }
        self._page_subs.clear();
        self.page = match route {
            Route::Reader { uid } => {
                let page = cx.new(|cx| ReaderPage::new(uid, self.stores.clone(), window, cx));
                self._page_subs.push(cx.subscribe_in(
                    &page,
                    window,
                    |this, _, ev: &Navigate, window, cx| this.navigate(ev.0.clone(), window, cx),
                ));
                self._page_subs.push(cx.subscribe_in(
                    &page,
                    window,
                    |this, _, _: &GoBack, window, cx| this.go_back(window, cx),
                ));
                Page::Reader(page)
            }
            Route::Library { uid, search: None } => {
                let page = cx.new(|cx| LibraryPage::new(uid, self.stores.clone(), cx));
                self._page_subs.push(cx.subscribe_in(
                    &page,
                    window,
                    |this, _, ev: &Navigate, window, cx| this.navigate(ev.0.clone(), window, cx),
                ));
                self._page_subs.push(cx.subscribe_in(
                    &page,
                    window,
                    |this, _, _: &GoBack, window, cx| this.go_back(window, cx),
                ));
                Page::Library(page)
            }
            Route::Library {
                search: Some(query),
                ..
            } => self.list_page(Source::Search { query }, window, cx),
            Route::Recent => self.list_page(Source::Recent, window, cx),
            Route::Reading => self.list_page(Source::Reading, window, cx),
            Route::Filtered { writer } => self.list_page(Source::Filtered { writer }, window, cx),
            Route::Settings => {
                Page::Settings(cx.new(|cx| SettingsPage::new(self.stores.clone(), cx)))
            }
            Route::Store => {
                let page = cx.new(|cx| StorePage::new(self.stores.clone(), cx));
                self._page_subs.push(cx.subscribe_in(
                    &page,
                    window,
                    |this, _, ev: &Navigate, window, cx| this.navigate(ev.0.clone(), window, cx),
                ));
                Page::Store(page)
            }
            Route::StoreDownloads => {
                Page::Downloads(cx.new(|cx| DownloadsPage::new(self.stores.clone(), cx)))
            }
        };
    }

    fn list_page(&mut self, source: Source, window: &mut Window, cx: &mut Context<Self>) -> Page {
        let page = cx.new(|cx| ListPage::new(source, self.stores.clone(), cx));
        self._page_subs.push(cx.subscribe_in(
            &page,
            window,
            |this, _, ev: &Navigate, window, cx| this.navigate(ev.0.clone(), window, cx),
        ));
        self._page_subs.push(
            cx.subscribe_in(&page, window, |this, _, _: &GoBack, window, cx| {
                this.go_back(window, cx)
            }),
        );
        Page::List(page)
    }

    pub fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.stores.prefs.update(cx, |p, cx| {
            p.update(cx, |p| p.sidebar_collapsed = !p.sidebar_collapsed)
        });
    }

    fn placeholder(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let status = match &self.server_status {
            ServerStatus::Starting => "Starting server…".to_string(),
            ServerStatus::Ready => "Not ported yet".to_string(),
            ServerStatus::Failed(msg) => {
                format!(
                    "Server failed to start:\n{msg}\n\nFull logs: {}",
                    logging::log_dir().display()
                )
            }
        };
        let failed = matches!(self.server_status, ServerStatus::Failed(_));
        let remote = server_config::current().remote;
        div()
            .flex()
            .flex_col()
            .flex_1()
            .items_center()
            .justify_center()
            .gap(px(8.0))
            .child(
                div()
                    .text_size(px(20.0))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(theme::text())
                    .child(self.history.current().title()),
            )
            .child(
                div()
                    .max_w(px(640.0))
                    .text_size(px(13.0))
                    .text_color(if failed {
                        theme::error()
                    } else {
                        theme::text_muted()
                    })
                    .child(status),
            )
            // A dead remote server must not lock the user out: offer a way back.
            .when(failed, |s| {
                s.child(
                    div()
                        .flex()
                        .gap(px(8.0))
                        .mt(px(12.0))
                        .child(button(
                            "open-logs",
                            "Open logs",
                            ButtonVariant::Secondary,
                            |_, _, _| {
                                let _ = open::that(logging::log_dir());
                            },
                        ))
                        .when(remote, |s| {
                            s.child(button(
                                "change-server",
                                "Change server",
                                ButtonVariant::Secondary,
                                cx.listener(|_, _, _, cx| modals::open_server(cx, true)),
                            ))
                            .child(button(
                                "use-local",
                                "Use local server",
                                ButtonVariant::Classic,
                                cx.listener(|_, _, _, cx| unlink_server(cx)),
                            ))
                        }),
                )
            })
            .into_any_element()
    }
}

/// Attach to an already-running server (remote mode or `BETTERRACK_SERVER_URL`): nothing is spawned.
async fn attach(
    url: String,
    api_key: Option<String>,
) -> Result<(ServerProcess, ApiClient), String> {
    let server = ServerProcess::external(url);
    let client = ApiClient::new(&server.base_url, api_key).map_err(|e| e.to_string())?;
    client
        .healthz()
        .await
        .map_err(|e| format!("could not reach {}: {e}", server.base_url))?;
    Ok((server, client))
}

/// Start a fresh copy of the app and quit this one. Changing the server re-runs the whole startup
/// sequence.
pub fn relaunch(cx: &mut App) {
    let spawned = std::env::current_exe().and_then(|exe| {
        std::process::Command::new(exe)
            .args(std::env::args_os().skip(1))
            .spawn()
    });
    match spawned {
        Ok(_) => cx.quit(),
        Err(e) => {
            tracing::error!("could not relaunch: {e}");
            toast::error(cx, "Saved. Restart BetterRack to apply the change.");
        }
    }
}

/// "Unlink server": clear the remote settings, return to the local sidecar.
pub fn unlink_server(cx: &mut App) {
    match server_config::clear_remote() {
        Ok(()) => relaunch(cx),
        Err(e) => toast::error(cx, format!("Could not unlink the server: {e}")),
    }
}

impl Focusable for AppRoot {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for AppRoot {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.loading {
            if let Some(route) = self.pending_route.take() {
                self.navigate(route, window, cx);
            }
            // Dev aid: `BETTERRACK_OPEN_IDENTIFY=<uid>:<query>` opens the identify modal and searches.
            if let Ok(spec) = std::env::var("BETTERRACK_OPEN_IDENTIFY") {
                if self.dev_identify_opened.replace(true) != Some(true) {
                    let (uid, query) = spec.split_once(':').unwrap_or((spec.as_str(), ""));
                    modals::open_identify(cx, uid.to_string());
                    modals::dev_prefill_identify(cx, query);
                }
            }
        }
        let layout = self.history.current().uses_layout();
        let sidebar_collapsed = self.stores.prefs.read(cx).prefs.sidebar_collapsed;
        let failed = matches!(self.server_status, ServerStatus::Failed(_));

        let loader_message = match self.stores.library.read(cx).identify_progress {
            Some((done, total)) if total > 0 => format!("Identifying library {done}/{total}"),
            Some(_) => "Identifying library…".to_string(),
            None if matches!(self.server_status, ServerStatus::Starting) => {
                "Starting server…".to_string()
            }
            None => "Loading your library…".to_string(),
        };

        let content = match &self.page {
            Page::Library(page) if !failed => AnyView::from(page.clone()).into_any_element(),
            Page::Reader(page) if !failed => AnyView::from(page.clone()).into_any_element(),
            Page::List(page) if !failed => AnyView::from(page.clone()).into_any_element(),
            Page::Settings(page) if !failed => AnyView::from(page.clone()).into_any_element(),
            Page::Store(page) if !failed => AnyView::from(page.clone()).into_any_element(),
            Page::Downloads(page) if !failed => AnyView::from(page.clone()).into_any_element(),
            _ => self.placeholder(cx),
        };

        div()
            .id("app-root")
            .key_context("AppRoot")
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &Back, window, cx| this.go_back(window, cx)))
            .on_action(cx.listener(|this, _: &Forward, window, cx| this.go_forward(window, cx)))
            .on_action(cx.listener(|_, _: &ToggleFullscreen, window, _| window.toggle_fullscreen()))
            // The Linux close button goes through here so it is guarded like the OS close.
            .on_action(cx.listener(|this, _: &RequestClose, window, cx| {
                if this.guard_close(window, cx) {
                    window.remove_window();
                }
            }))
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .font_family(theme::FONT_SANS)
            .text_size(px(14.0))
            .text_color(theme::text())
            .bg(theme::bg_app())
            .when(layout, |s| s.child(header(window, cx)))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .when(layout && !sidebar_collapsed, |s| {
                        s.child(self.sidebar.clone())
                    })
                    .child(content),
            )
            .child(self.toasts.clone())
            .child(self.modals.clone())
            .child(self.confirm.clone())
            .when(self.loading && !failed, |s| {
                s.child(app_loader(loader_message))
            })
    }
}

/// Dev aid. `BETTERRACK_OPEN_READER=<uid>`, or `BETTERRACK_OPEN=settings|store|downloads|recent|reading|
/// search:<query>|writer:<name>|folder:<uid>`.
fn dev_route() -> Option<Route> {
    if let Ok(uid) = std::env::var("BETTERRACK_OPEN_READER") {
        return Some(Route::Reader { uid });
    }
    let spec = std::env::var("BETTERRACK_OPEN").ok()?;
    Some(match spec.split_once(':') {
        Some(("search", q)) => Route::Library {
            uid: None,
            search: Some(q.to_string()),
        },
        Some(("writer", w)) => Route::Filtered {
            writer: w.to_string(),
        },
        Some(("folder", uid)) => Route::Library {
            uid: Some(uid.to_string()),
            search: None,
        },
        _ => match spec.as_str() {
            "settings" => Route::Settings,
            "store" => Route::Store,
            "downloads" => Route::StoreDownloads,
            "recent" => Route::Recent,
            "reading" => Route::Reading,
            _ => return None,
        },
    })
}
