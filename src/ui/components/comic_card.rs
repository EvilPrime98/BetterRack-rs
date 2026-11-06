//! `ComicCard` (`components/comic-card/*`): `cover` (cover-only grid) and `detail` (cover + text).
//! Built as a plain function so the virtualized grid can call it for visible rows only.
//!

use std::time::Duration;

use gpui::{
    Animation, AnimationExt as _, AnyElement, App, Context, EventEmitter, InteractiveElement,
    IntoElement, MouseButton, ObjectFit, ParentElement, SharedString, StatefulInteractiveElement,
    Styled, StyledImage as _, deferred, div, ease_in_out, img, linear_color_stop, linear_gradient,
    px, rgb, rgba,
};

use crate::model::{ComicsType, LibraryEntry, MetaSource, json_text};
use crate::route::{Navigate, Route};
use crate::state::Stores;
use crate::state::identify::CardInfo;
use crate::state::thumbnails::Thumb;
use crate::ui::confirm::{self, ConfirmOptions};
use crate::ui::icons::{Icon, icon};
use crate::ui::motion;
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
        ComicsType::Cover => {
            let w = col_w.min(COVER_MAX_CARD_W) - PAD_COVER * 2.0;
            PAD_COVER * 2.0 + w * COVER_RATIO + 6.0 + 4.0 + 6.0 + 2.0 + 34.0 + 4.0
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
    let file_name = || {
        if kind == ComicsType::Cover {
            item.name.replace('_', " ")
        } else {
            item.name.clone()
        }
    };
    let Some(comic) = info.comic.as_ref().filter(|c| c.title.is_some()) else {
        return file_name();
    };
    if kind != ComicsType::Cover {
        return file_name();
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
    if out.is_empty() { file_name() } else { out }
}

fn read_bar(percent: f32) -> impl IntoElement {
    let pct = percent.clamp(0.0, 100.0);
    div().w_full().h(px(4.0)).children((pct > 0.0).then(|| {
        div()
            .size_full()
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
    }))
}

fn swallow_mouse_down(_: &gpui::MouseDownEvent, _: &mut gpui::Window, cx: &mut App) {
    cx.stop_propagation();
}

fn star_row(size: f32, color: gpui::Rgba) -> gpui::Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(STAR_GAP))
        .children((0..5).map(|_| icon(Icon::Star, px(size)).text_color(color)))
}

fn star_fill(size: f32, halves: u32) -> gpui::Div {
    let pitch = size + STAR_GAP;
    let clip = (halves / 2) as f32 * pitch + if halves % 2 == 1 { size / 2.0 } else { 0.0 };
    div()
        .absolute()
        .top_0()
        .left_0()
        .h_full()
        .w(px(clip))
        .overflow_hidden()
        .child(star_row(size, rgb(0xfefcf3)))
}

const STAR_GAP: f32 = 2.0;

