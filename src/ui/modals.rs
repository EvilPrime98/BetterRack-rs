//! `NewFolderModal`, `MoveFileModal`, `LinkPickerModal` and `DownloadDirModal`
//! (`components/*-modal`), hosted by one global entity like the confirm dialog:
//! `modals::open_new_folder(cx, parent)`, `modals::open_move_file(cx, uid)`,
//! `modals::open_link_picker(cx, links, title, on_done)` and `modals::open_download_dir(cx, on_done)`
//! from anywhere. The two pickers answer through a callback (`None` = dismissed), which stands in
//! for the promises of the React context providers.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    AppContext as _, App, Context, Entity, FocusHandle, Focusable, Global, InteractiveElement,
    IntoElement, MouseButton, ParentElement, Render, SharedString, StatefulInteractiveElement,
    Styled, StyledImage as _, Subscription, Window, actions, div, img, prelude::*, px, rgb, uniform_list,
};

use crate::model::{DEFAULT_IMAGE_SIZE, LibraryGroup, MetaSource, StoreLink, WikiComic, json_text};
use crate::state::thumbnails::Thumb;
use crate::platform::server_config;
use crate::ui::confirm::{self, ConfirmOptions};
use crate::runtime;
use crate::state::Stores;
use crate::state::directories::filter_dirs;
use crate::ui::components::button::{ButtonVariant, button};
use crate::ui::components::checkbox::checkbox;
use crate::ui::components::text_input::{TextInput, TextInputEvent};
use crate::ui::icons::{Icon, icon};
use crate::ui::theme;

actions!(modals, [CloseModal]);

const ITEM_H: f32 = 34.0;

/// A folder the file can be moved into.
#[derive(Debug, Clone, PartialEq)]
pub struct Destination {
    pub uid: String,
    /// `Group / Parent / Folder`.
    pub path: String,
    /// Not a top-level folder of its group (hidden by the "Sub-folders" toggle).
    pub nested: bool,
}

/// `buildFolderPath`: a group is its own name; a folder is its ancestor chain, prefixed by the
/// group that owns the top of the chain.
pub fn folder_path(uid: &str, groups: &[LibraryGroup]) -> String {
    if let Some(g) = groups.iter().find(|g| g.uid == uid) {
        return g.name.clone();
    }
    let mut by_uid = HashMap::new();
    let mut owner = HashMap::new();
    for g in groups {
        for e in &g.entries {
            by_uid.insert(e.uid.as_str(), e);
            owner.insert(e.uid.as_str(), g);
        }
    }
    let mut names: Vec<&str> = Vec::new();
    let mut current = by_uid.get(uid).copied();
    // Bounded so a (corrupt) parent cycle cannot hang the UI.
    for _ in 0..256 {
        let Some(entry) = current else { break };
        names.push(&entry.name);
        if entry.parent_id.is_empty() {
            if let Some(g) = owner.get(entry.uid.as_str()) {
                names.push(&g.name);
            }
            break;
        }
        current = by_uid.get(entry.parent_id.as_str()).copied();
    }
    names.reverse();
    names.join(" / ")
}

/// Every folder except `moving` itself and anything nested under it (a folder cannot move into its
/// own subtree).
pub fn move_destinations(groups: &[LibraryGroup], moving: &str) -> Vec<Destination> {
    let entries: Vec<_> = groups.iter().flat_map(|g| g.entries.iter()).collect();
    let mut excluded: HashSet<&str> = HashSet::from([moving]);
    loop {
        let before = excluded.len();
        for e in &entries {
            if !e.parent_id.is_empty() && excluded.contains(e.parent_id.as_str()) {
                excluded.insert(e.uid.as_str());
            }
        }
        if excluded.len() == before {
            break;
        }
    }
    entries
        .iter()
        .filter(|e| e.did && !excluded.contains(e.uid.as_str()))
        .map(|e| Destination {
            uid: e.uid.clone(),
            path: folder_path(&e.uid, groups),
            nested: !e.parent_id.is_empty(),
        })
        .collect()
}

/// Destinations passing the search box and the "Sub-folders" toggle.
pub fn filter_destinations<'a>(all: &'a [Destination], query: &str, subfolders: bool) -> Vec<&'a Destination> {
    let q = query.trim().to_lowercase();
    all.iter()
        .filter(|d| (subfolders || !d.nested) && (q.is_empty() || d.path.to_lowercase().contains(&q)))
        .collect()
}

type PickLink = Box<dyn FnOnce(Option<StoreLink>, &mut App)>;
type PickDir = Box<dyn FnOnce(Option<String>, &mut App)>;

/// The folder list of the download-folder picker (`TListState`).
enum DirList {
    Loading,
    Failed(String),
    Ready(Rc<Vec<String>>),
}

/// What the identify modal lists (`suggestions` + `isSearching` in the React component).
enum Suggestions {
    Idle,
    Searching,
    Found(Rc<Vec<WikiComic>>),
    Failed(String),
}

