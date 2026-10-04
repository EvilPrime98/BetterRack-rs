//! `SideBar`: group-by switch, nav links, search, refresh button and the
//! virtualized folder/series list.

use std::collections::HashSet;
use std::rc::Rc;

use gpui::{
    AppContext as _, Context, Entity, EventEmitter, InteractiveElement, IntoElement, ParentElement,
    Render, SharedString, StatefulInteractiveElement, Styled, UniformListScrollHandle, Window,
    deferred, div, prelude::*, px, rgb, rgba, uniform_list,
};

use crate::model::LibraryStructure;
use crate::route::{Navigate, Route};
use crate::state::Stores;
use crate::ui::components::button::{ButtonVariant, button};
use crate::ui::components::text_input::{TextInput, TextInputEvent};
use crate::ui::icons::{Icon, icon};
use crate::ui::text::capitalize_words;
use crate::ui::theme;

const ROW_H: f32 = 36.0;

#[derive(Clone)]
enum Row {
    Group {
        uid: String,
        name: String,
        expanded: bool,
    },
    Dir {
        uid: String,
        name: String,
        indent: bool,
    },
}

struct RowsMemo {
    key: (u64, u64, LibraryStructure),
    rows: Rc<Vec<Row>>,
}

pub struct Sidebar {
    stores: Stores,
    search: Entity<TextInput>,
    expanded: HashSet<String>,
    expanded_rev: u64,
    scroll: UniformListScrollHandle,
    memo: Option<RowsMemo>,
    group_menu_open: bool,
    /// Set by `AppRoot` so the current page's nav link is highlighted.
    pub current: Route,
}

impl EventEmitter<Navigate> for Sidebar {}

impl Sidebar {
    pub fn new(stores: Stores, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| TextInput::new("Search library…", cx));
        cx.subscribe(
            &search,
            |this, input, event: &TextInputEvent, cx| match event {
                // The current folder filters live as you type; Enter opens the Search page, which looks
                // through the whole library.
                TextInputEvent::Changed => {
                    let q = input.read(cx).value().trim().to_string();
                    this.stores
                        .library
                        .update(cx, |s, cx| s.set_search_query(q, cx));
                }
                TextInputEvent::Submit => {
                    let q = input.read(cx).value().trim().to_string();
                    this.stores
                        .library
                        .update(cx, |s, cx| s.set_search_query(q.clone(), cx));
                    if !q.is_empty() {
                        this.go(
                            Route::Library {
                                uid: None,
                                search: Some(q),
                            },
                            cx,
                        );
                    }
                }
                TextInputEvent::Cancel => {
                    input.update(cx, |i, cx| i.set_value("", cx));
                    this.stores
                        .library
                        .update(cx, |s, cx| s.set_search_query("", cx));
                }
            },
        )
        .detach();
        cx.observe(&stores.library, |_, _, cx| cx.notify()).detach();
        cx.observe(&stores.prefs, |_, _, cx| cx.notify()).detach();
        Self {
            stores,
            search,
            expanded: HashSet::new(),
            expanded_rev: 0,
            scroll: UniformListScrollHandle::new(),
            memo: None,
            group_menu_open: false,
            current: Route::home(),
        }
    }

    fn rows(&mut self, cx: &gpui::App) -> Rc<Vec<Row>> {
        let lib = self.stores.library.read(cx);
        let key = (lib.revision, self.expanded_rev, lib.structure);
        if let Some(m) = &self.memo {
            if m.key == key {
                return m.rows.clone();
            }
        }
        let mut rows = Vec::new();
        if lib.structure == LibraryStructure::Series {
            rows.extend(lib.groups.iter().map(|g| Row::Dir {
                uid: g.uid.clone(),
                name: g.name.clone(),
                indent: false,
            }));
        } else {
            for g in &lib.groups {
                let expanded = self.expanded.contains(&g.uid);
                rows.push(Row::Group {
                    uid: g.uid.clone(),
                    name: g.name.clone(),
                    expanded,
                });
                if expanded {
                    rows.extend(lib.items(true, Some(&g.uid)).into_iter().map(|e| Row::Dir {
                        uid: e.uid,
                        name: e.name,
                        indent: true,
                    }));
                }
            }
        }
        let rows = Rc::new(rows);
        self.memo = Some(RowsMemo {
            key,
            rows: rows.clone(),
        });
        rows
    }

    fn go(&mut self, route: Route, cx: &mut Context<Self>) {
        cx.emit(Navigate(route));
    }

    /// Opening a folder from the list clears the search box (`SideBarElement.onClick`).
    fn open_dir(&mut self, uid: String, cx: &mut Context<Self>) {
        self.search.update(cx, |i, cx| i.set_value("", cx));
        self.stores
            .library
            .update(cx, |s, cx| s.set_search_query("", cx));
        self.go(
            Route::Library {
                uid: Some(uid),
                search: None,
            },
            cx,
        );
    }

    fn nav_item(
        &self,
        id: &'static str,
        label: &'static str,
        glyph: Icon,
        route: Route,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let active = self.current == route;
        div()
            .id(id)
            .flex()
            .items_center()
            .gap(px(10.0))
            .px(px(10.0))
            .py(px(9.0))
            .rounded(px(6.0))
            .cursor_pointer()
            .text_size(px(14.0))
            .text_color(if active {
                theme::accent()
            } else {
                rgb(0xd8d8d8)
            })
            .hover(|s| s.bg(rgba(0xffffff0f)))
            .on_click(cx.listener(move |this, _, _, cx| this.go(route.clone(), cx)))
            .child(icon(glyph, px(16.0)).text_color(if active {
                theme::accent()
            } else {
                rgb(0xd8d8d8)
            }))
            .child(capitalize_words(label))
    }

    fn section(title: &'static str) -> gpui::Div {
        div()
            .flex()
            .flex_col()
            .gap(px(2.0))
            .p(px(10.0))
            .flex_none()
            .border_t_1()
            .border_color(rgba(0xffffff14))
            .child(
                div()
                    .px(px(10.0))
                    .py(px(4.0))
                    .text_size(px(11.0))
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(0x8a8a8a))
                    .child(title.to_uppercase()),
            )
    }
}

