//! `ComicCard` (`components/comic-card/*`): `cover` (cover-only grid) and `detail` (cover + text).
//! Built as a plain function so the virtualized grid can call it for visible rows only.
//!
//! Note: the "board"/"bag" overlay that marks read comics is reduced to
//! a light cover outline.

use std::time::Duration;

use gpui::{
    Animation, AnimationExt as _, AnyElement, App, Context, EventEmitter, InteractiveElement,
    IntoElement, ObjectFit, ParentElement, SharedString, StatefulInteractiveElement, Styled,
    StyledImage as _, div, img, px, rgb, rgba,
};

use crate::model::{ComicsType, LibraryEntry, MetaSource, json_text};
use crate::route::{Navigate, Route};
use crate::state::Stores;
use crate::state::identify::CardInfo;
use crate::state::thumbnails::Thumb;
use crate::ui::confirm::{self, ConfirmOptions};
use crate::ui::icons::{Icon, icon};
use crate::ui::theme;

/// Cover aspect ratio is 50:77.
pub const COVER_RATIO: f32 = 77.0 / 50.0;
pub const COVER_MAX_CARD_W: f32 = 220.0;
pub const DETAIL_COVER_W: f32 = 180.0;
const PAD_COVER: f32 = 10.0;
const PAD_DETAIL: f32 = 14.0;

/// Pixel height of one grid row's card for the given layout/column width (the grid needs uniform
/// rows, so heights are computed instead of measured).
pub fn card_height(kind: ComicsType, col_w: f32, has_comics: bool) -> f32 {
    match kind {
        // Folder-only pages (the root, mostly) do not need a comic card's height.
        ComicsType::Detail if !has_comics => PAD_DETAIL * 2.0 + 76.0,
        // padding + cover + title(2 lines) + rating row + actions row
        ComicsType::Cover => {
            let w = col_w.min(COVER_MAX_CARD_W) - PAD_COVER * 2.0;
            PAD_COVER * 2.0 + w * COVER_RATIO + 8.0 + 34.0 + 8.0 + 16.0 + 8.0 + 24.0
        }
        ComicsType::Detail => PAD_DETAIL * 2.0 + DETAIL_COVER_W * COVER_RATIO + 8.0 + 6.0 + 4.0,
    }
}

fn display(s: &str) -> bool {
    !s.is_empty() && s != "undefined"
}

/// `ComicCardTitle`: in cover mode `"<series> #<issue> (<year>)"` when the comic is identified,
/// otherwise the file name.
fn title_text(item: &LibraryEntry, info: &CardInfo, kind: ComicsType) -> String {
    let Some(comic) = info.comic.as_ref().filter(|c| c.title.is_some()) else {
        return item.name.clone();
    };
    if kind != ComicsType::Cover {
        return item.name.clone();
    }
    let title = comic.title.as_deref().unwrap_or_default();
    let series = title.split("Vol").next().unwrap_or_default().trim();
    let issue = json_text(&comic.issue);
    let year = comic.release_date.as_ref().and_then(|d| d.year());
    let mut out = String::new();
    if display(series) {
        out.push_str(series);
    }
    if display(&issue) {
        out.push_str(&format!(" #{issue}"));
    }
    if let Some(y) = year {
        out.push_str(&format!(" ({y})"));
    }
    let out = out.trim().to_string();
    if out.is_empty() {
        item.name.clone()
    } else {
        out
    }
}

fn read_bar(percent: f32) -> impl IntoElement {
    let pct = percent.clamp(0.0, 100.0);
    div()
        .w_full()
        .h(px(4.0))
        .rounded(px(2.0))
        .overflow_hidden()
        .bg(rgb(0x605959))
        .child(
            div()
                .h_full()
                .w(gpui::relative(pct / 100.0))
                .bg(if pct >= 100.0 {
                    rgb(0x13a629)
                } else {
                    theme::accent()
                }),
        )
}

