//! Driving a single-line text input (`widget::text_box()`) from the raw UI
//! events a Yog screen receives — `click:X:Y`, `drag:X:Y`, `release:X:Y`,
//! `key:KEY:MODS` and `char:CODEPOINT` — like [`ScrollState`](crate::ScrollState)
//! does for scroll views.
//!
//! ```ignore
//! static SEARCH: Mutex<TextBoxState> = Mutex::new(TextBoxState::new());
//!
//! // render: feed the text, caret and focus into the tree
//! SEARCH.lock().unwrap().apply(root.find_mut("search").unwrap());
//!
//! // events: with the last frame's layout node of "search"; return `true`
//! // from the `register_ui` handler when it's consumed, so Esc unfocuses
//! // the box instead of closing the screen
//! match SEARCH.lock().unwrap().handle(event, node) {
//!     TextBoxEvent::Ignored => { /* not for the box: buttons etc. */ }
//!     TextBoxEvent::Changed => { /* re-filter */ }
//!     TextBoxEvent::Submitted => { /* Enter */ }
//!     TextBoxEvent::Handled => {}
//! }
//! ```

use std::time::Instant;

use crate::layout::LayoutNode;
use crate::text;
use crate::widget::{Widget, WidgetKind};

/// GLFW key codes and modifier bits used here.
mod key {
    pub const A: i32 = 65;
    pub const ESCAPE: i32 = 256;
    pub const ENTER: i32 = 257;
    pub const BACKSPACE: i32 = 259;
    pub const DELETE: i32 = 261;
    pub const RIGHT: i32 = 262;
    pub const LEFT: i32 = 263;
    pub const HOME: i32 = 268;
    pub const END: i32 = 269;
    pub const KP_ENTER: i32 = 335;
    pub const MOD_SHIFT: i32 = 0x1;
    pub const MOD_CONTROL: i32 = 0x2;
    pub const MOD_SUPER: i32 = 0x8;
}

/// Caret blink half-period.
const BLINK_MS: u128 = 530;

/// What [`TextBoxState::handle`] did with an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextBoxEvent {
    /// Not for this box — pass it on (buttons, other boxes, ...). A click
    /// outside a focused box unfocuses it and is still `Ignored`.
    Ignored,
    /// Used by the box (focus, caret, selection, Esc unfocusing, ...).
    Handled,
    /// The text changed (typing, Backspace, Delete).
    Changed,
    /// Enter was pressed while focused.
    Submitted,
}

impl TextBoxEvent {
    /// The event was used by the box — the screen should consume it (for a
    /// `register_ui` handler: return `true`).
    pub fn consumed(self) -> bool {
        self != TextBoxEvent::Ignored
    }
}

/// Text, caret, selection and focus of one text box.
#[derive(Debug, Clone)]
pub struct TextBoxState {
    text: String,
    /// Caret, in characters.
    caret: usize,
    /// Selection anchor; `== caret` = no selection.
    anchor: usize,
    /// First visible character.
    first: usize,
    focused: bool,
    /// A click inside is being dragged (extends the selection).
    dragging: bool,
    /// Last input, for restarting the caret blink.
    last_input: Option<Instant>,
    // Learned from the widget in `apply`.
    max_length: usize,
    pad_l: f32,
    pad_r: f32,
    font_scale: f32,
    font: String,
    on_change: Option<String>,
    on_submit: Option<String>,
}

impl Default for TextBoxState {
    fn default() -> Self {
        Self::new()
    }
}

/// `(x, y)` from `"X:Y"`.
fn xy(rest: &str) -> Option<(f32, f32)> {
    let mut it = rest.split(':');
    Some((it.next()?.parse().ok()?, it.next()?.parse().ok()?))
}

/// Byte offset of character `i` in `s` (`s.len()` past the end).
fn byte_at(s: &str, i: usize) -> usize {
    s.char_indices().nth(i).map_or(s.len(), |(b, _)| b)
}

/// Whether `c` may be typed into a box — what vanilla allows in chat.
fn allowed(c: char) -> bool {
    c != '§' && c >= ' ' && c != '\u{7f}'
}

impl TextBoxState {
    pub const fn new() -> Self {
        Self {
            text: String::new(),
            caret: 0,
            anchor: 0,
            first: 0,
            focused: false,
            dragging: false,
            last_input: None,
            max_length: 0,
            pad_l: 4.0,
            pad_r: 4.0,
            font_scale: 1.0,
            font: String::new(),
            on_change: None,
            on_submit: None,
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    /// Replaces the text (cut to the max length), caret at the end.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        if self.max_length > 0 {
            let cut = byte_at(&self.text, self.max_length);
            self.text.truncate(cut);
        }
        self.caret = self.len();
        self.anchor = self.caret;
        self.first = 0;
    }

