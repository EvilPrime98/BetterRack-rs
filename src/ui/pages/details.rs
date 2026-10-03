//! Details page (`pages/details-page.tsx` + `components/details-page/*`): the wiki record of one
//! comic — hero (cover, meta, credits, quotation), synopsis, stories, appearing, cover variants,
//! notes and trivia. Sections with nothing to show are left out, as in React.

use gpui::{
    AnyElement, Context, EventEmitter, InteractiveElement, IntoElement, ObjectFit, ParentElement,
    Render, SharedString, StatefulInteractiveElement, Styled, StyledImage as _, Window, div, img,
    px, rgb, rgba,
};

use crate::model::{WikiComic, json_text};
use crate::route::{GoBack, Navigate, Route};
use crate::runtime;
use crate::state::Stores;
use crate::state::thumbnails::Thumb;
use crate::ui::components::items_grid::PAGE_PAD_X;
use crate::ui::pages::common::{back_button, header_bar, note};
use crate::ui::theme;

const COVER_RATIO: f32 = 77.0 / 50.0;
/// Width asked from the wiki for the hero cover (2x the 220 px it is drawn at).
const COVER_SIZE: u32 = 440;

enum Load {
    Loading,
    Ready(Box<WikiComic>),
    Failed(String),
}

pub struct DetailsPage {
    stores: Stores,
    state: Load,
}

impl EventEmitter<Navigate> for DetailsPage {}
impl EventEmitter<GoBack> for DetailsPage {}

impl DetailsPage {
    pub fn new(page_id: String, source_wiki: Option<String>, stores: Stores, cx: &mut Context<Self>) -> Self {
        cx.observe(&stores.thumbs, |_, _, cx| cx.notify()).detach();
        let client = stores.library.read(cx).client.clone();
        let this = Self { stores, state: Load::Loading };
        let Some(client) = client else {
            return Self { state: Load::Failed("The server is not running.".into()), ..this };
        };
        cx.spawn(async move |this, cx| {
            let result = match source_wiki {
                // React refuses a details link without its wiki, too.
                None => Err("This comic could not be loaded.".to_string()),
                Some(wiki) => runtime::run(async move { client.wiki_comic(&page_id, &wiki, COVER_SIZE).await })
                    .await
                    .map_err(|e| e.to_string()),
            };
            this.update(cx, |p, cx| {
                p.state = match result {
                    Ok(comic) => {
                        p.request_covers(&comic, cx);
                        Load::Ready(Box::new(comic))
                    }
                    Err(message) => Load::Failed(message),
                };
                cx.notify();
            })
            .ok();
        })
        .detach();
        this
    }

    fn request_covers(&self, comic: &WikiComic, cx: &mut Context<Self>) {
        let urls: Vec<String> = std::iter::once(comic.cover())
            .chain(comic.cover_variants().into_iter().map(|v| v.image_url))
            .filter(|u| !u.is_empty())
            .collect();
        self.stores.thumbs.update(cx, |t, cx| {
            for url in &urls {
                t.ensure_external(url, cx);
            }
        });
    }

    fn picture(&self, url: &str, w: f32, cx: &Context<Self>) -> gpui::Div {
        let frame = div().flex_none().w(px(w)).h(px(w * COVER_RATIO)).rounded(px(6.0)).overflow_hidden().bg(rgb(0x232222));
        match self.stores.thumbs.read(cx).get(url) {
            Some(Thumb::Ready(image)) => frame.child(img(image.clone()).size_full().object_fit(ObjectFit::Cover)),
            _ => frame,
        }
    }

