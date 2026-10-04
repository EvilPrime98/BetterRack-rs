//! Settings page. A local draft of `{apiUrl, downloadDir, wikiSearch,
//! rescanOnStartup}` is saved with `PUT /api/settings`; library folders add/remove through their own
//! endpoints and rescan afterwards. The draft resets whenever the stored settings change.
//!
//! The Server section shows what the app is attached to and offers "Change server"
//! and (in remote mode) "Unlink server". Remote mode has no native folder picker (the paths belong
//! to the remote machine), so "Browse…" is hidden there.

use gpui::{
    AppContext as _, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render,
    SharedString, StatefulInteractiveElement, Styled, Subscription, Window, div,
    prelude::FluentBuilder as _, px, rgb,
};

use crate::model::SettingsUpdate;
use crate::platform::server_config;
use crate::state::Stores;
use crate::ui::components::button::{ButtonVariant, button};
use crate::ui::components::checkbox::checkbox;
use crate::ui::components::items_grid::PAGE_PAD_X;
use crate::ui::components::text_input::{TextInput, TextInputEvent};
use crate::ui::confirm::{self, ConfirmOptions};
use crate::ui::icons::{Icon, icon};
use crate::ui::modals;
use crate::ui::{theme, toast};

const WIKI_SEARCH_TIP: &str = "Comics are always identified from ComicInfo.xml. When enabled, comics without it are looked up on the wiki.";

/// Popup shown while hovering an option.
struct HoverTip(SharedString);

impl Render for HoverTip {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .max_w(px(280.0))
            .px(px(10.0))
            .py(px(8.0))
            .rounded(px(6.0))
            .border_1()
            .border_color(gpui::rgba(0xffffff24))
            .bg(rgb(0x1c1c1c))
            .text_size(px(12.0))
            .text_color(rgb(0xe6e6e6))
            .child(self.0.clone())
    }
}

pub struct SettingsPage {
    stores: Stores,
    api_url: Entity<TextInput>,
    download_dir: Entity<TextInput>,
    folder_path: Entity<TextInput>,
    wiki_search: bool,
    rescan_on_startup: bool,
    settings_error: String,
    folder_error: String,
    /// True while a native folder dialog is open; blocks re-triggering Browse.
    picker_open: bool,
    _subs: Vec<Subscription>,
}

impl SettingsPage {
    pub fn new(stores: Stores, cx: &mut Context<Self>) -> Self {
        let s = stores.settings.read(cx).settings.clone();
        let api_url = cx.new(|cx| {
            TextInput::new("https://example.com/wp-json/wp/v2", cx).with_value(s.api_url.clone())
        });
        let download_dir = cx
            .new(|cx| TextInput::new("/path/to/downloads", cx).with_value(s.download_dir.clone()));
        let folder_path = cx.new(|cx| TextInput::new("Folder path", cx));
        let subs = vec![
            cx.observe(&stores.settings, |this, _, cx| this.reset_draft(cx)),
            // Enter in the folder field adds it.
            cx.subscribe(&folder_path, |this, _, ev: &TextInputEvent, cx| {
                if matches!(ev, TextInputEvent::Submit) {
                    this.add_folder_from_field(cx);
                }
            }),
            cx.subscribe(&api_url, |_, _, _: &TextInputEvent, cx| cx.notify()),
            cx.subscribe(&download_dir, |_, _, _: &TextInputEvent, cx| cx.notify()),
        ];
        Self {
            stores,
            api_url,
            download_dir,
            folder_path,
            wiki_search: s.wiki_search,
            rescan_on_startup: s.rescan_on_startup,
            settings_error: String::new(),
            folder_error: String::new(),
            picker_open: false,
            _subs: subs,
        }
    }