    /// Empties the box.
    pub fn clear(&mut self) {
        self.set_text("");
    }

    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// Gives the box keyboard focus, caret at the end.
    pub fn focus(&mut self) {
        self.focused = true;
        self.caret = self.len();
        self.anchor = self.caret;
        self.touch();
    }

    /// Takes keyboard focus away.
    pub fn blur(&mut self) {
        self.focused = false;
        self.dragging = false;
        self.anchor = self.caret;
    }

    /// Caret position, in characters.
    pub fn caret(&self) -> usize {
        self.caret
    }

    /// Selected characters `start..end` (empty when nothing is selected).
    pub fn selection(&self) -> (usize, usize) {
        (self.caret.min(self.anchor), self.caret.max(self.anchor))
    }

    /// Selects all the text.
    pub fn select_all(&mut self) {
        self.anchor = 0;
        self.caret = self.len();
    }

    /// The `on_change`/`on_submit` event name the widget declared for a
    /// [`TextBoxEvent::Changed`]/[`TextBoxEvent::Submitted`] (learned in
    /// [`Self::apply`]); `None` for other results or when it has none.
    pub fn event_name(&self, ev: TextBoxEvent) -> Option<&str> {
        match ev {
            TextBoxEvent::Changed => self.on_change.as_deref(),
            TextBoxEvent::Submitted => self.on_submit.as_deref(),
            _ => None,
        }
    }

    fn len(&self) -> usize {
        self.text.chars().count()
    }

    fn touch(&mut self) {
        self.last_input = Some(Instant::now());
    }

    /// Writes the state into the text box widget `w` before layout: text,
    /// caret, selection, scroll, focus. Also picks up the widget's max
    /// length, padding, font and event names. Does nothing to other kinds.
    pub fn apply(&mut self, w: &mut Widget) {
        let WidgetKind::TextBox(tb) = &mut w.kind else {
            return;
        };
        self.max_length = tb.max_length;
        self.on_change = tb.on_change.clone();
        self.on_submit = tb.on_submit.clone();
        self.pad_l = w.style.pad[3];
        self.pad_r = w.style.pad[1];
        self.font_scale = w.style.font_scale;
        self.font = w.style.font.clone();
        if self.max_length > 0 && self.len() > self.max_length {
            let cut = byte_at(&self.text, self.max_length);
            self.text.truncate(cut);
        }
        let n = self.len();
        self.caret = self.caret.min(n);
        self.anchor = self.anchor.min(n);
        self.first = self.first.min(self.caret);
        tb.text = self.text.clone();
        tb.caret = self.focused.then_some(self.caret);
        tb.caret_visible = self.focused
            && self
                .last_input
                .is_none_or(|t| (t.elapsed().as_millis() / BLINK_MS).is_multiple_of(2));
        tb.selection = if self.focused {
            self.selection()
        } else {
            (0, 0)
        };
        tb.first = self.first;
        w.focused = self.focused;
    }

    /// Width available to the text inside `node`.
    fn inner_w(&self, node: &LayoutNode) -> f32 {
        (node.rect.w - 2.0 - self.pad_l - self.pad_r).max(0.0)
    }

    fn char_w(&self, c: char) -> f32 {
        text::char_width_in(c, &self.font) * self.font_scale
    }

    /// Scrolls so the caret is visible in `node`.
    fn scroll_to_caret(&mut self, node: &LayoutNode) {
        self.first = text_box_first(
            &self.text,
            self.first,
            self.caret,
            self.inner_w(node),
            |c| self.char_w(c),
        );
    }

    /// The character boundary nearest to GUI x `mx` in `node`.
    fn index_at(&self, node: &LayoutNode, mx: f32) -> usize {
        let mut x = node.rect.x + 1.0 + self.pad_l;
        let mut i = self.first;
        for c in self.text.chars().skip(self.first) {
            let w = self.char_w(c);
            if mx < x + w / 2.0 {
                return i;
            }
            x += w;
            i += 1;
        }
        i
    }

    /// Deletes the selection; `true` if there was one.
    fn delete_selection(&mut self) -> bool {
        let (a, b) = self.selection();
        if a == b {
            return false;
        }
        let (ba, bb) = (byte_at(&self.text, a), byte_at(&self.text, b));
        self.text.replace_range(ba..bb, "");
        self.caret = a;
        self.anchor = a;
        true
    }