fn rating(uid: &str, current: f32, stores: &Stores) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(2.0))
        .children((1..=5).map(|n| {
            let on = current >= n as f32;
            let stores = stores.clone();
            let uid = uid.to_string();
            div()
                .id(SharedString::from(format!("star-{uid}-{n}")))
                .cursor_pointer()
                .text_color(if on { rgb(0xfefcf3) } else { rgb(0x717479) })
                .on_click(move |_, _, cx| {
                    // Clicking the current rating clears it.
                    let value = if current == n as f32 { 0.0 } else { n as f32 };
                    stores
                        .comics
                        .update(cx, |s, cx| s.set_rating(&uid, value, cx));
                })
                .child(icon(Icon::Star, px(15.0)).text_color(if on {
                    rgb(0xfefcf3)
                } else {
                    rgb(0x717479)
                }))
        }))
}

fn action_button(
    id: SharedString,
    glyph: Icon,
    hint_active: bool,
    on_click: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .size(px(24.0))
        .rounded_full()
        .border_1()
        .border_color(if hint_active {
            theme::accent()
        } else {
            rgba(0xffffff1f).into()
        })
        .bg(rgba(0x121212bf))
        .text_color(if hint_active {
            theme::accent()
        } else {
            theme::text()
        })
        .cursor_pointer()
        .hover(|s| s.bg(rgba(0x34c3d12e)).text_color(theme::accent()))
        .on_click(move |_, _, cx| on_click(cx))
        .child(icon(glyph, px(13.0)).text_color(if hint_active {
            theme::accent()
        } else {
            theme::text()
        }))
}

fn info_row(label: &'static str, value: String) -> Option<impl IntoElement> {
    display(&value).then(|| {
        div()
            .flex()
            .items_baseline()
            .gap(px(6.0))
            .text_size(px(12.0))
            .child(
                div()
                    .w(px(56.0))
                    .flex_none()
                    .text_size(px(9.0))
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(theme::accent())
                    .child(label.to_uppercase()),
            )
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .text_color(rgb(0xcfcfcf))
                    .child(value),
            )
    })
}

fn cover(
    item: &LibraryEntry,
    info: &CardInfo,
    thumb: Option<Thumb>,
    w: f32,
    show_badge_always: bool,
    is_read: bool,
    group: &SharedString,
) -> impl IntoElement {
    let h = w * COVER_RATIO;
    let frame = div()
        .relative()
        .w(px(w))
        .h(px(h))
        .overflow_hidden()
        .bg(rgb(0x232222));
    let frame = match thumb {
        Some(Thumb::Ready(image)) => {
            frame.child(img(image).size_full().object_fit(ObjectFit::Cover))
        }
        Some(Thumb::Missing) => frame
            .flex()
            .items_center()
            .justify_center()
            .text_color(rgb(0x5c5c5c))
            .child(icon(Icon::BookOpen, px(36.0)).text_color(rgb(0x5c5c5c))),
        // Loading / not requested yet: a pulsing ring while the thumbnail is generated.
        _ => frame.flex().items_center().justify_center().child(
            div()
                .size(px((w * 0.2).clamp(18.0, 32.0)))
                .rounded_full()
                .border_2()
                .border_color(theme::accent())
                .with_animation(
                    SharedString::from(format!("thumb-loader-{}", item.uid)),
                    Animation::new(Duration::from_millis(900)).repeat(),
                    |el, t| el.opacity(0.25 + 0.75 * (t * std::f32::consts::TAU).sin().abs()),
                ),
        ),
    };
    let frame = if is_read {
        frame.border_1().border_color(rgba(0xffffff40))
    } else {
        frame
    };

    let issue = info
        .comic
        .as_ref()
        .map(|c| json_text(&c.issue))
        .unwrap_or_default();
    let badge = display(&issue).then(|| {
        let b = div()
            .absolute()
            .top(px(6.0))
            .right(px(6.0))
            .px(px(7.0))
            .py(px(2.0))
            .rounded_full()
            .bg(rgba(0x121212d9))
            .border_1()
            .border_color(rgba(0x34c3d173))
            .text_size(px(10.0))
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .font_family(theme::FONT_MONO)
            .text_color(theme::text())
            .child(format!("#{issue}"));
        if show_badge_always {
            b
        } else {
            b.opacity(0.0)
                .group_hover(group.clone(), |s| s.opacity(1.0))
        }
    });
    let event = info
        .comic
        .as_ref()
        .and_then(|c| c.extra.get("event"))
        .and_then(|v| v.as_str())
        .map(str::to_owned);
    let event_badge = event.filter(|e| !e.is_empty()).map(|e| {
        let b = div()
            .absolute()
            .left_0()
            .right_0()
            .bottom_0()
            .px(px(8.0))
            .pt(px(16.0))
            .pb(px(6.0))
            .text_size(px(9.0))
            .font_weight(gpui::FontWeight::BOLD)
            .text_color(rgb(0xffd66b))
            .bg(rgba(0x000000b3))
            .text_center()
            .truncate()
            .child(e.to_uppercase());
        if show_badge_always {
            b
        } else {
            b.opacity(0.0)
                .group_hover(group.clone(), |s| s.opacity(1.0))
        }
    });
    frame.children(badge).children(event_badge)
}