    /// `useEffect(() => setDraft(settings), [settings])`.
    fn reset_draft(&mut self, cx: &mut Context<Self>) {
        let s = self.stores.settings.read(cx).settings.clone();
        self.api_url
            .update(cx, |i, cx| i.set_value(s.api_url.clone(), cx));
        self.download_dir
            .update(cx, |i, cx| i.set_value(s.download_dir.clone(), cx));
        self.wiki_search = s.wiki_search;
        self.rescan_on_startup = s.rescan_on_startup;
        cx.notify();
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        self.settings_error.clear();
        let update = SettingsUpdate {
            api_url: Some(self.api_url.read(cx).value().to_string()),
            download_dir: Some(self.download_dir.read(cx).value().to_string()),
            wiki_search: Some(self.wiki_search),
            rescan_on_startup: Some(self.rescan_on_startup),
        };
        let task = self
            .stores
            .settings
            .update(cx, |s, cx| s.update(update, cx));
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                match &result {
                    Ok(()) => toast::success(cx, "Settings saved"),
                    Err(e) => {
                        let msg = if e.is_empty() {
                            "Something went wrong.".to_string()
                        } else {
                            e.clone()
                        };
                        this.settings_error = msg.clone();
                        toast::error(cx, msg);
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn add_folder_from_field(&mut self, cx: &mut Context<Self>) {
        let path = self.folder_path.read(cx).value().trim().to_string();
        self.add_folder(path, cx);
    }

    fn add_folder(&mut self, path: String, cx: &mut Context<Self>) {
        if path.is_empty() {
            return;
        }
        self.folder_error.clear();
        let task = self
            .stores
            .settings
            .update(cx, |s, cx| s.add_library_folder(path, cx));
        let library = self.stores.library.clone();
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                match &result {
                    Ok(()) => {
                        this.folder_path.update(cx, |i, cx| i.set_value("", cx));
                        toast::success(cx, "Folder added");
                        library.update(cx, |l, cx| l.refresh(true, cx));
                    }
                    Err(e) => {
                        let msg = if e.is_empty() {
                            "There was an error adding the folder.".to_string()
                        } else {
                            e.clone()
                        };
                        this.folder_error = msg.clone();
                        toast::error(cx, msg);
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn remove_folder(&mut self, dir: String, cx: &mut Context<Self>) {
        self.folder_error.clear();
        let task = self
            .stores
            .settings
            .update(cx, |s, cx| s.remove_library_folder(dir, cx));
        let library = self.stores.library.clone();
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                match &result {
                    Ok(()) => {
                        toast::success(cx, "Folder removed");
                        library.update(cx, |l, cx| l.refresh(true, cx));
                    }
                    Err(e) => {
                        let msg = if e.is_empty() {
                            "There was an error deleting the library.".to_string()
                        } else {
                            e.clone()
                        };
                        this.folder_error = msg.clone();
                        toast::error(cx, msg);
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Native folder picker. `then` gets the chosen path.
    fn browse(
        &mut self,
        cx: &mut Context<Self>,
        then: impl FnOnce(&mut Self, String, &mut Context<Self>) + 'static,
    ) {
        if self.picker_open {
            return;
        }
        self.picker_open = true;
        self.folder_error.clear();
        cx.spawn(async move |this, cx| {
            let picked = crate::runtime::run(async {
                rfd::AsyncFileDialog::new()
                    .pick_folder()
                    .await
                    .map(|h| h.path().to_string_lossy().into_owned())
            })
            .await;
            this.update(cx, |this, cx| {
                this.picker_open = false;
                if let Some(path) = picked {
                    then(this, path, cx);
                }
            })
            .ok();
        })
        .detach();
    }

    fn section(title: &'static str) -> gpui::Div {
        div()
            .flex()
            .flex_col()
            .gap(px(12.0))
            .p(px(18.0))
            .rounded(px(14.0))
            .border_1()
            .border_color(gpui::rgba(0xffffff12))
            .bg(gpui::rgba(0xffffff0b))
            .child(
                div()
                    .text_size(px(15.0))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(theme::text())
                    .child(title),
            )
    }

    fn label(text: &'static str) -> impl IntoElement {
        div()
            .text_size(px(12.0))
            .text_color(rgb(0xb8b8b8))
            .child(text)
    }

    fn field(input: &Entity<TextInput>) -> gpui::Div {
        div()
            .flex()
            .items_center()
            .flex_1()
            .min_w_0()
            .h(px(36.0))
            .px(px(12.0))
            .rounded(px(6.0))
            .border_1()
            .border_color(gpui::rgba(0xffffff24))
            .bg(gpui::rgba(0xffffff0a))
            .child(input.clone())
    }
}

impl Render for SettingsPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (output_dirs, server_url) = {
            let s = self.stores.settings.read(cx);
            (
                s.settings.output_dirs.clone(),
                s.client.as_ref().map(|c| c.base_url().to_string()),
            )
        };

        let remote = server_config::current().remote;
        let native_picker = server_config::has_native_folder_picker();
        let server = Self::section("Server")
            .child(div().text_size(px(13.0)).text_color(rgb(0xb8b8b8)).child(
                server_url.map(SharedString::from).unwrap_or_else(|| "No server configured".into()),
            ))
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(rgb(0x9a9a9a))
                    .child(if remote { "Remote server" } else { "Local server started by BetterRack" }),
            )
            .child(
                div()
                    .flex()
                    .gap(px(8.0))
                    .child(button(
                        "change-server",
                        "Change server",
                        ButtonVariant::Secondary,
                        cx.listener(|_, _, _, cx| modals::open_server(cx, false)),
                    ))
                    .when(remote, |s| {
                        s.child(button(
                            "unlink-server",
                            "Unlink server",
                            ButtonVariant::Secondary,
                            cx.listener(|_, _, _, cx| {
                                confirm::ask(
                                    cx,
                                    ConfirmOptions::new(
                                        "Unlink server?",
                                        "BetterRack will go back to its own local server and restart.",
                                    )
                                    .labels("Unlink", "Cancel"),
                                    |answer, _, cx| {
                                        if answer == Some(true) {
                                            crate::app::unlink_server(cx);
                                        }
                                    },
                                );
                            }),
                        ))
                    }),
            );

        let downloads = Self::section("Downloads")
            .child(Self::label("Download folder"))
            .child(
                div()
                    .flex()
                    .gap(px(8.0))
                    .child(Self::field(&self.download_dir))
                    .when(native_picker, |s| {
                        s.child(button(
                            "browse-download",
                            "Browse…",
                            ButtonVariant::Secondary,
                            cx.listener(|this, _, _, cx| {
                                this.browse(cx, |this, path, cx| {
                                    this.download_dir.update(cx, |i, cx| i.set_value(path, cx))
                                })
                            }),
                        ))
                    }),
            );

        let store = Self::section("Store configuration")
            .child(Self::label("API URL"))
            .child(div().flex().child(Self::field(&self.api_url)));

        let folder_rows: Vec<gpui::AnyElement> = if output_dirs.is_empty() {
            vec![
                div()
                    .text_size(px(13.0))
                    .text_color(rgb(0x9a9a9a))
                    .child("No library folders configured yet.")
                    .into_any_element(),
            ]
        } else {
            output_dirs
                .iter()
                .map(|dir| {
                    let remove = dir.clone();
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(10.0))
                        .px(px(12.0))
                        .h(px(36.0))
                        .rounded(px(6.0))
                        .bg(gpui::rgba(0xffffff0a))
                        .child(
                            div()
                                .min_w_0()
                                .truncate()
                                .text_size(px(13.0))
                                .child(dir.clone()),
                        )
                        .child(
                            div()
                                .id(SharedString::from(format!("remove-{dir}")))
                                .flex()
                                .items_center()
                                .justify_center()
                                .size(px(24.0))
                                .rounded_full()
                                .cursor_pointer()
                                .hover(|s| s.bg(gpui::rgba(0xe85d5d2e)))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.remove_folder(remove.clone(), cx)
                                }))
                                .child(icon(Icon::Close, px(14.0)).text_color(theme::text())),
                        )
                        .into_any_element()
                })
                .collect()
        };

        let folders = Self::section("Library folders")
            .children(folder_rows)
            .child(
                div()
                    .flex()
                    .gap(px(8.0))
                    .child(Self::field(&self.folder_path))
                    .when(native_picker, |s| {
                        s.child(button(
                            "browse-library",
                            "Browse…",
                            ButtonVariant::Secondary,
                            cx.listener(|this, _, _, cx| {
                                this.browse(cx, |this, path, cx| this.add_folder(path, cx))
                            }),
                        ))
                    })
                    .child(button(
                        "add-library",
                        "Add Folder",
                        ButtonVariant::Classic,
                        cx.listener(|this, _, _, cx| this.add_folder_from_field(cx)),
                    )),
            )
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(theme::error())
                    .child(self.folder_error.clone()),
            );

        let identification = Self::section("Identification")
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.0))
                    .child(
                        div()
                            .id("wiki-search-tip")
                            .self_start()
                            .tooltip(|_, cx| {
                                cx.new(|_| HoverTip(WIKI_SEARCH_TIP.into())).into()
                            })
                            .child(checkbox(
                                "wiki-search",
                                "Search the wiki for metadata",
                                self.wiki_search,
                                cx.listener_toggle(|this, on| this.wiki_search = on),
                            )),
                    )
                    .child(div().flex().child(button(
                        "reidentify-all",
                        "Re-identify all",
                        ButtonVariant::Secondary,
                        cx.listener(|this, _, _, cx| {
                            let library = this.stores.library.clone();
                            confirm::ask(
                                cx,
                                ConfirmOptions::new(
                                    "Re-identify all comics?",
                                    "This will re-run identification for every comic in your library, overwriting any existing matches.",
                                )
                                .labels("Re-identify", "Cancel"),
                                move |answer, _, cx| {
                                    if answer == Some(true) {
                                        library.update(cx, |s, cx| s.reidentify_all(cx));
                                    }
                                },
                            );
                        }),
                    ))),
            )
            .child(checkbox(
                "rescan",
                "Re-scan on start up",
                self.rescan_on_startup,
                cx.listener_toggle(|this, on| this.rescan_on_startup = on),
            ));

        let header = div()
            .flex()
            .flex_none()
            .items_center()
            .justify_between()
            .gap(px(12.0))
            .px(px(PAGE_PAD_X))
            .pt(px(14.0))
            .pb(px(18.0))
            .border_b_1()
            .border_color(theme::border_subtle())
            .child(
                div()
                    .text_size(px(18.0))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child("Settings"),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(theme::error())
                            .child(self.settings_error.clone()),
                    )
                    .child(button(
                        "save",
                        "Save",
                        ButtonVariant::Classic,
                        cx.listener(|this, _, _, cx| this.save(cx)),
                    )),
            );

        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .h_full()
            .child(header)
            .child(
                div()
                    .id("settings-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px(px(PAGE_PAD_X))
                    .py(px(16.0))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(px(16.0))
                            .items_start()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(16.0))
                                    .flex_1()
                                    .min_w(px(340.0))
                                    .child(server)
                                    .child(downloads)
                                    .child(store),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(16.0))
                                    .flex_1()
                                    .min_w(px(340.0))
                                    .child(folders)
                                    .child(identification),
                            ),
                    ),
            )
    }
}

/// Adapter from the checkbox's `(bool, &mut Window, &mut App)` callback to a page mutation.
trait ListenerToggle {
    fn listener_toggle(
        &self,
        f: impl Fn(&mut SettingsPage, bool) + 'static,
    ) -> impl Fn(bool, &mut Window, &mut gpui::App) + 'static;
}

impl ListenerToggle for Context<'_, SettingsPage> {
    fn listener_toggle(
        &self,
        f: impl Fn(&mut SettingsPage, bool) + 'static,
    ) -> impl Fn(bool, &mut Window, &mut gpui::App) + 'static {
        let this = self.entity();
        move |on, _, cx| {
            this.update(cx, |page, cx| {
                f(page, on);
                cx.notify();
            })
        }
    }
}