fn rating(uid: &str, current: f32, size: f32, stores: &Stores) -> impl IntoElement {
    let pitch = size + STAR_GAP;
    let row_group = SharedString::from(format!("rating-{uid}"));
    let current_halves = (current * 2.0).round().clamp(0.0, 10.0) as u32;

    let row = div()
        .id(SharedString::from(format!("stars-{uid}")))
        .group(row_group.clone())
        .relative()
        .flex_none()
        .w(px(pitch * 5.0))
        .h(px(size))
        .child(star_row(size, rgb(0xa3a7ae)))
        .child(star_fill(size, current_halves).group_hover(row_group.clone(), |s| s.opacity(0.0)));
    row.child(
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .children((1..=10u32).map(|halves| {
                let value = halves as f32 / 2.0;
                let stores = stores.clone();
                let uid = uid.to_string();
                div()
                    .id(SharedString::from(format!("star-{uid}-{halves}")))
                    .group(SharedString::from(format!("rating-{uid}-{halves}")))
                    .relative()
                    .cursor_pointer()
                    .w(px(pitch / 2.0))
                    .h_full()
                    .child(
                        star_fill(size, halves)
                            .left(px(-((halves - 1) as f32) * pitch / 2.0))
                            .opacity(0.0)
                            .group_hover(
                                SharedString::from(format!("rating-{uid}-{halves}")),
                                |s| s.opacity(1.0),
                            ),
                    )
                    .on_mouse_down(MouseButton::Left, swallow_mouse_down)
                    .on_click(move |_, _, cx| {
                        // Clicking the current rating clears it.
                        let value = if current == value { 0.0 } else { value };
                        stores.comics.update(cx, |s, cx| {
                            s.set_rating(&uid, value, cx);
                            if value > 0.0 {
                                s.set_read(&uid, true, cx);
                            }
                        });
                    })
            })),
    )
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
        .on_mouse_down(MouseButton::Left, swallow_mouse_down)
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
    rating: f32,
    group: &SharedString,
    fresh: bool,
) -> impl IntoElement {
    let h = w * COVER_RATIO;
    let zoom = |child: AnyElement| {
        let d = div().absolute().top_0().left_0().size_full();
        let d = if show_badge_always {
            d
        } else {
            d.group_hover(group.clone(), |s| {
                s.top(gpui::relative(-0.015))
                    .left(gpui::relative(-0.015))
                    .w(gpui::relative(1.03))
                    .h(gpui::relative(1.03))
            })
        };
        d.child(child)
    };
    let frame = div()
        .relative()
        .w(px(w))
        .h(px(h))
        .overflow_hidden()
        .bg(rgb(0x232222));
    let frame = match thumb {
        Some(Thumb::Ready(image)) => {
            let image = img(image).size_full().object_fit(ObjectFit::Cover);
            if fresh {
                frame.child(zoom(
                    image
                        .with_animation(
                            SharedString::from(format!("thumb-in-{}", item.uid)),
                            motion::once(motion::BASE),
                            |el, t| el.opacity(t),
                        )
                        .into_any_element(),
                ))
            } else {
                frame.child(zoom(image.into_any_element()))
            }
        }
        Some(Thumb::Missing) => frame
            .flex()
            .items_center()
            .justify_center()
            .text_color(rgb(0x5c5c5c))
            .child(icon(Icon::BookOpen, px(36.0)).text_color(rgb(0x5c5c5c))),
        _ => {
            let band_w = w * 0.7;
            let edge = rgba(0xffffff00);
            let peak = rgba(0xffffff12);
            frame
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .h_full()
                        .w(px(band_w))
                        .bg(linear_gradient(
                            90.0,
                            linear_color_stop(edge, 0.0),
                            linear_color_stop(peak, 0.5),
                        ))
                        .with_animation(
                            SharedString::from(format!("thumb-sweep-{}", item.uid)),
                            Animation::new(Duration::from_millis(1800))
                                .repeat()
                                .with_easing(ease_in_out),
                            move |el, t| el.left(px(-band_w + t * (w + band_w))),
                        ),
                )
                .child(
                    icon(Icon::BookOpen, px((w * 0.2).clamp(18.0, 32.0))).text_color(rgb(0x5c5c5c)),
                )
        }
    };
    let dim = is_read.then(|| {
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .bg(rgba(0x00000052))
    });
    let check = is_read.then(|| {
        div()
            .absolute()
            .top(px(6.0))
            .left(px(6.0))
            .flex()
            .items_center()
            .justify_center()
            .size(px(20.0))
            .rounded_full()
            .bg(rgb(0x13a629))
            .child(icon(Icon::Check, px(12.0)).text_color(rgb(0xffffff)))
    });
    let rating_badge = (!show_badge_always && rating > 0.0).then(|| {
        div()
            .absolute()
            .left(px(6.0))
            .bottom(px(6.0))
            .flex()
            .items_center()
            .gap(px(3.0))
            .px(px(6.0))
            .py(px(2.0))
            .rounded_full()
            .bg(rgba(0x121212d9))
            .text_size(px(10.0))
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .text_color(theme::text())
            .group_hover(group.clone(), |s| s.opacity(0.0))
            .child(icon(Icon::Star, px(10.0)).text_color(rgb(0xfefcf3)))
            .child(format!("{rating}"))
    });

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
    let event = event.filter(|_| show_badge_always);
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
    frame
        .children(dim)
        .children(check)
        .children(badge)
        .children(rating_badge)
        .children(event_badge)
}

