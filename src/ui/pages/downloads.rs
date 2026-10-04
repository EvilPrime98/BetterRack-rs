//! Downloads page.
//! Polls the server's job list while it is open (see `state/downloads.rs`) and lists each job
//! with its progress and Retry / Stop actions.

use gpui::{
    ClickEvent, Context, InteractiveElement, IntoElement, ParentElement, Render, SharedString,
    StatefulInteractiveElement, Styled, Task, Window, div, prelude::*, px, relative, rgb, rgba,
};

use crate::model::{JobState, JobStatus};
use crate::state::Stores;
use crate::state::downloads::{can_stop, detail_for, percent_for, state_label};
use crate::ui::components::items_grid::PAGE_PAD_X;
use crate::ui::confirm::{self, ConfirmOptions};
use crate::ui::pages::common::{header_bar, note, summary};
use crate::ui::theme;

pub struct DownloadsPage {
    stores: Stores,
    /// Polls while the page exists.
    _poll: Task<()>,
}

impl DownloadsPage {
    pub fn new(stores: Stores, cx: &mut Context<Self>) -> Self {
        cx.observe(&stores.downloads, |_, _, cx| cx.notify())
            .detach();
        let poll = stores.downloads.update(cx, |d, cx| d.start_polling(cx));
        Self {
            stores,
            _poll: poll,
        }
    }

    /// Stop asks first unless the user ticked "Don't ask again" on an earlier one.
    fn stop(&mut self, job: &JobStatus, cx: &mut Context<Self>) {
        let downloads = self.stores.downloads.clone();
        let id = job.job_id.clone();
        if !self.stores.prefs.read(cx).prefs.ask_stop_downloads {
            downloads.update(cx, |d, cx| d.stop(id, cx));
            return;
        }
        let prefs = self.stores.prefs.clone();
        let mut options = ConfirmOptions::new(
            "Stop this download?",
            format!(
                "\"{}\" will be stopped and removed from the list.",
                job.label
            ),
        )
        .labels("Stop download", "Cancel");
        options.dont_ask_again = true;
        confirm::ask(cx, options, move |answer, dont_ask, cx| {
            if answer != Some(true) {
                return;
            }
            if dont_ask {
                prefs.update(cx, |p, cx| p.update(cx, |p| p.ask_stop_downloads = false));
            }
            downloads.update(cx, |d, cx| d.stop(id, cx));
        });
    }
}

impl Render for DownloadsPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (jobs, error, loaded, retrying, stopping) = {
            let d = self.stores.downloads.read(cx);
            (
                d.jobs.clone(),
                d.error.clone(),
                d.loaded,
                d.retrying.clone(),
                d.stopping.clone(),
            )
        };

        let body = if jobs.is_empty() {
            let text = if !error.is_empty() {
                error
            } else if loaded {
                "No active downloads.".to_string()
            } else {
                "Loading downloads…".to_string()
            };
            note(text).into_any_element()
        } else {
            let rows = jobs.iter().map(|job| {
                let retry_id = job.job_id.clone();
                let stop_job = job.clone();
                job_row(
                    job,
                    retrying.contains(&job.job_id),
                    stopping.contains(&job.job_id),
                    cx.listener(move |this, _, _, cx| {
                        this.stores
                            .downloads
                            .update(cx, |d, cx| d.retry(retry_id.clone(), cx))
                    }),
                    cx.listener(move |this, _, _, cx| this.stop(&stop_job, cx)),
                )
            });
            div()
                .id("jobs-list")
                .flex()
                .flex_col()
                .gap(px(12.0))
                .size_full()
                .overflow_y_scroll()
                .children(rows)
                .into_any_element()
        };

        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .h_full()
            .child(header_bar(summary("Store", "Downloads"), div()))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .px(px(PAGE_PAD_X))
                    .pt(px(16.0))
                    .pb(px(20.0))
                    .child(body),
            )
    }
}