enum Modal {
    NewFolder { parent: Option<String>, input: Entity<TextInput> },
    MoveFile { uid: String, all: Rc<Vec<Destination>>, search: Entity<TextInput>, subfolders: bool },
    LinkPicker { title: String, links: Rc<Vec<StoreLink>>, on_done: Option<PickLink> },
    DownloadDir { dirs: DirList, search: Entity<TextInput>, subfolders: bool, on_done: Option<PickDir> },
    Identify { uid: String, input: Entity<TextInput>, results: Suggestions },
    /// `ServerModal`: connect to a remote server, or go back to the local one.
    Server { url: Entity<TextInput>, key: Entity<TextInput>, remote: bool, error: String, busy: bool },
}

pub struct ModalHost {
    modal: Option<Modal>,
    /// Focus the modal's text input on the next render (needs a `Window`).
    needs_focus: bool,
    /// Discards a directory list that arrives after its modal was closed or replaced.
    dir_request: u64,
    /// Same idea for identify searches (also the debounce token).
    ident_request: u64,
    focus: FocusHandle,
    _subs: Vec<Subscription>,
}

struct ModalGlobal(Entity<ModalHost>);
impl Global for ModalGlobal {}

pub fn init(cx: &mut App) -> Entity<ModalHost> {
    cx.bind_keys([gpui::KeyBinding::new("escape", CloseModal, Some("Modal"))]);
    let host = cx.new(|cx| ModalHost { modal: None, needs_focus: false, dir_request: 0, ident_request: 0, focus: cx.focus_handle(), _subs: Vec::new() });
    cx.set_global(ModalGlobal(host.clone()));
    host
}

fn host(cx: &App) -> Option<Entity<ModalHost>> {
    cx.try_global::<ModalGlobal>().map(|g| g.0.clone())
}

/// `openNewFolderModal(uid)`: `parent` is the folder (or group) the new folder is created in.
pub fn open_new_folder(cx: &mut App, parent: Option<String>) {
    let Some(host) = host(cx) else { return };
    host.update(cx, |h, cx| {
        h.close(cx);
        let input = cx.new(|cx| TextInput::new("Folder name", cx));
        h._subs = vec![cx.subscribe(&input, |this, _, ev: &TextInputEvent, cx| match ev {
            TextInputEvent::Submit => this.submit_new_folder(cx),
            TextInputEvent::Cancel => this.close(cx),
            TextInputEvent::Changed => {}
        })];
        h.modal = Some(Modal::NewFolder { parent, input });
        h.needs_focus = true;
        cx.notify();
    });
}

/// `openMoveFileModal(uid, name)`.
pub fn open_move_file(cx: &mut App, uid: String) {
    let Some(host) = host(cx) else { return };
    let Some(stores) = cx.try_global::<Stores>().cloned() else { return };
    let all = Rc::new(move_destinations(&stores.library.read(cx).groups, &uid));
    host.update(cx, |h, cx| {
        h.close(cx);
        let search = cx.new(|cx| TextInput::new("Search folders...", cx));
        h._subs = vec![cx.subscribe(&search, |this, _, ev: &TextInputEvent, cx| match ev {
            TextInputEvent::Changed => cx.notify(),
            TextInputEvent::Cancel => this.close(cx),
            TextInputEvent::Submit => {}
        })];
        h.modal = Some(Modal::MoveFile { uid, all, search, subfolders: true });
        h.needs_focus = true;
        cx.notify();
    });
}

/// `openServerModal()`: seeded from the saved config. Saving relaunches the app, which re-runs the
/// startup sequence against the new server.
pub fn open_server(cx: &mut App, _mandatory: bool) {
    let Some(host) = host(cx) else { return };
    let config = server_config::current().clone();
    host.update(cx, |h, cx| {
        h.close(cx);
        let url = cx.new(|cx| TextInput::new("192.168.1.100:3000", cx).with_value(config.url.clone()));
        let key = cx.new(|cx| {
            TextInput::new("API key (optional)", cx).with_value(server_config::api_key().unwrap_or_default())
        });
        let on_event = |this: &mut ModalHost, _: Entity<TextInput>, ev: &TextInputEvent, cx: &mut Context<ModalHost>| {
            match ev {
                TextInputEvent::Submit => this.submit_server(cx),
                TextInputEvent::Cancel => this.close(cx),
                TextInputEvent::Changed => {}
            }
        };
        h._subs = vec![cx.subscribe(&url, on_event), cx.subscribe(&key, on_event)];
        h.modal = Some(Modal::Server { url, key, remote: config.remote, error: String::new(), busy: false });
        h.needs_focus = true;
        cx.notify();
    });
}

/// `openLinkPickerModal(links, title)`: the user picks one download link. `on_done` gets `None`
/// when the dialog is dismissed or replaced.
pub fn open_link_picker(
    cx: &mut App,
    links: Vec<StoreLink>,
    title: String,
    on_done: impl FnOnce(Option<StoreLink>, &mut App) + 'static,
) {
    let Some(host) = host(cx) else {
        on_done(None, cx);
        return;
    };
    host.update(cx, |h, cx| {
        h.close(cx);
        h.modal = Some(Modal::LinkPicker { title, links: Rc::new(links), on_done: Some(Box::new(on_done)) });
        h.needs_focus = true;
        cx.notify();
    });
}