    fn insert(&mut self, c: char) -> bool {
        let had_selection = self.delete_selection();
        if self.max_length > 0 && self.len() >= self.max_length {
            return had_selection;
        }
        let at = byte_at(&self.text, self.caret);
        self.text.insert(at, c);
        self.caret += 1;
        self.anchor = self.caret;
        true
    }

    fn move_to(&mut self, i: usize, extend: bool) {
        self.caret = i.min(self.len());
        if !extend {
            self.anchor = self.caret;
        }
    }

    /// Applies one UI event; `node` is the box's laid-out node from the
    /// frame the user saw (`UiRoot::find_node`).
    pub fn handle(&mut self, event: &str, node: &LayoutNode) -> TextBoxEvent {
        let result = self.handle_inner(event, node);
        if result.consumed() {
            self.touch();
            self.scroll_to_caret(node);
        }
        result
    }

    fn handle_inner(&mut self, event: &str, node: &LayoutNode) -> TextBoxEvent {
        if let Some((mx, my)) = event.strip_prefix("click:").and_then(xy) {
            if node.enabled && node.rect.contains(mx, my) {
                self.focused = true;
                self.dragging = true;
                let i = self.index_at(node, mx);
                self.move_to(i, false);
                return TextBoxEvent::Handled;
            }
            if self.focused {
                self.blur();
            }
            return TextBoxEvent::Ignored;
        }
        if let Some((mx, _)) = event.strip_prefix("drag:").and_then(xy) {
            if !self.dragging {
                return TextBoxEvent::Ignored;
            }
            let i = self.index_at(node, mx);
            self.move_to(i, true);
            return TextBoxEvent::Handled;
        }
        if event.starts_with("release:") {
            return if std::mem::take(&mut self.dragging) {
                TextBoxEvent::Handled
            } else {
                TextBoxEvent::Ignored
            };
        }
        if !self.focused {
            return TextBoxEvent::Ignored;
        }
        if let Some(cp) = event.strip_prefix("char:") {
            let Some(c) = cp.parse::<u32>().ok().and_then(char::from_u32) else {
                return TextBoxEvent::Ignored;
            };
            if !allowed(c) {
                return TextBoxEvent::Handled;
            }
            return if self.insert(c) {
                TextBoxEvent::Changed
            } else {
                TextBoxEvent::Handled
            };
        }
        let Some(rest) = event.strip_prefix("key:") else {
            return TextBoxEvent::Ignored;
        };
        let mut it = rest.split(':');
        let Some(k) = it.next().and_then(|k| k.parse::<i32>().ok()) else {
            return TextBoxEvent::Ignored;
        };
        let mods = it.next().and_then(|m| m.parse::<i32>().ok()).unwrap_or(0);
        let shift = mods & key::MOD_SHIFT != 0;
        let ctrl = mods & (key::MOD_CONTROL | key::MOD_SUPER) != 0;
        match k {
            key::ESCAPE => self.blur(),
            key::ENTER | key::KP_ENTER => return TextBoxEvent::Submitted,
            key::BACKSPACE => {
                if !self.delete_selection() {
                    if self.caret == 0 {
                        return TextBoxEvent::Handled;
                    }
                    self.anchor = self.caret - 1;
                    self.delete_selection();
                }
                return TextBoxEvent::Changed;
            }
            key::DELETE => {
                if !self.delete_selection() {
                    if self.caret >= self.len() {
                        return TextBoxEvent::Handled;
                    }
                    self.anchor = self.caret + 1;
                    self.delete_selection();
                }
                return TextBoxEvent::Changed;
            }
            key::LEFT => {
                let (a, b) = self.selection();
                let to = if !shift && a != b {
                    a
                } else {
                    self.caret.saturating_sub(1)
                };
                self.move_to(to, shift);
            }
            key::RIGHT => {
                let (a, b) = self.selection();
                let to = if !shift && a != b { b } else { self.caret + 1 };
                self.move_to(to, shift);
            }
            key::HOME => self.move_to(0, shift),
            key::END => self.move_to(self.len(), shift),
            key::A if ctrl => self.select_all(),
            // Any other key while focused is the box's too (so it doesn't
            // trigger the screen's own shortcuts).
            _ => {}
        }
        TextBoxEvent::Handled
    }
}

