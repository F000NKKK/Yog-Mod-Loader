//! Binding helpers used by generated `.yui` code: a [`Slot`] remembers a
//! binding's last value so widgets are only patched when it changes, and
//! the conversion functions turn view-model values into widget values with
//! readable compile errors.

use std::fmt::Display;
use std::hash::{Hash, Hasher};

use yog_api::ui::{StateFlags, Units, Widget};

/// The last value of one binding.
#[derive(Debug, Clone, Default)]
pub struct Slot<T>(Option<T>);

impl<T: PartialEq> Slot<T> {
    pub const fn new() -> Self {
        Slot(None)
    }
    /// Stores `v`; `true` when it differs from the previous value (or is
    /// the first).
    pub fn set(&mut self, v: T) -> bool {
        if self.0.as_ref() == Some(&v) {
            return false;
        }
        self.0 = Some(v);
        true
    }
    /// The current value. Panics before the first [`Self::set`].
    pub fn get(&self) -> &T {
        self.0.as_ref().expect("yog-kit: binding read before it was set")
    }
    /// Forgets the value, so the next `set` reports a change.
    pub fn reset(&mut self) {
        self.0 = None;
    }
}

/// A value shown as text by a `.yui` binding.
#[diagnostic::on_unimplemented(
    message = "`{Self}` can't be shown as text in a .yui binding",
    note = "bind a value that implements Display, or call a view-model method returning String"
)]
pub trait BindText {
    fn bind_text(&self) -> String;
}

impl<T: Display + ?Sized> BindText for T {
    fn bind_text(&self) -> String {
        self.to_string()
    }
}

/// A value used as a `0xAARRGGBB` colour by a `.yui` binding.
#[diagnostic::on_unimplemented(
    message = "`{Self}` can't be used as a colour in a .yui binding",
    note = "colours are u32 values 0xAARRGGBB (a .yres colour, or a view-model method returning u32)"
)]
pub trait BindColor {
    fn bind_color(&self) -> u32;
}

impl BindColor for u32 {
    fn bind_color(&self) -> u32 {
        *self
    }
}

impl<T: BindColor + ?Sized> BindColor for &T {
    fn bind_color(&self) -> u32 {
        (**self).bind_color()
    }
}

/// Text of a bound value.
pub fn text<T: BindText + ?Sized>(v: &T) -> String {
    v.bind_text()
}

/// Colour of a bound value.
pub fn color<T: BindColor>(v: T) -> u32 {
    v.bind_color()
}

/// A bound flag (`enabled`, `visible`, `checked`).
pub fn flag(v: impl Into<bool>) -> bool {
    v.into()
}

/// A bound size (`w`, `h`).
pub fn units(v: impl Into<Units>) -> Units {
    v.into()
}

/// A bound number (`gap`, `font_scale`, ...).
pub fn number(v: impl Into<f64>) -> f32 {
    v.into() as f32
}

/// A bound count (`count`); out-of-range values become 0.
pub fn count<T: TryInto<u32>>(v: T) -> u32 {
    v.try_into().unwrap_or(0)
}

/// Hash of a bound key (`key`, `scroll_key`) — stable within a process.
pub fn key<K: Hash + ?Sized>(k: &K) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    k.hash(&mut h);
    h.finish()
}

/// Whether the item with `key` is the selected one.
pub fn is_selected<S: PartialEq<K>, K: ?Sized>(selected: &Option<S>, key: &K) -> bool {
    selected.as_ref().is_some_and(|s| s == key)
}

/// Sets the placeholder of a text box; `false` for other kinds.
pub fn set_placeholder(w: &mut Widget, text: &str) -> bool {
    match &mut w.kind {
        yog_api::ui::widget::WidgetKind::TextBox(tb) => {
            tb.placeholder = text.to_owned();
            true
        }
        _ => false,
    }
}

/// Sets the item of an item slot / inventory slot; `false` for other kinds.
pub fn set_item(w: &mut Widget, item: &str) -> bool {
    use yog_api::ui::widget::WidgetKind;
    match &mut w.kind {
        WidgetKind::ItemSlot(id) | WidgetKind::InvSlot { item_id: id, .. } => {
            *id = item.to_owned();
            true
        }
        _ => false,
    }
}

/// Sets the stack count of an inventory slot; `false` for other kinds.
pub fn set_count(w: &mut Widget, n: u32) -> bool {
    match &mut w.kind {
        yog_api::ui::widget::WidgetKind::InvSlot { count, .. } => {
            *count = n;
            true
        }
        _ => false,
    }
}

/// Sets the image source of an image; `false` for other kinds.
pub fn set_source(w: &mut Widget, source: &str) -> bool {
    use yog_api::ui::widget::WidgetKind;
    match &mut w.kind {
        WidgetKind::Image(img) => {
            img.source = source.to_owned();
            true
        }
        WidgetKind::McImage { id, .. } => {
            *id = source.to_owned();
            true
        }
        _ => false,
    }
}

/// Sets or clears the CHECKED state (inherited by non-interactive children).
pub fn set_checked(w: &mut Widget, on: bool) {
    w.set_state_inherited(StateFlags::CHECKED, on);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_reports_changes_only() {
        let mut s = Slot::new();
        assert!(s.set(1));
        assert!(!s.set(1));
        assert!(s.set(2));
        assert_eq!(*s.get(), 2);
        s.reset();
        assert!(s.set(2));
    }

    #[test]
    fn conversions() {
        assert_eq!(text(&5), "5");
        assert_eq!(text("x"), "x");
        assert_eq!(color(0xFF00FF00u32), 0xFF00FF00);
        assert_eq!(color(&7u32), 7);
        assert_eq!(count(-1i32), 0);
        assert_eq!(count(3usize), 3);
        assert_eq!(key("a"), key(&"a".to_string()));
        let sel: Option<&str> = Some("b");
        assert!(is_selected(&sel, &"b".to_string()));
        assert!(!is_selected(&None::<&str>, &"b".to_string()));
    }
}
