//! Single-line text field. GPUI core has no text input, so this handles typing,
//! caret movement, selection (shift+arrows, select all, drag-free), copy/cut/paste, backspace/delete
//! and Enter/Escape.
//! Known gaps: no IME composition, no mouse-drag selection, caret is char-based (not grapheme-based).

use std::{cell::Cell, rc::Rc, time::Duration};

use gpui::{
    Animation, AnimationExt, AnyElement, App, Bounds, ClipboardItem, Context, EventEmitter,
    FocusHandle, Focusable, InteractiveElement, IntoElement, KeyDownEvent, MouseButton,
    MouseDownEvent, ParentElement, Pixels, Render, SharedString, Styled, TextRun, Window, canvas,
    div, pulsating_between, px,
};

use crate::ui::theme;

const CLICK_BIAS: Pixels = px(10.0);

pub enum TextInputEvent {
    Changed,
    Submit,
    Cancel,
}

pub struct TextInput {
    value: String,
    /// Caret position in chars.
    caret: usize,
    /// Other end of the selection (in chars); `None` or equal to `caret` means no selection.
    anchor: Option<usize>,
    placeholder: SharedString,
    focus: FocusHandle,
    text_bounds: Rc<Cell<Bounds<Pixels>>>,
}

impl EventEmitter<TextInputEvent> for TextInput {}

impl TextInput {
    pub fn new(placeholder: impl Into<SharedString>, cx: &mut Context<Self>) -> Self {
        Self {
            value: String::new(),
            caret: 0,
            anchor: None,
            placeholder: placeholder.into(),
            focus: cx.focus_handle(),
            text_bounds: Rc::default(),
        }
    }

    pub fn with_value(mut self, value: impl Into<String>) -> Self {
        self.value = value.into();
        self.caret = self.len();
        self
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    /// Replace the text (no `Changed` event: callers use this to load a value).
    pub fn set_value(&mut self, value: impl Into<String>, cx: &mut Context<Self>) {
        self.value = value.into();
        self.caret = self.len();
        self.anchor = None;
        cx.notify();
    }

    pub fn select_all(&mut self, cx: &mut Context<Self>) {
        self.anchor = Some(0);
        self.caret = self.len();
        cx.notify();
    }

    fn len(&self) -> usize {
        self.value.chars().count()
    }

    fn byte_index(&self, chars: usize) -> usize {
        self.value
            .char_indices()
            .nth(chars)
            .map(|(i, _)| i)
            .unwrap_or(self.value.len())
    }

    /// Selected range in chars, ordered.
    fn selection(&self) -> Option<(usize, usize)> {
        let anchor = self.anchor?;
        (anchor != self.caret).then(|| (anchor.min(self.caret), anchor.max(self.caret)))
    }

    fn selected_text(&self) -> Option<String> {
        let (from, to) = self.selection()?;
        Some(self.value[self.byte_index(from)..self.byte_index(to)].to_string())
    }

    /// Delete the selection, leaving the caret where it was. Returns whether anything was removed.
    fn delete_selection(&mut self) -> bool {
        let Some((from, to)) = self.selection() else {
            return false;
        };
        let (b_from, b_to) = (self.byte_index(from), self.byte_index(to));
        self.value.replace_range(b_from..b_to, "");
        self.caret = from;
        self.anchor = None;
        true
    }

    fn insert(&mut self, text: &str, cx: &mut Context<Self>) {
        // Single line: drop control characters and newlines.
        let clean: String = text.chars().filter(|c| !c.is_control()).collect();
        if clean.is_empty() {
            return;
        }
        self.delete_selection();
        let at = self.byte_index(self.caret);
        self.value.insert_str(at, &clean);
        self.caret += clean.chars().count();
        cx.emit(TextInputEvent::Changed);
    }

    /// Move the caret, extending the selection when `extend` is set.
    fn move_caret(&mut self, to: usize, extend: bool) {
        if extend {
            self.anchor.get_or_insert(self.caret);
        } else {
            self.anchor = None;
        }
        self.caret = to;
    }

    fn index_at(&self, x: Pixels, window: &Window) -> usize {
        if self.value.is_empty() {
            return 0;
        }
        let style = window.text_style();
        let run = TextRun {
            len: self.value.len(),
            font: style.font(),
            color: style.color,
            background_color: None,
            underline: None,
            strikethrough: None,
            letter_spacing: None,
        };
        let line = window.text_system().shape_line(
            SharedString::from(self.value.clone()),
            px(13.0),
            &[run],
            None,
        );
        let byte = line
            .closest_index_for_x(x - self.text_bounds.get().origin.x - CLICK_BIAS)
            .min(self.value.len());
        self.value[..byte].chars().count()
    }

    fn on_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let ks = &event.keystroke;
        let len = self.len();
        let command = ks.modifiers.control || ks.modifiers.platform;
        let shift = ks.modifiers.shift;
        match ks.key.as_str() {
            "enter" => cx.emit(TextInputEvent::Submit),
            "escape" => {
                cx.emit(TextInputEvent::Cancel);
                window.blur();
            }
            "backspace" => {
                if !self.delete_selection() {
                    if self.caret == 0 {
                        return cx.stop_propagation();
                    }
                    let (from, to) = (self.byte_index(self.caret - 1), self.byte_index(self.caret));
                    self.value.replace_range(from..to, "");
                    self.caret -= 1;
                }
                cx.emit(TextInputEvent::Changed);
            }
            "delete" => {
                if !self.delete_selection() {
                    if self.caret >= len {
                        return cx.stop_propagation();
                    }
                    let (from, to) = (self.byte_index(self.caret), self.byte_index(self.caret + 1));
                    self.value.replace_range(from..to, "");
                }
                cx.emit(TextInputEvent::Changed);
            }
            "left" => match self.selection() {
                Some((from, _)) if !shift => {
                    self.anchor = None;
                    self.caret = from;
                }
                _ => self.move_caret(self.caret.saturating_sub(1), shift),
            },
            "right" => match self.selection() {
                Some((_, to)) if !shift => {
                    self.anchor = None;
                    self.caret = to;
                }
                _ => self.move_caret((self.caret + 1).min(len), shift),
            },
            "home" => self.move_caret(0, shift),
            "end" => self.move_caret(len, shift),
            "a" if command => {
                self.anchor = Some(0);
                self.caret = len;
            }
            "c" if command => {
                if let Some(text) = self.selected_text() {
                    cx.write_to_clipboard(ClipboardItem::new_string(text));
                }
            }
            "x" if command => {
                if let Some(text) = self.selected_text() {
                    cx.write_to_clipboard(ClipboardItem::new_string(text));
                    self.delete_selection();
                    cx.emit(TextInputEvent::Changed);
                }
            }
            "v" if command => {
                if let Some(text) = cx.read_from_clipboard().and_then(|c| c.text()) {
                    self.insert(&text, cx);
                }
            }
            _ if command || ks.modifiers.alt => return,
            _ => match &ks.key_char {
                Some(ch) => self.insert(ch, cx),
                None => return,
            },
        }
        cx.stop_propagation();
        cx.notify();
    }
}