/// The first visible character of a box `inner_w` wide whose first visible
/// character was `first`, adjusted so the caret is in view. Shared by
/// [`TextBoxState`] and the renderer.
pub(crate) fn text_box_first(
    text: &str,
    first: usize,
    caret: usize,
    inner_w: f32,
    char_w: impl Fn(char) -> f32,
) -> usize {
    let mut first = first.min(caret);
    let widths: Vec<f32> = text.chars().map(char_w).collect();
    let caret = caret.min(widths.len());
    while first < caret && widths[first..caret].iter().sum::<f32>() > inner_w {
        first += 1;
    }
    first
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{self, FlexDir};
    use crate::widget;

    fn node() -> LayoutNode {
        let root = widget::panel(FlexDir::Column)
            .child(widget::text_box("").id("box").w(50.0).at(10.0, 10.0));
        let n = layout::compute(&root, 300.0, 300.0);
        layout::find_node(&n, "box").unwrap().clone()
    }

    fn typed(s: &mut TextBoxState, n: &LayoutNode, text: &str) {
        for c in text.chars() {
            assert_eq!(s.handle(&format!("char:{}", c as u32), n), TextBoxEvent::Changed);
        }
    }

    #[test]
    fn focus_type_and_edit() {
        let n = node();
        assert_eq!((n.rect.x, n.rect.w, n.rect.h), (10.0, 50.0, 16.0));
        let mut s = TextBoxState::new();
        assert_eq!(s.handle("char:65", &n), TextBoxEvent::Ignored, "unfocused");
        assert_eq!(s.handle("click:20:15", &n), TextBoxEvent::Handled);
        assert!(s.is_focused());
        typed(&mut s, &n, "héllo");
        assert_eq!(s.text(), "héllo");
        assert_eq!(s.handle("key:259:0", &n), TextBoxEvent::Changed);
        assert_eq!(s.text(), "héll");
        assert_eq!(s.handle("key:268:0", &n), TextBoxEvent::Handled);
        assert_eq!(s.handle("key:261:0", &n), TextBoxEvent::Changed);
        assert_eq!(s.text(), "éll");
        assert_eq!(s.handle("key:262:1", &n), TextBoxEvent::Handled);
        assert_eq!(s.selection(), (0, 1));
        typed(&mut s, &n, "E");
        assert_eq!(s.text(), "Ell");
        assert_eq!(s.handle("key:65:2", &n), TextBoxEvent::Handled);
        assert_eq!(s.selection(), (0, 3));
        assert_eq!(s.handle("key:259:0", &n), TextBoxEvent::Changed);
        assert_eq!(s.text(), "");
        assert_eq!(s.handle("key:257:0", &n), TextBoxEvent::Submitted);
        // Esc unfocuses (consumed); the next Esc is the screen's.
        assert_eq!(s.handle("key:256:0", &n), TextBoxEvent::Handled);
        assert_eq!(s.handle("key:256:0", &n), TextBoxEvent::Ignored);
    }

    #[test]
    fn click_outside_unfocuses_and_passes_on() {
        let n = node();
        let mut s = TextBoxState::new();
        s.handle("click:20:15", &n);
        assert_eq!(s.handle("click:200:200", &n), TextBoxEvent::Ignored);
        assert!(!s.is_focused());
    }

    #[test]
    fn max_length_and_scrolling() {
        let n = node();
        let mut s = TextBoxState::new();
        let mut w = widget::text_box("").max_length(12);
        s.apply(&mut w);
        s.handle("click:20:15", &n);
        for _ in 0..20 {
            s.handle("char:120", &n); // 'x', 6 px each
        }
        assert_eq!(s.text().len(), 12);
        // 50 - 2 border - 8 padding = 40 px: six x's fit; caret at 12.
        assert_eq!(s.first, 6);
        s.handle("key:268:0", &n);
        assert_eq!(s.first, 0);
        // Clicking maps x to the nearest boundary.
        s.handle("click:26:15", &n); // 10 + 1 + 4 = 15 → (26-15)/6 ≈ 1.8 → 2
        assert_eq!(s.caret(), 2);
        s.apply(&mut w);
        match &w.kind {
            WidgetKind::TextBox(tb) => {
                assert_eq!(tb.caret, Some(2));
                assert_eq!(tb.text.len(), 12);
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn event_names_come_from_the_widget() {
        let mut s = TextBoxState::new();
        let mut w = widget::text_box("").on_change("search").on_submit("go");
        s.apply(&mut w);
        assert_eq!(s.event_name(TextBoxEvent::Changed), Some("search"));
        assert_eq!(s.event_name(TextBoxEvent::Submitted), Some("go"));
        assert_eq!(s.event_name(TextBoxEvent::Handled), None);
    }
}