fn badge(state: JobState) -> impl IntoElement {
    let (text, border) = match state {
        JobState::Running => (rgb(0x8fdde5), rgba(0x34c3d180)),
        JobState::Queued => (theme::accent(), rgba(0xe8a33d80)),
        JobState::Error => (rgb(0xf2a1a1), rgba(0xdc5a5a80)),
        JobState::Done => (rgb(0xb7b7b7), theme::border_subtle()),
    };
    div()
        .flex_none()
        .px(px(8.0))
        .py(px(3.0))
        .rounded_full()
        .border_1()
        .border_color(border)
        .text_size(px(10.0))
        .font_weight(gpui::FontWeight::BOLD)
        .text_color(text)
        .child(state_label(state).to_uppercase())
}

/// The outlined red `Retry` / `Stop` buttons. Disabled ones dim and ignore clicks.
fn danger_button(
    id: SharedString,
    label: &'static str,
    disabled: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut gpui::App) + 'static,
) -> impl IntoElement {
    let b = div()
        .id(id)
        .flex()
        .items_center()
        .px(px(10.0))
        .py(px(5.0))
        .rounded(px(6.0))
        .border_1()
        .border_color(rgba(0xdc5a5a80))
        .text_size(px(11.0))
        .font_weight(gpui::FontWeight::BOLD)
        .text_color(rgb(0xf2a1a1))
        .child(label.to_uppercase());
    if disabled {
        b.opacity(0.5)
    } else {
        b.cursor_pointer()
            .hover(|s| s.bg(rgba(0xdc5a5a1f)))
            .on_click(on_click)
    }
}

fn job_row(
    job: &JobStatus,
    retrying: bool,
    stopping: bool,
    on_retry: impl Fn(&ClickEvent, &mut Window, &mut gpui::App) + 'static,
    on_stop: impl Fn(&ClickEvent, &mut Window, &mut gpui::App) + 'static,
) -> impl IntoElement {
    let percent = percent_for(job.progress.as_ref());
    let detail = detail_for(job.progress.as_ref());
    let failed = job.state == JobState::Error;

    div()
        .flex()
        .flex_col()
        .flex_none()
        .gap(px(8.0))
        .px(px(16.0))
        .py(px(14.0))
        .rounded(px(10.0))
        .border_1()
        .border_color(theme::border_subtle())
        .bg(theme::bg_canvas())
        .when(job.state == JobState::Done, |s| s.opacity(0.6))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(12.0))
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_size(px(13.0))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(rgb(0xededed))
                        .child(job.label.clone()),
                )
                .child(badge(job.state)),
        )
        .children(percent.map(|p| {
            div()
                .flex()
                .flex_col()
                .gap(px(8.0))
                .child(
                    div()
                        .w_full()
                        .h(px(8.0))
                        .overflow_hidden()
                        .rounded_full()
                        .border_1()
                        .border_color(theme::border_subtle())
                        .bg(theme::bg_app())
                        .child(
                            div()
                                .h_full()
                                .w(relative(p as f32 / 100.0))
                                .bg(theme::accent()),
                        ),
                )
                .child(
                    div()
                        .text_size(px(10.0))
                        .font_family(theme::FONT_MONO)
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(rgb(0x8fdde5))
                        .child(format!("{p}%")),
                )
        }))
        .when(!detail.is_empty(), |s| {
            s.child(
                div()
                    .truncate()
                    .text_size(px(12.0))
                    .text_color(if failed { rgb(0xf2a1a1) } else { rgb(0x9a9a9a) })
                    .child(detail),
            )
        })
        .when(failed, |s| {
            s.child(div().flex().child(danger_button(
                format!("retry-{}", job.job_id).into(),
                "Retry",
                retrying,
                on_retry,
            )))
        })
        .when(can_stop(job), |s| {
            s.child(div().flex().child(danger_button(
                format!("stop-{}", job.job_id).into(),
                "Stop",
                stopping,
                on_stop,
            )))
        })
}