fn caret_bar(focused: bool) -> AnyElement {
    let slot = div().relative().w(px(0.0)).h(px(15.0));
    if !focused {
        return slot.into_any_element();
    }
    slot.child(
        div()
            .absolute()
            .top_0()
            .left_0()
            .w(px(1.0))
            .h_full()
            .bg(theme::accent())
            .with_animation(
                "caret-pulse",
                Animation::new(Duration::from_millis(1100))
                    .repeat()
                    .with_easing(pulsating_between(0.1, 1.0)),
                |bar, opacity| bar.opacity(opacity),
            ),
    )
    .into_any_element()
}

impl Focusable for TextInput {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for TextInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focused = self.focus.is_focused(window);
        let clear = gpui::rgba(0x00000000);
        let overlay = |before: String, selected: Option<String>, caret_first: bool| {
            let mut row = div()
                .absolute()
                .top_0()
                .left_0()
                .h_full()
                .flex()
                .items_center()
                .whitespace_nowrap()
                .text_color(clear)
                .child(div().child(before));
            if let Some(sel) = selected {
                let sel = div().bg(theme::accent_soft()).child(sel);
                row = if caret_first {
                    row.child(caret_bar(focused)).child(sel)
                } else {
                    row.child(sel).child(caret_bar(focused))
                };
            } else {
                row = row.child(caret_bar(focused));
            }
            row
        };

        let content = if self.value.is_empty() {
            div()
                .relative()
                .flex()
                .items_center()
                .child(overlay(String::new(), None, true))
                .child(
                    div()
                        .whitespace_nowrap()
                        .text_color(gpui::rgb(0x8a8a8a))
                        .child(self.placeholder.clone()),
                )
        } else {
            let (before, selected) = match self.selection() {
                Some((from, to)) => {
                    let (b_from, b_to) = (self.byte_index(from), self.byte_index(to));
                    (
                        self.value[..b_from].to_string(),
                        Some(self.value[b_from..b_to].to_string()),
                    )
                }
                None => (self.value[..self.byte_index(self.caret)].to_string(), None),
            };
            let caret_first = self.selection().is_some_and(|(from, _)| self.caret == from);
            div()
                .relative()
                .flex()
                .items_center()
                .child(div().whitespace_nowrap().child(self.value.clone()))
                .child(overlay(before, selected, caret_first))
                .child({
                    let bounds = self.text_bounds.clone();
                    canvas(move |b, _, _| bounds.set(b), |_, _, _, _| {})
                        .absolute()
                        .size_full()
                })
        };

        div()
            .id("text-input")
            .key_context("TextInput")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, e, window, cx| this.on_key(e, window, cx)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, e: &MouseDownEvent, window, cx| {
                    window.focus(&this.focus, cx);
                    this.anchor = None;
                    this.caret = this.index_at(e.position.x, window);
                    cx.notify();
                }),
            )
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .items_center()
            .overflow_hidden()
            .text_size(px(13.0))
            .text_color(theme::text())
            .cursor_text()
            .child(content)
    }
}