pub struct CardEnv {
    pub stores: Stores,
    pub kind: ComicsType,
}

/// One comic card of `col_w` px wide cell.
pub fn comic_card<V: EventEmitter<Navigate> + 'static>(
    item: &LibraryEntry,
    info: &CardInfo,
    thumb: Option<Thumb>,
    cache: crate::model::ComicCache,
    env: &CardEnv,
    col_w: f32,
    cx: &mut Context<V>,
) -> AnyElement {
    let uid = item.uid.clone();
    let group: SharedString = format!("card-{uid}").into();
    let detail = env.kind == ComicsType::Detail;
    let read_per = if cache.read { 100.0 } else { cache.read_per };
    let is_read = cache.read;
    let open = {
        let uid = uid.clone();
        cx.listener(move |_, _, _, cx| cx.emit(Navigate(Route::Reader { uid: uid.clone() })))
    };

    let open_cover = {
        let uid = uid.clone();
        cx.listener(move |_, _, _, cx| cx.emit(Navigate(Route::Reader { uid: uid.clone() })))
    };

    let stores = env.stores.clone();
    let actions = div()
        .flex()
        .gap(px(5.0))
        .when_hover_only(detail, &group)
        .child(action_button(
            format!("identify-{uid}").into(),
            Icon::Wand,
            info.identified,
            {
                let uid = uid.clone();
                move |cx| crate::ui::modals::open_identify(cx, uid.clone())
            },
        ))
        .child(action_button(
            format!("read-{uid}").into(),
            Icon::BookOpen,
            is_read,
            {
                let (stores, uid) = (stores.clone(), uid.clone());
                move |cx| {
                    stores
                        .comics
                        .update(cx, |s, cx| s.set_read(&uid, !is_read, cx))
                }
            },
        ))
        .child(action_button(
            format!("refresh-{uid}").into(),
            Icon::Refresh,
            false,
            {
                let (stores, uid) = (stores.clone(), uid.clone());
                move |cx| refresh_comic(&stores, &uid, cx)
            },
        ))
        .child(action_button(
            format!("move-{uid}").into(),
            Icon::Folder,
            false,
            {
                let uid = uid.clone();
                move |cx| crate::ui::modals::open_move_file(cx, uid.clone())
            },
        ))
        .child(action_button(
            format!("delete-{uid}").into(),
            Icon::Trash,
            false,
            {
                let (stores, uid, name) = (stores.clone(), uid.clone(), item.name.clone());
                move |cx| confirm_delete(&stores, &uid, &name, cx)
            },
        ));

    let title = title_text(item, info, env.kind);
    let title_el = div()
        .id(SharedString::from(format!("title-{uid}")))
        .cursor_pointer()
        .text_size(px(if detail { 16.0 } else { 13.0 }))
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .line_height(px(if detail { 20.0 } else { 17.0 }))
        .line_clamp(2)
        .text_color(theme::text())
        .when_text_center(!detail)
        .hover(|s| s.text_color(theme::accent()))
        .on_click(open)
        .child(title);

    if detail {
        let comic = info.comic.as_ref();
        let rows = div()
            .flex()
            .flex_col()
            .gap(px(3.0))
            .mt(px(8.0))
            .pt(px(8.0))
            .border_t_1()
            .border_color(rgba(0xffffff14))
            .flex_1()
            .child(if info.loading {
                div()
                    .text_size(px(11.0))
                    .text_color(rgb(0x8f8f8f))
                    .child("Loading…")
                    .into_any_element()
            } else if comic.is_none() {
                div()
                    .text_size(px(11.0))
                    .text_color(rgb(0x8f8f8f))
                    .child("No information found.")
                    .into_any_element()
            } else {
                let c = comic.unwrap();
                let details = c.page_id().map(|page_id| {
                    let source_wiki = c.source_wiki();
                    div()
                        .id(SharedString::from(format!("details-{uid}")))
                        .mt(px(4.0))
                        .text_size(px(11.0))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(theme::accent())
                        .cursor_pointer()
                        .hover(|s| s.opacity(0.8))
                        .on_click(cx.listener(move |_, _, _, cx| {
                            cx.emit(Navigate(Route::Details {
                                page_id: page_id.clone(),
                                source_wiki: source_wiki.clone(),
                            }))
                        }))
                        .child("View more")
                });
                let source = info.meta_source.map(|m| match m {
                    MetaSource::Wiki => "From wiki",
                    MetaSource::Comicinfo => "From ComicInfo.xml",
                });
                div()
                    .flex()
                    .flex_col()
                    .gap(px(3.0))
                    .children(source.map(|s| {
                        div()
                            .text_size(px(9.0))
                            .text_color(rgb(0x8a8a8a))
                            .child(s.to_uppercase())
                    }))
                    .children(info_row("Comic", c.title.clone().unwrap_or_default()))
                    .children(info_row("Volume", json_text(&c.volume)))
                    .children(info_row("Issue", json_text(&c.issue)))
                    .children(info_row(
                        "Year",
                        c.release_date
                            .as_ref()
                            .and_then(|d| d.year())
                            .map(|y| y.to_string())
                            .unwrap_or_default(),
                    ))
                    .children(info_row("Writer", c.writers().join(", ")))
                    .children(info_row("Artist", c.artists().join(", ")))
                    .children(info_row(
                        "Released",
                        c.release_date
                            .as_ref()
                            .map(|d| d.display())
                            .unwrap_or_default(),
                    ))
                    .children(details)
                    .into_any_element()
            });

        return div()
            .id(SharedString::from(format!("comic-{uid}")))
            .group(group.clone())
            .flex()
            .items_center()
            .gap(px(16.0))
            .w(px(col_w))
            .p(px(PAD_DETAIL))
            .rounded(px(14.0))
            .border_1()
            .border_color(rgba(0xffffff12))
            .bg(rgba(0xffffff0b))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .w(px(DETAIL_COVER_W))
                    .flex_none()
                    .child(
                        div()
                            .id(SharedString::from(format!("cover-{uid}")))
                            .cursor_pointer()
                            .on_click(open_cover)
                            .child(cover(
                                item,
                                info,
                                thumb,
                                DETAIL_COVER_W,
                                true,
                                is_read,
                                &group,
                            )),
                    )
                    .child(read_bar(read_per)),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .self_stretch()
                    .child(title_el)
                    .child(rows)
                    .child(
                        div()
                            .flex()
                            .gap(px(16.0))
                            .items_center()
                            .child(rating(&uid, cache.rating, &env.stores))
                            .child(actions),
                    ),
            )
            .into_any_element();
    }

    let card_w = col_w.min(COVER_MAX_CARD_W);
    let inner_w = card_w - PAD_COVER * 2.0;
    div()
        .w(px(col_w))
        .flex()
        .justify_center()
        .child(
            div()
                .id(SharedString::from(format!("comic-{uid}")))
                .group(group.clone())
                .flex()
                .flex_col()
                .items_center()
                .gap(px(6.0))
                .w(px(card_w))
                .p(px(PAD_COVER))
                .child(
                    div()
                        .id(SharedString::from(format!("cover-{uid}")))
                        .cursor_pointer()
                        .on_click(open_cover)
                        .child(cover(item, info, thumb, inner_w, false, is_read, &group)),
                )
                .child(read_bar(read_per))
                .child(div().w_full().mt(px(2.0)).child(title_el))
                .child(rating(&uid, cache.rating, &env.stores))
                .child(actions),
        )
        .into_any_element()
}