/// `openDownloadDirModal()`: the user picks one of the known folders. The cached list shows at
/// once and is refreshed in the background. `on_done` gets `None` when dismissed or replaced.
pub fn open_download_dir(cx: &mut App, on_done: impl FnOnce(Option<String>, &mut App) + 'static) {
    let (Some(host), Some(stores)) = (host(cx), cx.try_global::<Stores>().cloned()) else {
        on_done(None, cx);
        return;
    };
    let cached = stores.directories.read(cx).cached();
    let refresh = stores.directories.update(cx, |d, cx| d.refresh(cx));
    host.update(cx, |h, cx| {
        h.close(cx);
        let search = cx.new(|cx| TextInput::new("Search folders...", cx));
        h._subs = vec![cx.subscribe(&search, |this, _, ev: &TextInputEvent, cx| match ev {
            TextInputEvent::Changed => cx.notify(),
            TextInputEvent::Cancel => this.close(cx),
            TextInputEvent::Submit => {}
        })];
        let dirs = cached.map_or(DirList::Loading, |d| DirList::Ready(Rc::new(d)));
        h.modal = Some(Modal::DownloadDir { dirs, search, subfolders: true, on_done: Some(Box::new(on_done)) });
        h.needs_focus = true;
        h.dir_request += 1;
        let request = h.dir_request;
        cx.spawn(async move |this, cx| {
            let result = refresh.await;
            this.update(cx, |h, cx| h.set_dirs(request, result, cx)).ok();
        })
        .detach();
        cx.notify();
    });
}

/// `ComicIdentifier`: search the wiki for the file `uid` and commit the picked comic.
pub fn open_identify(cx: &mut App, uid: String) {
    let Some(host) = host(cx) else { return };
    let thumbs = cx.try_global::<Stores>().map(|s| s.thumbs.clone());
    host.update(cx, |h, cx| {
        h.close(cx);
        let input = cx.new(|cx| TextInput::new("Search a comic..", cx));
        let mut subs = vec![cx.subscribe(&input, |this, _, ev: &TextInputEvent, cx| match ev {
            TextInputEvent::Changed => this.identify_query_changed(cx),
            TextInputEvent::Cancel => this.close(cx),
            TextInputEvent::Submit => {}
        })];
        // Suggestion covers arrive asynchronously.
        if let Some(thumbs) = thumbs {
            subs.push(cx.observe(&thumbs, |_, _, cx| cx.notify()));
        }
        h._subs = subs;
        h.modal = Some(Modal::Identify { uid, input, results: Suggestions::Idle });
        h.needs_focus = true;
        cx.notify();
    });
}

/// Dev aid (`BETTERRACK_OPEN_IDENTIFY`): type `query` into the open identify modal.
pub fn dev_prefill_identify(cx: &mut App, query: &str) {
    let Some(host) = host(cx) else { return };
    host.update(cx, |h, cx| {
        if let Some(Modal::Identify { input, .. }) = &h.modal {
            input.update(cx, |i, cx| i.set_value(query, cx));
            h.identify_query_changed(cx);
        }
    });
}