    fn hero(&self, comic: &WikiComic, cx: &mut Context<Self>) -> impl IntoElement {
        let event = comic.event();
        let cover = self.picture(&comic.cover(), 220.0, cx).relative().children((!event.is_empty()).then(|| {
            div()
                .absolute()
                .left_0()
                .right_0()
                .bottom_0()
                .px(px(8.0))
                .pt(px(16.0))
                .pb(px(8.0))
                .truncate()
                .text_center()
                .text_size(px(10.0))
                .font_weight(gpui::FontWeight::BOLD)
                .text_color(rgb(0xffd66b))
                .bg(rgba(0x000000b8))
                .child(event.to_uppercase())
        }));

        let volume = json_text(&comic.volume);
        let issue = json_text(&comic.issue);
        let released = comic.release_date.as_ref().map(|d| d.display()).unwrap_or_default();
        let meta = [
            ("Volume", volume),
            ("Issue", if issue.is_empty() { issue } else { format!("#{issue}") }),
            ("Released", released),
            ("Rating", comic.rating_label()),
        ];
        let meta_grid = div().flex().flex_wrap().gap_x(px(28.0)).gap_y(px(14.0)).children(
            meta.into_iter().filter(|(_, v)| !v.is_empty()).map(|(label, value)| {
                div()
                    .flex()
                    .flex_col()
                    .gap(px(3.0))
                    .child(small_caps(label, 9.0, theme::accent()))
                    .child(
                        div()
                            .text_size(px(13.0))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(rgb(0xf2f2f2))
                            .child(value),
                    )
            }),
        );

        let first_writer = comic.writers().first().cloned();
        let rows = comic.credit_rows();
        let credits = (!rows.is_empty()).then(|| {
            div()
                .flex()
                .flex_col()
                .gap(px(6.0))
                .pt(px(14.0))
                .border_t_1()
                .border_color(theme::border_subtle())
                .children(rows.into_iter().map(|(label, names)| {
                    let clickable = label == "Writer" && first_writer.is_some();
                    let row = div()
                        .id(SharedString::from(format!("credit-{label}")))
                        .flex()
                        .items_baseline()
                        .gap(px(8.0))
                        .text_size(px(12.0))
                        .child(div().flex_none().w(px(110.0)).child(small_caps(label, 9.0, rgb(0x8a8a8a))))
                        .child(div().min_w_0().text_color(rgb(0xd8d8d8)).child(names.join(", ")));
                    match (clickable, first_writer.clone()) {
                        (true, Some(writer)) => row
                            .cursor_pointer()
                            .rounded(px(3.0))
                            .hover(|s| s.bg(rgba(0xffffff0f)).text_color(theme::accent()))
                            .on_click(cx.listener(move |_, _, _, cx| {
                                cx.emit(Navigate(Route::Filtered { writer: writer.clone() }))
                            }))
                            .into_any_element(),
                        _ => row.into_any_element(),
                    }
                }))
        });

        let quotation = comic.quotation().map(|(quote, speaker)| {
            div()
                .flex()
                .flex_col()
                .gap(px(6.0))
                .mt(px(4.0))
                .px(px(16.0))
                .py(px(12.0))
                .border_l_2()
                .border_color(theme::accent())
                .bg(rgba(0xffffff08))
                .child(div().text_size(px(13.0)).italic().text_color(rgb(0xe6e6e6)).child(format!("“{quote}”")))
                .children((!speaker.is_empty()).then(|| {
                    div()
                        .text_size(px(11.0))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(rgb(0x9a9a9a))
                        .child(speaker)
                }))
        });

        div()
            .flex()
            .gap(px(32.0))
            .items_start()
            .child(cover)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .gap(px(18.0))
                    .child(meta_grid)
                    .children(credits)
                    .children(quotation),
            )
    }

    fn body(&self, comic: &WikiComic, cx: &mut Context<Self>) -> AnyElement {
        let mut sections: Vec<AnyElement> = vec![self.hero(comic, cx).into_any_element()];

        let synopsis = comic.synopsis();
        if !synopsis.is_empty() {
            sections.push(section(
                "Synopsis",
                div().text_size(px(13.0)).line_height(px(22.0)).text_color(rgb(0xcfcfcf)).child(synopsis),
            ));
        }
        for (title, items) in [("Stories", comic.story_titles())] {
            if !items.is_empty() {
                sections.push(section(title, bullets(items)));
            }
        }

        let groups = comic.appearing_groups();
        if !groups.is_empty() {
            sections.push(section(
                "Appearing",
                div().flex().flex_col().gap(px(16.0)).children(groups.into_iter().map(|(label, entries)| {
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(8.0))
                        .child(small_caps(label, 10.0, rgb(0x8a8a8a)))
                        .child(div().flex().flex_wrap().gap(px(8.0)).children(entries.into_iter().map(|e| {
                            let chip = div()
                                .px(px(10.0))
                                .py(px(5.0))
                                .rounded_full()
                                .border_1()
                                .border_color(theme::border_subtle())
                                .bg(theme::bg_panel())
                                .text_size(px(12.0))
                                .text_color(rgb(0xe0e0e0));
                            // The status note is a hover tooltip in React; show it inline instead.
                            if e.status_note.is_empty() {
                                chip.child(e.name)
                            } else {
                                chip.child(format!("{} {}", e.name, e.status_note))
                            }
                        })))
                })),
            ));
        }

        let variants = comic.cover_variants();
        if !variants.is_empty() {
            sections.push(section(
                "Cover Variants",
                div().flex().flex_wrap().gap(px(18.0)).children(variants.into_iter().map(|v| {
                    let label = if v.image_label.is_empty() { format!("Cover {}", v.cover_number) } else { v.image_label };
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(6.0))
                        .w(px(130.0))
                        .child(self.picture(&v.image_url, 130.0, cx))
                        .child(div().text_size(px(11.0)).font_weight(gpui::FontWeight::SEMIBOLD).text_color(rgb(0xe0e0e0)).child(label))
                        .children((!v.artists.is_empty()).then(|| {
                            div().text_size(px(10.0)).text_color(rgb(0x9a9a9a)).child(v.artists.join(", "))
                        }))
                })),
            ));
        }

        for (title, items) in [("Notes", comic.notes()), ("Trivia", comic.trivia())] {
            if !items.is_empty() {
                sections.push(section(title, bullets(items)));
            }
        }

        div()
            .flex()
            .flex_col()
            .gap(px(36.0))
            .w_full()
            .max_w(px(980.0))
            .mx_auto()
            .children(sections)
            .into_any_element()
    }
}