fn cover_overlay(
    item: &LibraryEntry,
    info: &CardInfo,
    is_read: bool,
    current_rating: f32,
    menu_open: bool,
    group: &SharedString,
    stores: &Stores,
) -> impl IntoElement {
    let uid = item.uid.clone();
    let overlay = div()
        .absolute()
        .left_0()
        .right_0()
        .bottom_0()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(6.0))
        .px(px(6.0))
        .pt(px(28.0))
        .pb(px(7.0))
        .bg(linear_gradient(
            180.0,
            linear_color_stop(rgba(0x00000000), 0.0),
            linear_color_stop(rgba(0x000000d0), 1.0),
        ));
    let overlay = if menu_open {
        overlay
    } else {
        overlay
            .opacity(0.0)
            .group_hover(group.clone(), |s| s.opacity(1.0))
    };
    overlay
        .child(
            div()
                .px(px(8.0))
                .py(px(3.0))
                .rounded_full()
                .bg(rgba(0x000000a6))
                .child(rating(&uid, current_rating, 18.0, stores)),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(5.0))
                .w_full()
                .child(action_button(
                    format!("read-{uid}").into(),
                    Icon::Check,
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
                    format!("identify-{uid}").into(),
                    Icon::Wand,
                    info.identified,
                    {
                        let uid = uid.clone();
                        move |cx| crate::ui::modals::open_identify(cx, uid.clone())
                    },
                ))
                .child(div().flex_1())
                .child(action_button(
                    format!("more-{uid}").into(),
                    Icon::Dots,
                    menu_open,
                    {
                        let (stores, uid) = (stores.clone(), uid.clone());
                        move |cx| set_card_menu(&stores, &uid, !menu_open, cx)
                    },
                )),
        )
}

fn set_card_menu(stores: &Stores, uid: &str, open: bool, cx: &mut App) {
    stores
        .prefs
        .update(cx, |p, cx| p.set_card_menu(uid, open, cx));
}

fn menu_item(
    id: SharedString,
    glyph: Icon,
    label: &'static str,
    danger: bool,
    on_click: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
    let color = if danger { rgb(0xf0707a) } else { rgb(0xd8d8d8) };
    div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(8.0))
        .px(px(10.0))
        .py(px(7.0))
        .rounded(px(4.0))
        .cursor_pointer()
        .text_size(px(13.0))
        .text_color(color)
        .hover(|s| s.bg(rgba(0xffffff0f)))
        .on_mouse_down(MouseButton::Left, swallow_mouse_down)
        .on_click(move |_, _, cx| on_click(cx))
        .child(icon(glyph, px(13.0)).text_color(color))
        .child(label)
}