impl Render for Sidebar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let rows = self.rows(cx);
        let (structure, busy, progress, loading) = {
            let lib = self.stores.library.read(cx);
            (
                lib.structure,
                lib.refreshing || lib.identify_progress.is_some(),
                lib.identify_progress,
                lib.loading,
            )
        };
        let compact = self.stores.prefs.read(cx).prefs.sidebar_compact;

        let refresh_label: SharedString = match progress {
            Some((done, total)) if total > 0 => format!("Identifying {done}/{total}").into(),
            Some(_) => "Identifying…".into(),
            None => "Refresh Libraries".into(),
        };

        let folded = (!compact).then(|| {
            div()
                .flex()
                .flex_col()
                .flex_none()
                .child(Self::section("User").child(self.nav_item(
                    "nav-settings",
                    "Settings",
                    Icon::Gear,
                    Route::Settings,
                    cx,
                )))
                .child(
                    Self::section("Store")
                        .child(self.nav_item("nav-store", "Store", Icon::Shop, Route::Store, cx))
                        .child(self.nav_item(
                            "nav-downloads",
                            "Downloads",
                            Icon::Download,
                            Route::StoreDownloads,
                            cx,
                        )),
                )
                .child(
                    Self::section("Browse")
                        .child(self.nav_item(
                            "nav-recent",
                            "Recently added",
                            Icon::Bookmark,
                            Route::Recent,
                            cx,
                        ))
                        .child(self.nav_item(
                            "nav-reading",
                            "Keep reading",
                            Icon::BookOpen,
                            Route::Reading,
                            cx,
                        )),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .h(px(34.0))
                        .mx(px(16.0))
                        .mt(px(10.0))
                        .px(px(12.0))
                        .rounded_full()
                        .border_1()
                        .border_color(rgba(0xffffff24))
                        .child(icon(Icon::Search, px(16.0)).text_color(rgb(0x8a8a8a)))
                        .child(self.search.clone()),
                )
                .child(
                    div()
                        .id("refresh-library")
                        .flex()
                        .items_center()
                        .justify_center()
                        .h(px(34.0))
                        .mx(px(16.0))
                        .my(px(10.0))
                        .rounded_full()
                        .border_1()
                        .border_color(if busy {
                            rgba(0x34c3d173)
                        } else {
                            rgba(0xffffff24)
                        })
                        .text_size(px(11.0))
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_color(if busy { theme::accent() } else { rgb(0xc7c7c7) })
                        .when(!busy, |s| {
                            s.cursor_pointer()
                                .hover(|s| s.bg(rgba(0xffffff0f)).text_color(theme::text()))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.stores
                                        .library
                                        .update(cx, |s, cx| s.refresh_with_prompt(cx));
                                }))
                        })
                        .child(refresh_label.to_uppercase()),
                )
        });

        let list = if rows.is_empty() {
            div()
                .p(px(16.0))
                .text_size(px(13.0))
                .text_color(rgb(0x9a9a9a))
                .text_center()
                .child(if loading {
                    "Loading Library…"
                } else {
                    "No Folders Found"
                })
                .into_any_element()
        } else {
            let rows_for_list = rows.clone();
            uniform_list(
                "sidebar-list",
                rows.len(),
                cx.processor(move |_this, range: std::ops::Range<usize>, _w, cx| {
                    range
                        .map(|ix| match rows_for_list[ix].clone() {
                            Row::Group {
                                uid,
                                name,
                                expanded,
                            } => {
                                let toggle = uid.clone();
                                div()
                                    .id(SharedString::from(format!("group-{uid}")))
                                    .flex()
                                    .items_center()
                                    .gap(px(10.0))
                                    .h(px(ROW_H))
                                    .px(px(10.0))
                                    .rounded(px(6.0))
                                    .cursor_pointer()
                                    .text_size(px(14.0))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(rgb(0xd8d8d8))
                                    .hover(|s| s.bg(rgba(0xffffff0f)))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if !this.expanded.remove(&toggle) {
                                            this.expanded.insert(toggle.clone());
                                        }
                                        this.expanded_rev += 1;
                                        cx.notify();
                                    }))
                                    .child(icon(Icon::Folder, px(16.0)).text_color(rgb(0xd8d8d8)))
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .truncate()
                                            .child(capitalize_words(&name)),
                                    )
                                    .child(
                                        icon(
                                            if expanded {
                                                Icon::ChevronDown
                                            } else {
                                                Icon::ChevronRight
                                            },
                                            px(14.0),
                                        )
                                        .text_color(rgb(0xd8d8d8)),
                                    )
                            }
                            Row::Dir { uid, name, indent } => {
                                let open = uid.clone();
                                div()
                                    .id(SharedString::from(format!("dir-{uid}")))
                                    .flex()
                                    .items_center()
                                    .gap(px(10.0))
                                    .h(px(ROW_H))
                                    .pl(px(if indent { 24.0 } else { 10.0 }))
                                    .pr(px(10.0))
                                    .rounded(px(6.0))
                                    .cursor_pointer()
                                    .text_size(px(14.0))
                                    .text_color(rgb(0xd8d8d8))
                                    .hover(|s| s.bg(rgba(0xffffff0f)))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.open_dir(open.clone(), cx)
                                    }))
                                    .child(icon(Icon::Folder, px(16.0)).text_color(rgb(0xd8d8d8)))
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .truncate()
                                            .child(capitalize_words(&name)),
                                    )
                            }
                        })
                        .collect()
                }),
            )
            .track_scroll(&self.scroll)
            .size_full()
            .into_any_element()
        };

        let menu_open = self.group_menu_open;
        let structure_label = match structure {
            LibraryStructure::Folders => "Folders",
            LibraryStructure::Series => "Series",
        };

        div()
            .flex()
            .flex_col()
            .flex_none()
            .w(theme::sidebar_width())
            .h_full()
            .bg(theme::bg_panel())
            .border_r_1()
            .border_color(theme::border_subtle())
            .child(div().mx(px(16.0)).my(px(6.0)).child(button(
                "sidebar-compact",
                if compact { "Show More" } else { "Show Less" },
                ButtonVariant::Ghost,
                cx.listener(|this, _, _, cx| {
                    this.stores.prefs.update(cx, |p, cx| {
                        p.update(cx, |p| p.sidebar_compact = !p.sidebar_compact)
                    });
                }),
            )))
            .child(
                Self::section("Group by").child(
                    div()
                        .relative()
                        .child(
                            div()
                                .id("group-by")
                                .flex()
                                .items_center()
                                .justify_between()
                                .px(px(10.0))
                                .py(px(8.0))
                                .rounded(px(6.0))
                                .border_1()
                                .border_color(if menu_open {
                                    theme::accent()
                                } else {
                                    rgba(0xffffff1f)
                                })
                                .bg(rgba(0xffffff0a))
                                .text_size(px(14.0))
                                .text_color(rgb(0xd8d8d8))
                                .cursor_pointer()
                                .hover(|s| s.bg(rgba(0xffffff0f)))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.group_menu_open = !this.group_menu_open;
                                    cx.notify();
                                }))
                                .child(structure_label)
                                .child(icon(Icon::ChevronDown, px(12.0)).text_color(rgb(0xd8d8d8))),
                        )
                        .when(menu_open, |s| {
                            // Click-away backdrop: a bounds-based `on_mouse_down_out` on the wrapper
                            // would also fire for clicks on the menu (it lies outside the wrapper's
                            // bounds) and close it before the option's click lands.
                            s.child(
                                deferred(
                                    div()
                                        .absolute()
                                        .top(px(-3000.0))
                                        .left(px(-3000.0))
                                        .w(px(8000.0))
                                        .h(px(8000.0))
                                        .occlude()
                                        .on_mouse_down(
                                            gpui::MouseButton::Left,
                                            cx.listener(|this, _, _, cx| {
                                                this.group_menu_open = false;
                                                cx.notify();
                                            }),
                                        ),
                                )
                                .with_priority(1),
                            )
                            .child(
                                deferred(
                                    div()
                                        .absolute()
                                        .top(px(40.0))
                                        .left_0()
                                        .right_0()
                                        .occlude()
                                        .flex()
                                        .flex_col()
                                        .p(px(4.0))
                                        .rounded(px(6.0))
                                        .border_1()
                                        .border_color(rgba(0xffffff24))
                                        .bg(theme::bg_panel())
                                        .shadow_lg()
                                        .children(
                                            [
                                                (LibraryStructure::Folders, "Folders"),
                                                (LibraryStructure::Series, "Series"),
                                            ]
                                            .into_iter()
                                            .map(
                                                |(value, label)| {
                                                    let selected = value == structure;
                                                    div()
                                                        .id(label)
                                                        .px(px(10.0))
                                                        .py(px(7.0))
                                                        .rounded(px(4.0))
                                                        .cursor_pointer()
                                                        .text_size(px(14.0))
                                                        .text_color(if selected {
                                                            theme::accent()
                                                        } else {
                                                            rgb(0xd8d8d8)
                                                        })
                                                        .hover(|s| s.bg(rgba(0xffffff0f)))
                                                        .on_click(cx.listener(
                                                            move |this, _, _, cx| {
                                                                this.group_menu_open = false;
                                                                if this
                                                                    .stores
                                                                    .library
                                                                    .read(cx)
                                                                    .structure
                                                                    != value
                                                                {
                                                                    this.stores.prefs.update(
                                                                        cx,
                                                                        |p, cx| {
                                                                            p.update(cx, |p| {
                                                                                p.structure = value
                                                                            })
                                                                        },
                                                                    );
                                                                    this.stores.library.update(
                                                                        cx,
                                                                        |s, cx| {
                                                                            s.set_structure(
                                                                                value, cx,
                                                                            )
                                                                        },
                                                                    );
                                                                }
                                                                cx.notify();
                                                            },
                                                        ))
                                                        .child(label)
                                                },
                                            ),
                                        ),
                                )
                                .with_priority(2),
                            )
                        }),
                ),
            )
            .children(folded)
            .child(div().flex_1().min_h_0().p(px(10.0)).child(list))
    }
}