fn small_caps(text: &str, size: f32, color: gpui::Rgba) -> impl IntoElement {
    div().text_size(px(size)).font_weight(gpui::FontWeight::BOLD).text_color(color).child(text.to_uppercase())
}

fn section(title: &'static str, content: impl IntoElement) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(12.0))
        .child(
            div()
                .pb(px(10.0))
                .border_b_1()
                .border_color(theme::border_subtle())
                .text_size(px(13.0))
                .font_weight(gpui::FontWeight::BOLD)
                .text_color(rgb(0xf2f2f2))
                .child(title.to_uppercase()),
        )
        .child(content)
        .into_any_element()
}

fn bullets(items: Vec<String>) -> impl IntoElement {
    div().flex().flex_col().gap(px(8.0)).children(items.into_iter().map(|item| {
        div()
            .flex()
            .gap(px(8.0))
            .text_size(px(13.0))
            .line_height(px(20.0))
            .text_color(rgb(0xcfcfcf))
            .child(div().flex_none().child("•"))
            .child(div().min_w_0().child(item))
    }))
}

impl Render for DetailsPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (eyebrow, title) = match &self.state {
            Load::Ready(c) => {
                let issue = json_text(&c.issue);
                let volume = json_text(&c.volume);
                let eyebrow = format!(
                    "{}{}",
                    if volume.is_empty() { "Comic".to_string() } else { volume },
                    if issue.is_empty() { String::new() } else { format!(" #{issue}") }
                );
                (eyebrow, c.title.clone().filter(|t| !t.is_empty()).unwrap_or_else(|| "Comic details".into()))
            }
            Load::Loading => ("Comic".to_string(), "Loading…".to_string()),
            Load::Failed(_) => ("Comic".to_string(), "Comic details".to_string()),
        };

        let header = header_bar(
            div()
                .flex()
                .items_center()
                .gap(px(14.0))
                .min_w_0()
                .child(back_button("details-back", cx.listener(|_, _, _, cx| cx.emit(GoBack))))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .min_w_0()
                        .child(small_caps(&eyebrow, 10.0, theme::accent()))
                        .child(
                            div()
                                .truncate()
                                .text_size(px(18.0))
                                .font_weight(gpui::FontWeight::BOLD)
                                .text_color(rgb(0xf2f2f2))
                                .child(title),
                        ),
                ),
            div(),
        );

        let content: AnyElement = match &self.state {
            Load::Loading => note("Loading comic details…").into_any_element(),
            Load::Failed(message) => note(if message.is_empty() { "Failed to load comic details.".to_string() } else { message.clone() })
                .into_any_element(),
            Load::Ready(comic) => {
                let comic = comic.clone();
                self.body(&comic, cx)
            }
        };

        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .h_full()
            .child(header)
            .child(
                div()
                    .id("details-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px(px(PAGE_PAD_X))
                    .pt(px(24.0))
                    .pb(px(48.0))
                    .child(content),
            )
    }
}
