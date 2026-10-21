//! The host's input: events routed to scrollers, text boxes, surfaces,
//! lists and the focus, clicks to actions.

use yog_api::ui::event::keys;
use yog_api::ui::widget::WidgetKind;
use yog_api::ui::{Action, Pointer, Rect, TextBoxEvent, UiEvent, layout, surface};

use super::{FALLBACK_PAGE, Host, LOG_BACK, WHEEL_STEP, with_root};
use crate::screen;
use crate::{View, ViewEvent, ViewModel};

impl<W: View> Host<W> {
    /// Handles one raw UI event; `true` = consumed.
    pub(crate) fn on_event(&mut self, raw: &str) -> bool {
        self.drain_queue();
        let ev = UiEvent::parse(raw);
        match ev {
            UiEvent::Open { w, h } => {
                self.size = (w, h);
                if self.opened {
                    self.close_vm();
                }
                self.open_vm();
                false
            }
            UiEvent::Close => {
                self.captured = None;
                self.leave_surface();
                if self.opened {
                    self.close_vm();
                }
                if self.live {
                    screen::screen_closed();
                }
                false
            }
            UiEvent::Move { x, y } => {
                self.mouse = Some((x, y));
                self.hover = self.hit(x, y);
                let under = self.surface_at(x, y);
                if under != self.hovered_surface {
                    self.leave_surface();
                    self.hovered_surface = under.clone();
                }
                under.is_some_and(|p| self.to_surface(&p, x, y, |x, y| Pointer::Move { x, y }))
            }
            UiEvent::Scroll { .. } => self.scroll(&ev),
            UiEvent::Click { x, y } => self.click(x, y, &ev),
            UiEvent::Drag { x, y } | UiEvent::Release { x, y } if self.captured.is_some() => {
                let release = matches!(ev, UiEvent::Release { .. });
                let path = if release {
                    self.pressed = None;
                    self.captured.take()
                } else {
                    self.captured.clone()
                };
                if let Some(path) = path {
                    self.to_surface(&path, x, y, |x, y| {
                        if release {
                            Pointer::Up { x, y }
                        } else {
                            Pointer::Drag { x, y }
                        }
                    });
                }
                true
            }
            UiEvent::Drag { .. } | UiEvent::Release { .. } => {
                if matches!(ev, UiEvent::Release { .. }) {
                    self.pressed = None;
                }
                let used = self.feed_scrollers(&ev, false);
                self.feed_text_boxes(&ev).is_some() | used
            }
            UiEvent::Key { key, mods } => self.key(key, mods, &ev),
            UiEvent::Char(_) => self.text_input(&ev),
            UiEvent::Other(_) => false,
        }
    }

