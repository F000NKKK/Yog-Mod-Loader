//! Driving a screen in unit tests — no game, no graphics.
//!
//! ```ignore
//! let mut h = TestHost::<View>::new(CounterVm::default());
//! h.open();
//! h.click_id("add");
//! assert_eq!(h.vm().count, 1);
//! assert_eq!(h.text("label").as_deref(), Some("Clicked 1 times"));
//! ```
//!
//! Every input is followed by a frame, like in the game.

use yog_api::ui::event::keys;
use yog_api::ui::widget::WidgetKind;
use yog_api::ui::{LayoutNode, Rect, StateFlags, UiRoot, Widget};

use crate::screens::host::{Host, LOG_CLOSE, find_path};
use crate::{ScreenHandle, View};

/// A screen hosted for tests.
pub struct TestHost<W: View> {
    host: Host<W>,
}

/// The GUI size (w, h) a new [`TestHost`] starts at.
const DEFAULT_SIZE: (f32, f32) = (427.0, 240.0);

impl<W: View> TestHost<W> {
    /// The screen with `vm`, sized 427 × 240 (a common GUI size) — not
    /// opened yet (see [`Self::open`]).
    pub fn new(vm: W::Vm) -> Self {
        let mut host = Host::new(vm, ScreenHandle::new(W::META), false);
        host.size = DEFAULT_SIZE;
        Self { host }
    }

    /// Changes the GUI size.
    pub fn size(&mut self, w: f32, h: f32) -> &mut Self {
        self.host.size = (w, h);
        self
    }

    /// Opens the screen (`on_open`) and draws a frame.
    pub fn open(&mut self) -> &mut Self {
        let (w, h) = self.host.size;
        self.event(&format!("open:{w}:{h}"));
        self
    }

    /// Closes the screen (`on_close`).
    pub fn close(&mut self) -> &mut Self {
        self.event("close");
        self
    }

    /// Draws a frame (queued updates, bindings, layout).
    pub fn frame(&mut self) -> &mut Self {
        let (w, h) = self.host.size;
        self.host.frame(w, h);
        self
    }

    /// Sends a raw host event (`click:X:Y`, `key:K:M`, ...), then a frame;
    /// returns whether the screen consumed it.
    pub fn event(&mut self, raw: &str) -> bool {
        let used = self.host.on_event(raw);
        self.frame();
        used
    }

    /// Clicks (and releases) the middle of the widget at content `path`.
    pub fn click_path(&mut self, path: &[u16]) -> bool {
        let Some(r) = self.host.content_node(path).map(|n| n.rect) else {
            panic!("yog-kit test: nothing laid out at {path:?}");
        };
        let (x, y) = (r.x + r.w / 2.0, r.y + r.h / 2.0);
        self.move_to(x, y);
        let used = self.event(&format!("click:{x}:{y}"));
        self.event(&format!("release:{x}:{y}"));
        used
    }

    /// Clicks the widget tagged `id`.
    pub fn click_id(&mut self, id: &str) -> bool {
        let path = self.path(id);
        self.click_path(&path)
    }

    /// Moves the cursor.
    pub fn move_to(&mut self, x: f32, y: f32) -> &mut Self {
        self.event(&format!("move:{x}:{y}"));
        self
    }

    /// A mouse drag: the button goes down at the first point, moves through
    /// the others and comes up at the last; returns whether the press was
    /// consumed.
    pub fn drag(&mut self, points: &[(f32, f32)]) -> bool {
        let Some((&(x0, y0), rest)) = points.split_first() else {
            return false;
        };
        self.move_to(x0, y0);
        let used = self.event(&format!("click:{x0}:{y0}"));
        for &(x, y) in rest {
            self.event(&format!("drag:{x}:{y}"));
        }
        let (x, y) = points[points.len() - 1];
        self.event(&format!("release:{x}:{y}"));
        used
    }

    /// Mouse wheel (positive = up).
    pub fn scroll(&mut self, dy: f32) -> bool {
        self.event(&format!("scroll:{dy}"))
    }

    /// A key press (GLFW code, see `yog_api::ui::event::keys`).
    pub fn key(&mut self, key: i32) -> bool {
        self.key_mods(key, 0)
    }

    pub fn key_mods(&mut self, key: i32, mods: i32) -> bool {
        self.event(&format!("key:{key}:{mods}"))
    }

    /// Types `text` into the focused text box.
    pub fn type_text(&mut self, text: &str) -> &mut Self {
        for c in text.chars() {
            self.event(&format!("char:{}", c as u32));
        }
        self
    }

    /// Clicks the text box `id` and types `text` at its end.
    pub fn type_into(&mut self, id: &str, text: &str) -> &mut Self {
        self.click_id(id);
        self.key(keys::END);
        self.type_text(text)
    }

    pub fn vm(&self) -> &W::Vm {
        &self.host.vm
    }

    /// The view model, for arranging state; the bindings update at the
    /// next frame.
    pub fn vm_mut(&mut self) -> &mut W::Vm {
        self.host.dirty = true;
        &mut self.host.vm
    }

    /// A handle like the one `register()` returns.
    pub fn handle(&self) -> ScreenHandle<W::Vm> {
        self.host.handle.clone()
    }

    /// The content tree.
    pub fn root(&self) -> &Widget {
        self.host.content()
    }

    /// The whole laid-out screen (content is child 0).
    pub fn ui(&self) -> &UiRoot {
        &self.host.ui
    }

    /// Content path of the widget tagged `id`; panics when there's none.
    pub fn path(&self, id: &str) -> Vec<u16> {
        find_path(self.root(), id)
            .unwrap_or_else(|| panic!("yog-kit test: no widget with id {id:?}"))
    }

    pub fn widget(&self, id: &str) -> Option<&Widget> {
        find_path(self.root(), id).and_then(|p| self.root().get_path(&p))
    }

    /// The text of a label, button or text box.
    pub fn text_at(&self, path: &[u16]) -> Option<String> {
        match &self.root().get_path(path)?.kind {
            WidgetKind::Label(t) | WidgetKind::Button(t) => Some(t.clone()),
            WidgetKind::TextBox(tb) => Some(tb.text.clone()),
            _ => None,
        }
    }

    /// The text of the widget tagged `id`.
    pub fn text(&self, id: &str) -> Option<String> {
        self.text_at(&find_path(self.root(), id)?)
    }

    /// The widget `id` is visible (it and every ancestor).
    pub fn is_visible(&self, id: &str) -> bool {
        let Some(p) = find_path(self.root(), id) else {
            return false;
        };
        (0..=p.len()).all(|n| self.root().get_path(&p[..n]).is_some_and(|w| w.visible))
    }

    pub fn state(&self, id: &str) -> StateFlags {
        self.widget(id).map_or(StateFlags::NONE, |w| w.state)
    }

    pub fn node(&self, id: &str) -> Option<&LayoutNode> {
        self.host.content_node(&find_path(self.root(), id)?)
    }

    pub fn rect(&self, id: &str) -> Option<Rect> {
        self.node(id).map(|n| n.rect)
    }

    /// Scroll offset of scroller `i`.
    pub fn scroll_offset(&self, i: usize) -> f32 {
        self.host.parts.scrollers[i].state.offset
    }

    /// Requests made so far: `close`, `navigate:<id>`, `back`.
    pub fn requests(&self) -> &[String] {
        &self.host.log
    }

    /// The screen asked to close.
    pub fn closed(&self) -> bool {
        self.host.log.iter().any(|r| r == LOG_CLOSE)
    }
}
