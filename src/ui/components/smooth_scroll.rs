//! Eased scrolling for notched mouse wheels, which otherwise move a list in fixed jumps.
//!
//! The owner keeps a [`SmoothScroll`], calls [`SmoothScroll::step`] once per render, and places a
//! [`wheel_capture`] over the scrollable area. Pixel-delta devices (touchpads) are not intercepted:
//! they are already smooth.

use std::time::Instant;

use gpui::{
    App, Context, DispatchPhase, IntoElement, ScrollDelta, ScrollWheelEvent, Styled, Window, canvas,
};

/// Pixels per wheel line for notched wheels.
const WHEEL_LINE_PX: f32 = 24.0;
/// Time constant of the easing: ~63 % of the remaining distance is covered per `TAU` seconds.
const TAU: f32 = 0.08;
/// Upper bound on queued distance, so a fast spin does not keep gliding for seconds.
const MAX_PENDING: f32 = 3000.0;

#[derive(Default)]
pub struct SmoothScroll {
    /// Pixels still to scroll (positive = down).
    pending: f32,
    last_frame: Option<Instant>,
}

impl SmoothScroll {
    pub fn push(&mut self, dy: f32) {
        self.pending = (self.pending + dy).clamp(-MAX_PENDING, MAX_PENDING);
    }

    /// Hand this frame's share of the pending distance to `scroll_by` (positive = down) and ask for
    /// another frame while any is left.
    pub fn step(&mut self, window: &mut Window, scroll_by: impl FnOnce(f32)) {
        if self.pending == 0.0 {
            self.last_frame = None;
            return;
        }
        let now = Instant::now();
        let dt = self
            .last_frame
            .map_or(1.0 / 60.0, |t| (now - t).as_secs_f32())
            .min(0.05);
        self.last_frame = Some(now);
        let step = if self.pending.abs() < 0.5 {
            self.pending
        } else {
            self.pending * (1.0 - (-dt / TAU).exp())
        };
        self.pending -= step;
        scroll_by(step);

        if self.pending != 0.0 {
            window.request_animation_frame();
        }
    }
}

/// Invisible overlay that turns notched wheel events over its bounds into `push(view, dy, cx)`
/// calls. The event is captured, so the list underneath never also jumps.
pub fn wheel_capture<V: 'static>(
    cx: &mut Context<V>,
    push: fn(&mut V, f32, &mut Context<V>),
) -> impl IntoElement {
    let me = cx.entity();
    canvas(
        |_, _, _| (),
        move |bounds, _, window: &mut Window, _: &mut App| {
            window.on_mouse_event(move |ev: &ScrollWheelEvent, phase, _, cx| {
                if phase != DispatchPhase::Capture
                    || ev.modifiers.secondary()
                    || !bounds.contains(&ev.position)
                {
                    return;
                }
                if let ScrollDelta::Lines(lines) = ev.delta {
                    me.update(cx, |t, cx| push(t, -lines.y * WHEEL_LINE_PX, cx));
                    cx.stop_propagation();
                }
            });
        },
    )
    .absolute()
    .size_full()
}