impl ModalHost {
    /// Debounced (500 ms) wiki search; a newer keystroke, or closing, drops the older answer.
    fn identify_query_changed(&mut self, cx: &mut Context<Self>) {
        let Some(Modal::Identify { input, results, .. }) = &mut self.modal else { return };
        let query = input.read(cx).value().trim().to_string();
        self.ident_request += 1;
        let request = self.ident_request;
        if query.is_empty() {
            *results = Suggestions::Idle;
            cx.notify();
            return;
        }
        let Some(client) = cx.try_global::<Stores>().and_then(|s| s.library.read(cx).client.clone()) else {
            return;
        };
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(500)).await;
            let current = this
                .update(cx, |h, cx| {
                    if h.ident_request != request {
                        return false;
                    }
                    if let Some(Modal::Identify { results, .. }) = &mut h.modal {
                        *results = Suggestions::Searching;
                        cx.notify();
                        return true;
                    }
                    false
                })
                .unwrap_or(false);
            if !current {
                return;
            }
            let found = runtime::run(async move { client.wiki_search(&query, DEFAULT_IMAGE_SIZE).await }).await;
            this.update(cx, |h, cx| h.set_suggestions(request, found.map_err(|e| e.to_string()), cx)).ok();
        })
        .detach();
    }

    fn set_suggestions(&mut self, request: u64, found: Result<Vec<WikiComic>, String>, cx: &mut Context<Self>) {
        if request != self.ident_request {
            return;
        }
        let Some(Modal::Identify { results, .. }) = &mut self.modal else { return };
        *results = match found {
            Ok(comics) => {
                if let Some(stores) = cx.try_global::<Stores>().cloned() {
                    stores.thumbs.update(cx, |t, cx| {
                        for c in &comics {
                            let cover = c.cover();
                            if !cover.is_empty() {
                                t.ensure_external(&cover, cx);
                            }
                        }
                    });
                }
                Suggestions::Found(Rc::new(comics))
            }
            Err(message) => Suggestions::Failed(message),
        };
        cx.notify();
    }

    /// Commit a pick: the card updates at once; a failed commit rolls back with an error toast.
    fn pick_comic(&mut self, comic: WikiComic, cx: &mut Context<Self>) {
        let Some(Modal::Identify { uid, .. }) = &self.modal else { return };
        let uid = uid.clone();
        self.close(cx);
        let Some(stores) = cx.try_global::<Stores>().cloned() else { return };
        let Some(client) = stores.library.read(cx).client.clone() else { return };
        let previous = stores.identify.read(cx).snapshot(&uid);
        stores.identify.update(cx, |s, cx| s.set_identified(&uid, Some(comic.clone()), Some(MetaSource::Wiki), cx));
        cx.spawn(async move |_, cx| {
            let (id, c) = (uid.clone(), comic);
            let result = runtime::run(async move { client.identify_file(&id, &c).await }).await;
            cx.update(|cx| match result {
                Ok(()) => crate::ui::toast::success(cx, "Comic identified"),
                Err(e) => {
                    stores.identify.update(cx, |s, cx| s.restore(&uid, previous, cx));
                    crate::ui::toast::error(cx, e.to_string());
                }
            });
        })
        .detach();
    }

    /// "Unidentify" button: confirm, then clear the metadata.
    fn unidentify(&mut self, cx: &mut Context<Self>) {
        let Some(Modal::Identify { uid, .. }) = &self.modal else { return };
        let uid = uid.clone();
        confirm::ask(
            cx,
            ConfirmOptions::new("Un-identify comic?", "This will remove the identified metadata for this comic.")
                .labels("Un-identify", "Cancel"),
            move |answer, _, cx| {
                if answer != Some(true) {
                    return;
                }
                if let Some(host) = host(cx) {
                    host.update(cx, |h, cx| h.close(cx));
                }
                if let Some(stores) = cx.try_global::<Stores>().cloned() {
                    stores.library.update(cx, |s, cx| s.unidentify_file(uid, cx));
                }
            },
        );
    }

    fn close(&mut self, cx: &mut Context<Self>) {
        let modal = self.modal.take();
        self.ident_request += 1;
        self._subs.clear();
        cx.notify();
        // A dismissed picker answers `None` so the flow waiting on it can end. Deferred, so the
        // callback may open another modal.
        match modal {
            Some(Modal::LinkPicker { on_done: Some(done), .. }) => cx.defer(move |cx| done(None, cx)),
            Some(Modal::DownloadDir { on_done: Some(done), .. }) => cx.defer(move |cx| done(None, cx)),
            _ => {}
        }
    }

    fn set_dirs(&mut self, request: u64, result: Result<Vec<String>, String>, cx: &mut Context<Self>) {
        if request != self.dir_request {
            return;
        }
        let Some(Modal::DownloadDir { dirs, .. }) = &mut self.modal else { return };
        match (result, &*dirs) {
            (Ok(fresh), DirList::Ready(shown)) if **shown == fresh => return,
            (Ok(fresh), _) => *dirs = DirList::Ready(Rc::new(fresh)),
            // A failed refresh must not replace a list the user can already see.
            (Err(_), DirList::Ready(_)) => return,
            (Err(message), _) => *dirs = DirList::Failed(message),
        }
        cx.notify();
    }

    fn pick_link(&mut self, link: StoreLink, cx: &mut Context<Self>) {
        let Some(Modal::LinkPicker { on_done, .. }) = &mut self.modal else { return };
        let done = on_done.take();
        self.close(cx);
        if let Some(done) = done {
            cx.defer(move |cx| done(Some(link), cx));
        }
    }

    fn pick_dir(&mut self, dir: String, cx: &mut Context<Self>) {
        let Some(Modal::DownloadDir { on_done, .. }) = &mut self.modal else { return };
        let done = on_done.take();
        self.close(cx);
        if let Some(done) = done {
            cx.defer(move |cx| done(Some(dir), cx));
        }
    }

    fn submit_new_folder(&mut self, cx: &mut Context<Self>) {
        let Some(Modal::NewFolder { parent, input }) = &self.modal else { return };
        let name = input.read(cx).value().trim().to_string();
        if name.is_empty() {
            return;
        }
        let parent = parent.clone();
        self.close(cx);
        if let Some(stores) = cx.try_global::<Stores>().cloned() {
            stores.library.update(cx, |s, cx| s.create_folder(name, parent, cx));
        }
    }

    /// "Connect": remote on = check the server answers as BetterRack, save it (key to the keychain),
    /// relaunch. Remote off = back to the local sidecar.
    fn submit_server(&mut self, cx: &mut Context<Self>) {
        let Some(Modal::Server { url, key, remote, error, busy }) = &mut self.modal else { return };
        if *busy {
            return;
        }
        if !*remote {
            self.close(cx);
            cx.defer(|cx| crate::app::unlink_server(cx));
            return;
        }
        let (url, key) = (url.read(cx).value().trim().to_string(), key.read(cx).value().trim().to_string());
        if url.is_empty() {
            *error = "Enter the server address.".into();
            cx.notify();
            return;
        }
        error.clear();
        *busy = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let check = {
                let (url, key) = (url.clone(), key.clone());
                runtime::run(async move {
                    let client = crate::api::ApiClient::new(&url, Some(key)).map_err(|e| e.to_string())?;
                    match client.healthz().await {
                        Ok(h) if h.app == "betterrack" => Ok(()),
                        Ok(_) => Err("That server does not look like BetterRack.".to_string()),
                        Err(e) => Err(format!("Could not reach the server: {e}")),
                    }
                })
                .await
            };
            let result = check.and_then(|()| server_config::set_remote(&url, &key));
            this.update(cx, |this, cx| match result {
                Ok(()) => {
                    this.close(cx);
                    cx.defer(|cx| crate::app::relaunch(cx));
                }
                Err(message) => {
                    if let Some(Modal::Server { error, busy, .. }) = &mut this.modal {
                        *error = message;
                        *busy = false;
                    }
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn move_to(&mut self, uid: String, target: Option<String>, cx: &mut Context<Self>) {
        self.close(cx);
        if let Some(stores) = cx.try_global::<Stores>().cloned() {
            stores.library.update(cx, |s, cx| s.move_file(uid, target, cx));
        }
    }

    fn frame(&self, title: &'static str, cx: &mut Context<Self>) -> gpui::Div {
        let _ = cx;
        div()
            .flex()
            .flex_col()
            .gap(px(14.0))
            .w(px(420.0))
            .max_w_full()
            .p(px(20.0))
            .bg(theme::bg_modal())
            .border_1()
            .border_color(theme::border_subtle())
            .rounded(px(14.0))
            .child(
                div()
                    .text_size(px(15.0))
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(theme::text())
                    .child(title),
            )
    }

    fn note(text: impl Into<SharedString>) -> gpui::Div {
        div().py(px(16.0)).text_center().text_size(px(13.0)).text_color(rgb(0x8f8f8f)).child(text.into())
    }

    fn field(input: &Entity<TextInput>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .h(px(36.0))
            .px(px(12.0))
            .rounded(px(6.0))
            .border_1()
            .border_color(gpui::rgba(0xffffff24))
            .bg(gpui::rgba(0xffffff0a))
            .child(input.clone())
    }
}

impl Focusable for ModalHost {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for ModalHost {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(modal) = &self.modal else { return div().into_any_element() };
        if self.needs_focus {
            self.needs_focus = false;
            match modal {
                Modal::NewFolder { input, .. } => {
                    let handle = input.read(cx).focus_handle(cx);
                    window.focus(&handle, cx)
                }
                Modal::MoveFile { search, .. } | Modal::DownloadDir { search, .. } => {
                    let handle = search.read(cx).focus_handle(cx);
                    window.focus(&handle, cx)
                }
                // No input: the host itself takes focus so Escape reaches `CloseModal`.
                Modal::LinkPicker { .. } => window.focus(&self.focus, cx),
                Modal::Identify { input, .. } => {
                    let handle = input.read(cx).focus_handle(cx);
                    window.focus(&handle, cx)
                }
                Modal::Server { url, .. } => {
                    let handle = url.read(cx).focus_handle(cx);
                    window.focus(&handle, cx)
                }
            }
        }

        // The identify search sits near the top, like the command-palette style of the original.
        let top_aligned = matches!(modal, Modal::Identify { .. });
        let body = match modal {
            Modal::Server { url, key, remote, error, busy } => {
                let this = cx.entity();
                self.frame("Connect to your server", cx)
                    .child(
                        div()
                            .text_size(px(13.0))
                            .text_color(rgb(0x9a9a9a))
                            .child("Enter the address of the BetterRack server on your network."),
                    )
                    .child(Self::field(url))
                    .child(checkbox("server-remote", "Use this as my library server", *remote, move |on, _, cx| {
                        this.update(cx, |h, cx| {
                            if let Some(Modal::Server { remote, .. }) = &mut h.modal {
                                *remote = on;
                                cx.notify();
                            }
                        })
                    }))
                    .when(*remote, |s| s.child(Self::field(key)))
                    .child(div().text_size(px(12.0)).text_color(theme::error()).child(error.clone()))
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap(px(8.0))
                            .child(button("srv-cancel", "Cancel", ButtonVariant::Secondary, cx.listener(|this, _, _, cx| this.close(cx))))
                            .child(button(
                                "srv-connect",
                                if *busy { "Connecting…" } else { "Connect" },
                                ButtonVariant::Classic,
                                cx.listener(|this, _, _, cx| this.submit_server(cx)),
                            )),
                    )
                    .into_any_element()
            }
            Modal::NewFolder { input, .. } => self
                .frame("New folder", cx)
                .child(Self::field(input))
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap(px(8.0))
                        .child(button("nf-cancel", "Cancel", ButtonVariant::Secondary, cx.listener(|this, _, _, cx| this.close(cx))))
                        .child(button("nf-create", "Create", ButtonVariant::Classic, cx.listener(|this, _, _, cx| this.submit_new_folder(cx)))),
                )
                .into_any_element(),
            Modal::MoveFile { uid, all, search, subfolders } => {
                let query = search.read(cx).value().to_string();
                let shown: Rc<Vec<Destination>> =
                    Rc::new(filter_destinations(all, &query, *subfolders).into_iter().cloned().collect());
                let q = query.trim().to_lowercase();
                let show_root = q.is_empty() || "library root".contains(&q);
                let empty_note = (!show_root && shown.is_empty())
                    .then(|| if all.is_empty() { "No folders yet." } else { "No folders match your search." });
                // Row 0 is "Library root" when it matches, then the folders.
                let total = shown.len() + usize::from(show_root);
                let moving = uid.clone();
                let rows = shown.clone();

                let list = uniform_list(
                    "move-targets",
                    total,
                    cx.processor(move |_this, range: std::ops::Range<usize>, _w, cx| {
                        range
                            .map(|ix| {
                                let (target, label, glyph_color): (Option<String>, SharedString, _) =
                                    if show_root && ix == 0 {
                                        (None, "Library root".into(), theme::accent())
                                    } else {
                                        let d = &rows[ix - usize::from(show_root)];
                                        (Some(d.uid.clone()), d.path.clone().into(), rgb(0xc7c7c7))
                                    };
                                let moving = moving.clone();
                                div()
                                    .id(SharedString::from(format!("move-target-{ix}")))
                                    .w_full()
                                    .flex()
                                    .items_center()
                                    .gap(px(10.0))
                                    .h(px(ITEM_H))
                                    .px(px(10.0))
                                    .rounded(px(6.0))
                                    .cursor_pointer()
                                    .text_size(px(13.0))
                                    .text_color(rgb(0xe6e6e6))
                                    .hover(|s| s.bg(gpui::rgba(0xffffff0f)))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.move_to(moving.clone(), target.clone(), cx)
                                    }))
                                    .child(icon(Icon::Folder, px(14.0)).text_color(glyph_color))
                                    .child(div().min_w_0().truncate().child(label))
                            })
                            .collect()
                    }),
                )
                .h(px(ITEM_H * 8.0));

                self.frame("Move to:", cx)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(12.0))
                            .child(div().flex_1().child(Self::field(search)))
                            .child(checkbox("move-subfolders", "Sub-folders", *subfolders, {
                                let this = cx.entity();
                                move |on, _, cx| {
                                    this.update(cx, |h, cx| {
                                        if let Some(Modal::MoveFile { subfolders, .. }) = &mut h.modal {
                                            *subfolders = on;
                                            cx.notify();
                                        }
                                    })
                                }
                            })),
                    )
                    .child(list)
                    .children(empty_note.map(|n| {
                        div().text_size(px(13.0)).text_color(rgb(0x9a9a9a)).child(n)
                    }))
                    .into_any_element()
            }
            Modal::LinkPicker { title, links, .. } => {
                let rows = links.iter().enumerate().map(|(ix, link)| {
                    let picked = link.clone();
                    div()
                        .id(SharedString::from(format!("link-{ix}")))
                        .flex()
                        .items_center()
                        .gap(px(10.0))
                        .px(px(10.0))
                        .py(px(9.0))
                        .rounded(px(8.0))
                        .cursor_pointer()
                        .text_size(px(13.0))
                        .text_color(rgb(0xe0e0e0))
                        .hover(|s| s.bg(gpui::rgba(0xffffff0f)))
                        .on_click(cx.listener(move |this, _, _, cx| this.pick_link(picked.clone(), cx)))
                        .child(
                            div()
                                .flex_none()
                                .min_w(px(22.0))
                                .px(px(6.0))
                                .py(px(2.0))
                                .rounded(px(5.0))
                                .bg(gpui::rgba(0x34c3d11f))
                                .text_center()
                                .text_size(px(10.0))
                                .font_weight(gpui::FontWeight::BOLD)
                                .text_color(theme::accent())
                                .child((ix + 1).to_string()),
                        )
                        .child(div().min_w_0().truncate().child(link.title.clone()))
                }).collect::<Vec<_>>();
                self.frame("Choose a link", cx)
                    .w(px(460.0))
                    .when(!title.is_empty(), |s| {
                        s.child(div().truncate().text_size(px(12.0)).text_color(rgb(0x8f8f8f)).child(title.clone()))
                    })
                    .child(
                        div()
                            .id("link-list")
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .max_h(px(320.0))
                            .overflow_y_scroll()
                            .when(links.is_empty(), |s| s.child(Self::note("No links available.")))
                            .children(rows),
                    )
                    .into_any_element()
            }
            Modal::DownloadDir { dirs, search, subfolders, .. } => {
                let query = search.read(cx).value().to_string();
                let default_dir = cx
                    .try_global::<Stores>()
                    .map(|s| s.settings.read(cx).settings.download_dir.clone())
                    .unwrap_or_default();
                let (shown, empty_note): (Rc<Vec<String>>, Option<&'static str>) = match dirs {
                    DirList::Loading => (Rc::default(), Some("Loading…")),
                    DirList::Failed(_) => (Rc::default(), None),
                    DirList::Ready(all) if all.is_empty() => {
                        (Rc::default(), Some("No directories available. Set a download or library folder in Settings."))
                    }
                    DirList::Ready(all) => {
                        let shown = filter_dirs(all, &query, *subfolders);
                        let note = shown.is_empty().then_some("No folders match your search.");
                        (Rc::new(shown), note)
                    }
                };
                let failed = match dirs {
                    DirList::Failed(m) => Some(m.clone()),
                    _ => None,
                };
                let rows = shown.clone();
                let list = uniform_list(
                    "download-dirs",
                    shown.len(),
                    cx.processor(move |_this, range: std::ops::Range<usize>, _w, cx| {
                        range
                            .map(|ix| {
                                let dir = rows[ix].clone();
                                let is_default = dir == default_dir;
                                let chosen = dir.clone();
                                div()
                                    .id(SharedString::from(format!("download-dir-{ix}")))
                                    .w_full()
                                    .flex()
                                    .items_center()
                                    .gap(px(10.0))
                                    .h(px(ITEM_H))
                                    .px(px(10.0))
                                    .rounded(px(8.0))
                                    .cursor_pointer()
                                    .text_size(px(13.0))
                                    .text_color(rgb(0xe0e0e0))
                                    .when(is_default, |s| s.bg(gpui::rgba(0x34c3d114)))
                                    .hover(|s| s.bg(gpui::rgba(0xffffff0f)))
                                    .on_click(cx.listener(move |this, _, _, cx| this.pick_dir(chosen.clone(), cx)))
                                    .child(icon(Icon::Folder, px(14.0)).text_color(if is_default {
                                        theme::accent()
                                    } else {
                                        rgb(0xc7c7c7)
                                    }))
                                    .child(div().flex_1().min_w_0().truncate().child(dir))
                                    .when(is_default, |s| {
                                        s.child(
                                            div()
                                                .flex_none()
                                                .px(px(6.0))
                                                .py(px(2.0))
                                                .rounded(px(5.0))
                                                .bg(gpui::rgba(0x34c3d11f))
                                                .text_size(px(10.0))
                                                .font_weight(gpui::FontWeight::BOLD)
                                                .text_color(theme::accent())
                                                .child("DEFAULT"),
                                        )
                                    })
                            })
                            .collect()
                    }),
                )
                .h(px(ITEM_H * shown.len().min(8) as f32));

                self.frame("Download to:", cx)
                    .w(px(460.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(12.0))
                            .child(div().flex_1().child(Self::field(search)))
                            .child(checkbox("dir-subfolders", "Sub-folders", *subfolders, {
                                let this = cx.entity();
                                move |on, _, cx| {
                                    this.update(cx, |h, cx| {
                                        if let Some(Modal::DownloadDir { subfolders, .. }) = &mut h.modal {
                                            *subfolders = on;
                                            cx.notify();
                                        }
                                    })
                                }
                            })),
                    )
                    .child(list)
                    .children(empty_note.map(Self::note))
                    .children(failed.map(Self::note))
                    .into_any_element()
            }
            Modal::Identify { uid, input, results } => {
                let thumbs = cx.try_global::<Stores>().map(|s| s.thumbs.clone());
                let list: gpui::AnyElement = match results {
                    Suggestions::Found(comics) if !comics.is_empty() => {
                        let rows = comics.iter().enumerate().map(|(ix, comic)| {
                            let thumb = thumbs.as_ref().and_then(|t| t.read(cx).get(&comic.cover()).cloned());
                            let picked = comic.clone();
                            let issue = json_text(&comic.issue);
                            let issue = if issue.is_empty() { issue } else { format!("#{issue}") };
                            let meta = [json_text(&comic.volume), issue]
                                .into_iter()
                                .filter(|s| !s.is_empty())
                                .collect::<Vec<_>>()
                                .join(" · ");
                            let cover = div()
                                .flex_none()
                                .w(px(56.0))
                                .h(px(56.0 * 77.0 / 50.0))
                                .rounded(px(6.0))
                                .overflow_hidden()
                                .bg(rgb(0x232222));
                            let cover = match thumb {
                                Some(Thumb::Ready(image)) => {
                                    cover.child(img(image).size_full().object_fit(gpui::ObjectFit::Cover))
                                }
                                _ => cover,
                            };
                            div()
                                .id(SharedString::from(format!("suggestion-{ix}")))
                                .flex()
                                .items_center()
                                .gap(px(12.0))
                                .w_full()
                                .p(px(8.0))
                                .rounded(px(8.0))
                                .cursor_pointer()
                                .hover(|s| s.bg(gpui::rgba(0xffffff0f)))
                                .on_click(cx.listener(move |this, _, _, cx| this.pick_comic(picked.clone(), cx)))
                                .child(cover)
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap(px(2.0))
                                        .min_w_0()
                                        .child(
                                            div()
                                                .truncate()
                                                .text_size(px(13.0))
                                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                                .text_color(rgb(0xffffff))
                                                .child(comic.title.clone().unwrap_or_default()),
                                        )
                                        .child(
                                            div()
                                                .truncate()
                                                .text_size(px(10.0))
                                                .font_family(theme::FONT_MONO)
                                                .text_color(theme::accent())
                                                .child(meta.to_uppercase()),
                                        ),
                                )
                        });
                        div()
                            .id("suggestions")
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .p(px(8.0))
                            .max_h(px(420.0))
                            .overflow_y_scroll()
                            .children(rows)
                            .into_any_element()
                    }
                    other => {
                        let query = input.read(cx).value().trim().to_string();
                        let text: SharedString = match other {
                            Suggestions::Searching => "Searching…".into(),
                            Suggestions::Failed(m) if !m.is_empty() => m.clone().into(),
                            Suggestions::Found(_) | Suggestions::Failed(_) => "No comics found".into(),
                            // Typing but still inside the debounce window: nothing to say yet.
                            Suggestions::Idle if !query.is_empty() => "".into(),
                            Suggestions::Idle => format!("Start typing to search the wiki for item: {uid}").into(),
                        };
                        Self::note(text).px(px(10.0)).into_any_element()
                    }
                };
                div()
                    .flex()
                    .flex_col()
                    .w(px(480.0))
                    .max_w_full()
                    .bg(theme::bg_modal())
                    .border_1()
                    .border_color(theme::border_subtle())
                    .rounded(px(14.0))
                    .overflow_hidden()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(10.0))
                            .px(px(14.0))
                            .py(px(12.0))
                            .border_b_1()
                            .border_color(theme::border_subtle())
                            .child(icon(Icon::Search, px(16.0)).text_color(rgb(0x8f8f8f)))
                            .child(div().flex_1().min_w_0().child(input.clone()))
                            .child(
                                div()
                                    .id("unidentify")
                                    .flex_none()
                                    .px(px(10.0))
                                    .py(px(5.0))
                                    .rounded(px(6.0))
                                    .border_1()
                                    .border_color(gpui::rgba(0xdc5a5a80))
                                    .text_size(px(11.0))
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(rgb(0xf2a1a1))
                                    .cursor_pointer()
                                    .hover(|s| s.bg(gpui::rgba(0xdc5a5a1f)))
                                    .on_click(cx.listener(|this, _, _, cx| this.unidentify(cx)))
                                    .child("UNIDENTIFY"),
                            )
                            .child(
                                div()
                                    .id("identify-close")
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .flex_none()
                                    .size(px(26.0))
                                    .rounded(px(6.0))
                                    .cursor_pointer()
                                    .hover(|s| s.bg(gpui::rgba(0xffffff14)))
                                    .on_click(cx.listener(|this, _, _, cx| this.close(cx)))
                                    .child(icon(Icon::Close, px(14.0)).text_color(theme::text())),
                            ),
                    )
                    .child(list)
                    .into_any_element()
            }
        };

        div()
            .id("modal-overlay")
            .key_context("Modal")
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &CloseModal, _, cx| this.close(cx)))
            .absolute()
            .inset_0()
            .flex()
            .when(top_aligned, |s| s.items_start())
            .when(!top_aligned, |s| s.items_center())
            .justify_center()
            .p(px(16.0))
            .when(top_aligned, |s| s.pt(px(96.0)))
            .bg(gpui::rgba(0x0000008c))
            .occlude()
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| this.close(cx)))
            .child(
                div()
                    .id("modal-body")
                    // Clicks inside must not reach the dismissing backdrop.
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(body),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn entry(v: serde_json::Value) -> crate::model::LibraryEntry {
        serde_json::from_value(v).unwrap()
    }

    fn groups() -> Vec<LibraryGroup> {
        vec![LibraryGroup {
            uid: "g".into(),
            name: "Comics".into(),
            path: "/c".into(),
            entries: vec![
                entry(json!({"uid":"a","did":true,"name":"Batman","parentId":""})),
                entry(json!({"uid":"b","did":true,"name":"Year One","parentId":"a"})),
                entry(json!({"uid":"c","did":true,"name":"Deep","parentId":"b"})),
                entry(json!({"uid":"d","did":true,"name":"Superman","parentId":""})),
                entry(json!({"uid":"f","did":false,"name":"file.cbz","parentId":"a"})),
            ],
        }]
    }

    #[test]
    fn paths_are_group_then_ancestors() {
        assert_eq!(folder_path("g", &groups()), "Comics");
        assert_eq!(folder_path("c", &groups()), "Comics / Batman / Year One / Deep");
        assert_eq!(folder_path("d", &groups()), "Comics / Superman");
    }

    #[test]
    fn a_folder_cannot_move_into_its_own_subtree() {
        let uids: Vec<_> = move_destinations(&groups(), "a").into_iter().map(|d| d.uid).collect();
        assert_eq!(uids, ["d"]);
        // A file excludes only itself (and files are not destinations anyway).
        let uids: Vec<_> = move_destinations(&groups(), "f").into_iter().map(|d| d.uid).collect();
        assert_eq!(uids, ["a", "b", "c", "d"]);
    }

    #[test]
    fn filter_matches_path_and_respects_subfolder_toggle() {
        let all = move_destinations(&groups(), "f");
        // The search runs on the whole path, so the folder nested under "Year One" matches too.
        assert_eq!(filter_destinations(&all, "year", true).len(), 2);
        assert_eq!(filter_destinations(&all, "year", false).len(), 0);
        let top: Vec<_> = filter_destinations(&all, "", false).iter().map(|d| d.uid.as_str()).collect();
        assert_eq!(top, ["a", "d"]);
    }
}