fn cover_menu(item: &LibraryEntry, stores: &Stores) -> impl IntoElement {
    let uid = item.uid.clone();
    let name = item.name.clone();
    let close = {
        let (stores, uid) = (stores.clone(), uid.clone());
        std::rc::Rc::new(move |cx: &mut App| set_card_menu(&stores, &uid, false, cx))
    };
    let backdrop_close = close.clone();
    let then_close = |f: Box<dyn Fn(&mut App)>| {
        let close = close.clone();
        move |cx: &mut App| {
            close(cx);
            f(cx);
        }
    };
    div()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .child(
            deferred(
                div()
                    .absolute()
                    .top(px(-3000.0))
                    .left(px(-3000.0))
                    .w(px(8000.0))
                    .h(px(8000.0))
                    .occlude()
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| backdrop_close(cx)),
            )
            .with_priority(1),
        )
        .child(
            deferred(
                div()
                    .absolute()
                    .right(px(6.0))
                    .bottom(px(38.0))
                    .min_w(px(130.0))
                    .occlude()
                    .flex()
                    .flex_col()
                    .p(px(4.0))
                    .rounded(px(6.0))
                    .border_1()
                    .border_color(rgba(0xffffff24))
                    .bg(theme::bg_panel())
                    .shadow_lg()
                    .child(menu_item(
                        format!("refresh-{uid}").into(),
                        Icon::Refresh,
                        "Refresh",
                        false,
                        then_close(Box::new({
                            let (stores, uid) = (stores.clone(), uid.clone());
                            move |cx| refresh_comic(&stores, &uid, cx)
                        })),
                    ))
                    .child(menu_item(
                        format!("download-{uid}").into(),
                        Icon::Download,
                        "Download",
                        false,
                        then_close(Box::new({
                            let (stores, uid, name) = (stores.clone(), uid.clone(), name.clone());
                            move |cx| download_comic(&stores, &uid, &name, cx)
                        })),
                    ))
                    .child(menu_item(
                        format!("move-{uid}").into(),
                        Icon::Folder,
                        "Move",
                        false,
                        then_close(Box::new({
                            let uid = uid.clone();
                            move |cx| crate::ui::modals::open_move_file(cx, uid.clone())
                        })),
                    ))
                    .child(menu_item(
                        format!("delete-{uid}").into(),
                        Icon::Trash,
                        "Delete",
                        true,
                        then_close(Box::new({
                            let (stores, uid, name) = (stores.clone(), uid.clone(), name.clone());
                            move |cx| confirm_delete(&stores, &uid, &name, cx)
                        })),
                    )),
            )
            .with_priority(2),
        )
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
    fresh: bool,
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
    let actions = detail.then(|| {
        div()
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
                format!("download-{uid}").into(),
                Icon::Download,
                false,
                {
                    let (stores, uid, name) = (stores.clone(), uid.clone(), item.name.clone());
                    move |cx| download_comic(&stores, &uid, &name, cx)
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
            ))
    });

    let title = title_text(item, info, env.kind);
    let title_el = div()
        .id(SharedString::from(format!("title-{uid}")))
        .cursor_pointer()
        .w_full()
        .min_w_0()
        .text_size(px(if detail { 16.0 } else { 13.0 }))
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .line_height(px(if detail { 20.0 } else { 17.0 }))
        .line_clamp(2)
        .overflow_hidden()
        .text_ellipsis()
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
                                cache.rating,
                                &group,
                                fresh,
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
                            .flex_wrap()
                            .gap_x(px(16.0))
                            .gap_y(px(6.0))
                            .items_center()
                            .child(rating(&uid, cache.rating, 16.0, &env.stores))
                            .children(actions),
                    ),
            )
            .into_any_element();
    }

    let card_w = col_w.min(COVER_MAX_CARD_W);
    let inner_w = card_w - PAD_COVER * 2.0;
    let menu_open = env.stores.prefs.read(cx).card_menu.as_deref() == Some(uid.as_str());
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
                .rounded(px(10.0))
                .group_hover(group.clone(), |s| s.bg(rgba(0xffffff0b)))
                .child(
                    div()
                        .relative()
                        .child(
                            div()
                                .id(SharedString::from(format!("cover-{uid}")))
                                .cursor_pointer()
                                .on_click(open_cover)
                                .child(cover(
                                    item,
                                    info,
                                    thumb,
                                    inner_w,
                                    false,
                                    is_read,
                                    cache.rating,
                                    &group,
                                    fresh,
                                )),
                        )
                        .child(cover_overlay(
                            item,
                            info,
                            is_read,
                            cache.rating,
                            menu_open,
                            &group,
                            &env.stores,
                        ))
                        .children(menu_open.then(|| cover_menu(item, &env.stores))),
                )
                .child(read_bar(read_per))
                .child(div().w_full().min_w_0().mt(px(2.0)).child(title_el)),
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

fn download_comic(stores: &Stores, uid: &str, name: &str, cx: &mut App) {
    let Some(client) = stores.thumbs.read(cx).client.clone() else {
        return;
    };
    let (uid, name) = (uid.to_string(), name.to_string());
    cx.spawn(async move |cx| {
        let outcome = crate::runtime::run(async move {
            let Some(handle) = rfd::AsyncFileDialog::new()
                .set_file_name(&name)
                .save_file()
                .await
            else {
                return None;
            };
            let dest = handle.path().to_path_buf();
            Some(client.download_comic(&uid, &dest).await.map(|()| dest))
        })
        .await;
        cx.update(|cx| match outcome {
            Some(Ok(dest)) => crate::ui::toast::success(cx, format!("Saved {}", dest.display())),
            Some(Err(e)) => crate::ui::toast::error(cx, e.to_string()),
            None => {}
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