/// The refresh button: regenerate the cover thumbnail and re-identify the comic, like
/// `RefreshComicButton`. Either half may fail on its own.
pub(crate) fn refresh_comic(stores: &Stores, uid: &str, cx: &mut App) {
    let Some(client) = stores.thumbs.read(cx).client.clone() else {
        return;
    };
    let stores = stores.clone();
    let uid = uid.to_string();
    cx.spawn(async move |cx| {
        let id = uid.clone();
        let (thumb, ident) = crate::runtime::run(async move {
            tokio::join!(client.retry_thumbnail(&id), client.identify_reset(&id))
        })
        .await;
        cx.update(|cx| {
            match thumb {
                Ok(()) => stores.thumbs.update(cx, |t, cx| t.reload(&uid, cx)),
                Err(e) => crate::ui::toast::error(cx, e.to_string()),
            }
            if let Ok(r) = ident {
                let found =
                    r.identified == Some(true) && r.comic.is_some() && r.meta_source.is_some();
                let (comic, meta) = if found {
                    (r.comic, r.meta_source)
                } else {
                    (None, None)
                };
                stores
                    .identify
                    .update(cx, |s, cx| s.set_identified(&uid, comic, meta, cx));
            }
        });
    })
    .detach();
}

fn confirm_delete(stores: &Stores, uid: &str, name: &str, cx: &mut App) {
    let library = stores.library.clone();
    let uid = uid.to_string();
    confirm::ask(
        cx,
        ConfirmOptions::new(
            "Delete comic?",
            format!("\"{name}\" will be permanently deleted from disk."),
        )
        .labels("Delete", "Cancel"),
        move |answer, _, cx| {
            if answer == Some(true) {
                library.update(cx, |s, cx| s.delete_file(uid, cx));
            }
        },
    );
}

trait CardStyle: Sized {
    fn when_hover_only(self, detail: bool, group: &SharedString) -> Self;
    fn when_text_center(self, center: bool) -> Self;
}

impl CardStyle for gpui::Div {
    /// Cover-mode actions only appear while the card is hovered; detail mode always shows them.
    fn when_hover_only(self, detail: bool, group: &SharedString) -> Self {
        if detail {
            self
        } else {
            self.opacity(0.0)
                .group_hover(group.clone(), |s| s.opacity(1.0))
        }
    }

    fn when_text_center(self, center: bool) -> Self {
        if center { self.text_center() } else { self }
    }
}

impl CardStyle for gpui::Stateful<gpui::Div> {
    fn when_hover_only(self, _: bool, _: &SharedString) -> Self {
        self
    }

    fn when_text_center(self, center: bool) -> Self {
        if center { self.text_center() } else { self }
    }
}