    fn scroll(&mut self, ev: &UiEvent<'_>) -> bool {
        // A surface under the cursor gets the wheel first.
        if let (Some((x, y)), UiEvent::Scroll { dy }) = (self.mouse, ev)
            && let Some((s, r)) = self.surface_at(x, y).and_then(|p| self.surface_box(&p))
            && s.scroll(*dy, (r.w, r.h))
        {
            return true;
        }
        let under = self
            .mouse
            .and_then(|(x, y)| layout::deepest_path(&self.ui.layout_root, x, y))
            .filter(|p| p.first() == Some(&0))
            .map(|p| p[1..].to_vec());
        let target = under
            .and_then(|p| {
                self.parts
                    .scrollers
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| p.starts_with(&s.path))
                    .max_by_key(|(_, s)| s.path.len())
                    .map(|(i, _)| i)
            })
            .or_else(|| {
                self.focused_list()
                    .and_then(|l| l.scroller.map(usize::from))
            })
            .or_else(|| (!self.parts.scrollers.is_empty()).then_some(0));
        let Some(i) = target else {
            return false;
        };
        let s = &mut self.parts.scrollers[i];
        let Some(node) = layout::node_at(&self.ui.layout_root, &with_root(&s.path)) else {
            return false;
        };
        s.state.handle_event(ev, node, WHEEL_STEP);
        self.dirty = true;
        true
    }

    fn click(&mut self, x: f32, y: f32, ev: &UiEvent<'_>) -> bool {
        self.mouse = Some((x, y));
        self.hover = self.hit(x, y);
        if self.feed_scrollers(ev, true) {
            return true;
        }
        if let Some(i) = self.feed_text_boxes(ev) {
            self.focus = Some(self.parts.text_boxes[i].path.clone());
            self.focus_visible = false;
            return true;
        }
        if let Some(path) = self.surface_at(x, y)
            && self.to_surface(&path, x, y, |x, y| Pointer::Down { x, y })
        {
            self.captured = Some(path);
            self.focus = None;
            return true;
        }
        let Some(path) = self.hit(x, y) else {
            self.focus = None;
            return false;
        };
        self.pressed = Some(path.clone());
        self.focus = self
            .parts
            .lists
            .iter()
            .filter(|l| l.scroller.is_some() && path.starts_with(&l.path))
            .max_by_key(|l| l.path.len())
            .map(|l| l.path.clone());
        self.focus_visible = false;
        self.activate(&path);
        true
    }

    /// Feeds a pointer event to the scrollers (stopping at the first taker
    /// when `first`); true if any used it.
    fn feed_scrollers(&mut self, ev: &UiEvent<'_>, first: bool) -> bool {
        let root = &self.ui.layout_root;
        let mut used = false;
        for s in &mut self.parts.scrollers {
            let Some(node) = layout::node_at(root, &with_root(&s.path)) else {
                continue;
            };
            if s.state.handle_event(ev, node, WHEEL_STEP) {
                used = true;
                if first {
                    break;
                }
            }
        }
        self.dirty |= used;
        used
    }

    /// Feeds a pointer event to every text box; the last one that took it.
    fn feed_text_boxes(&mut self, ev: &UiEvent<'_>) -> Option<usize> {
        let root = &self.ui.layout_root;
        let mut taker = None;
        for (i, b) in self.parts.text_boxes.iter_mut().enumerate() {
            let Some(node) = layout::node_at(root, &with_root(&b.path)) else {
                continue;
            };
            if b.state.handle_event(ev, node).consumed() {
                taker = Some(i);
            }
        }
        taker
    }

    // ── surfaces ────────────────────────────────────────────────────────────

    /// Content path of the enabled surface under `(x, y)`: the deepest node
    /// there or its nearest surface ancestor.
    fn surface_at(&self, x: f32, y: f32) -> Option<Vec<u16>> {
        let p = layout::deepest_path(&self.ui.layout_root, x, y)?;
        if p.first() != Some(&0) {
            return None;
        }
        let mut p = p[1..].to_vec();
        loop {
            if let Some(w) = self.content().get_path(&p)
                && matches!(w.kind, WidgetKind::Surface(_))
                && w.enabled
                && w.visible
            {
                return Some(p);
            }
            p.pop()?;
        }
    }

    /// The surface at `path` and its content box (rect less padding).
    fn surface_box(&self, path: &[u16]) -> Option<(yog_sync::Arc<dyn yog_api::ui::Surface>, Rect)> {
        let w = self.content().get_path(path)?;
        let WidgetKind::Surface(id) = &w.kind else {
            return None;
        };
        let rect = self.node(path)?.rect.inset(w.effective_style().pad);
        Some((surface::get(id)?, rect))
    }

    /// Hands the surface at `path` a pointer event at screen `(x, y)`.
    fn to_surface(
        &self,
        path: &[u16],
        x: f32,
        y: f32,
        ev: impl FnOnce(f32, f32) -> Pointer,
    ) -> bool {
        match self.surface_box(path) {
            Some((s, r)) => s.pointer(ev(x - r.x, y - r.y), (r.w, r.h)),
            None => false,
        }
    }

    /// Tells the hovered surface the cursor left it.
    fn leave_surface(&mut self) {
        if let Some(path) = self.hovered_surface.take()
            && let Some((s, r)) = self.surface_box(&path)
        {
            s.pointer(Pointer::Leave, (r.w, r.h));
        }
    }

    /// Runs the action of the widget at `path`.
    fn activate(&mut self, path: &[u16]) {
        let Some(w) = self.content().get_path(path) else {
            return;
        };
        if !w.enabled || !w.visible {
            return;
        }
        if let Some(a) = w.action {
            if a.slot == Action::SELECT {
                self.reveal = Some((a.owner, a.index));
            }
            self.dispatch(ViewEvent::Action(a));
        }
    }

    fn focused_box(&self) -> Option<usize> {
        self.parts
            .text_boxes
            .iter()
            .position(|t| t.state.is_focused())
    }

    fn focused_list(&self) -> Option<&crate::ListPart> {
        let f = self.focus.as_ref()?;
        self.parts.lists.iter().find(|l| &l.path == f)
    }

    /// Feeds a key/char event to the focused text box.
    fn text_input(&mut self, ev: &UiEvent<'_>) -> bool {
        let Some(i) = self.focused_box() else {
            return false;
        };
        let b = &mut self.parts.text_boxes[i];
        let Some(node) = layout::node_at(&self.ui.layout_root, &with_root(&b.path)) else {
            return false;
        };
        let r = b.state.handle_event(ev, node);
        let text = || self.parts.text_boxes[i].state.text().to_owned();
        match r {
            TextBoxEvent::Changed => {
                let text = text();
                self.dispatch(ViewEvent::TextChanged {
                    slot: i as u16,
                    text,
                });
            }
            TextBoxEvent::Submitted => {
                let text = text();
                self.dispatch(ViewEvent::TextSubmitted {
                    slot: i as u16,
                    text,
                });
            }
            _ => {}
        }
        if !self.parts.text_boxes[i].state.is_focused() {
            self.focus = None;
        }
        r.consumed()
    }

    fn key(&mut self, key: i32, mods: i32, ev: &UiEvent<'_>) -> bool {
        if key == keys::TAB {
            self.cycle_focus(mods & keys::MOD_SHIFT != 0);
            return true;
        }
        if self.focused_box().is_some() {
            return self.text_input(ev);
        }
        match key {
            keys::ESCAPE => {
                if self.with_cx(|vm, _, cx| vm.on_back(cx)) {
                    return true;
                }
                if self.live && screen::go_back() {
                    self.log.push(LOG_BACK.into());
                    return true;
                }
                false
            }
            keys::ENTER | keys::KP_ENTER | keys::SPACE => {
                let Some(p) = self.focus.clone() else {
                    return false;
                };
                if self
                    .content()
                    .get_path(&p)
                    .is_some_and(|w| w.is_interactive())
                {
                    self.activate(&p);
                    return true;
                }
                false
            }
            keys::UP | keys::DOWN | keys::PAGE_UP | keys::PAGE_DOWN | keys::HOME | keys::END => {
                self.list_key(key)
            }
            _ => false,
        }
    }

    /// Up/Down/PgUp/PgDn/Home/End in the focused list (else the first).
    fn list_key(&mut self, key: i32) -> bool {
        let list = self
            .focused_list()
            .or_else(|| self.parts.lists.iter().find(|l| l.scroller.is_some()));
        let Some(list) = list.cloned() else {
            return false;
        };
        if list.len == 0 {
            return true;
        }
        let last = list.len - 1;
        let page = match (list.item_height, list.scroller) {
            (Some(ih), Some(s)) => {
                let view_h = self
                    .parts
                    .scrollers
                    .get(s as usize)
                    .and_then(|s| self.node(&s.path))
                    .map_or(0.0, |n| n.rect.h);
                ((view_h / ih.max(1.0)) as u32).max(1)
            }
            _ => FALLBACK_PAGE,
        };
        let new = match (key, list.selected) {
            (keys::HOME, _) => 0,
            (keys::END, _) => last,
            (_, None) => 0,
            (keys::UP, Some(i)) => i.saturating_sub(1),
            (keys::DOWN, Some(i)) => (i + 1).min(last),
            (keys::PAGE_UP, Some(i)) => i.saturating_sub(page),
            (_, Some(i)) => (i + page).min(last),
        };
        if list.selected != Some(new) {
            self.dispatch(ViewEvent::Select {
                list: list.owner,
                index: new,
            });
        }
        self.reveal = Some((list.owner, new));
        if self.focus.is_none() {
            self.focus = Some(list.path.clone());
        }
        true
    }

    pub(super) fn set_focus(&mut self, path: Option<Vec<u16>>, visible: bool) {
        for t in &mut self.parts.text_boxes {
            t.state.blur();
        }
        if let Some(p) = &path
            && let Some(t) = self.parts.text_boxes.iter_mut().find(|t| &t.path == p)
        {
            t.state.focus();
        }
        self.focus = path;
        self.focus_visible = visible;
    }

    /// Tab / Shift+Tab through the visible, enabled focusables.
    fn cycle_focus(&mut self, back: bool) {
        let n = self.parts.focusables.len();
        if n == 0 {
            return;
        }
        let cur = self
            .focus
            .as_ref()
            .and_then(|f| self.parts.focusables.iter().position(|p| p == f));
        for step in 1..=n {
            let i = match (cur, back) {
                (None, false) => step - 1,
                (None, true) => n - step,
                (Some(c), false) => (c + step) % n,
                (Some(c), true) => (c + n - step % n) % n,
            };
            let p = &self.parts.focusables[i];
            if self
                .node(p)
                .is_some_and(|n| n.visible && n.enabled && n.rect.w > 0.0)
            {
                let p = p.clone();
                self.set_focus(Some(p), true);
                return;
            }
        }
    }
}
