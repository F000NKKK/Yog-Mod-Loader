//! The screen host: owns the view model, the view and the laid-out tree,
//! runs binding updates and routes input. Used by [`crate::Screen`] (the
//! game) and [`crate::testing::TestHost`] (tests) alike; nothing here needs
//! a graphics context.

use yog_api::ui::{FlexDir, LayoutNode, StateFlags, UiRoot, Units, Widget, layout, widget};

mod input;

use crate::screens::screen::{self, Request};
use crate::{Anchor, Cx, Parts, ScreenHandle, View, ViewEvent, ViewModel};

/// All of its parent, as a percent.
const FULL: f32 = 100.0;
/// Pixels per mouse-wheel notch.
const WHEEL_STEP: f32 = 16.0;
/// Rows PgUp/PgDn move in a list whose row height or viewport is unknown.
const FALLBACK_PAGE: u32 = 5;
/// [`Host::log`] entries for a close / back request.
pub(crate) const LOG_CLOSE: &str = "close";
const LOG_BACK: &str = "back";

/// A scroller's visible window during an update.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Viewport {
    /// Scroll offset (0 = top).
    pub offset: f32,
    /// Height of the scroller in the last layout (0 before the first).
    pub height: f32,
}

/// What a [`View::update`] changed, plus what it can ask of the host.
#[derive(Debug, Default)]
pub struct Update {
    pub(crate) relayout: bool,
    pub(crate) repaint: bool,
    pub(crate) viewports: Vec<Viewport>,
    pub(crate) text_sets: Vec<(u16, String)>,
    pub(crate) scroll_resets: Vec<u16>,
    pub(crate) lists: Vec<(u16, u32, Option<u32>)>,
}

impl Update {
    /// A standalone update (no scrollers) — for tests of generated views.
    pub fn new() -> Self {
        Self::default()
    }
    /// A change that may move or resize widgets.
    pub fn relayout(&mut self) {
        self.relayout = true;
    }
    /// A paint-only change.
    pub fn repaint(&mut self) {
        self.repaint = true;
    }
    pub fn needs_relayout(&self) -> bool {
        self.relayout
    }
    pub fn needs_repaint(&self) -> bool {
        self.repaint || self.relayout
    }
    /// The visible window of scroller `i`.
    pub fn viewport(&self, scroller: u16) -> Viewport {
        self.viewports
            .get(scroller as usize)
            .copied()
            .unwrap_or_default()
    }
    /// Sets text box `slot`'s text (kept if it's already that, so the caret
    /// doesn't jump while typing).
    pub fn set_text_box(&mut self, slot: u16, text: &str) {
        self.text_sets.push((slot, text.to_owned()));
    }
    /// Scrolls scroller `i` back to the top.
    pub fn reset_scroll(&mut self, scroller: u16) {
        self.scroll_resets.push(scroller);
        self.relayout();
    }
    /// Records list `owner`'s item count and selected index.
    pub fn list_info(&mut self, owner: u16, len: u32, selected: Option<u32>) {
        self.lists.push((owner, len, selected));
    }
}

/// Child-index path of the first widget tagged `id`.
pub(crate) fn find_path(w: &Widget, id: &str) -> Option<Vec<u16>> {
    if w.id.as_deref() == Some(id) {
        return Some(Vec::new());
    }
    w.children.iter().enumerate().find_map(|(i, c)| {
        find_path(c, id).map(|mut p| {
            p.insert(0, i as u16);
            p
        })
    })
}

fn with_root(path: &[u16]) -> Vec<u16> {
    let mut p = Vec::with_capacity(path.len() + 1);
    p.push(0);
    p.extend_from_slice(path);
    p
}

pub(crate) struct Host<W: View> {
    pub(crate) vm: W::Vm,
    view: W,
    /// Root = a full-screen panel whose only child is the view's content.
    pub(crate) ui: UiRoot,
    pub(crate) parts: Parts,
    pub(crate) size: (f32, f32),
    pub(crate) dirty: bool,
    /// Content-relative paths.
    hover: Option<Vec<u16>>,
    pressed: Option<Vec<u16>>,
    focus: Option<Vec<u16>>,
    /// The focus came from the keyboard (draw its outline).
    focus_visible: bool,
    outlined: Option<Vec<u16>>,
    mouse: Option<(f32, f32)>,
    /// The surface a button went down on (it gets the drags and the release).
    captured: Option<Vec<u16>>,
    /// The surface under the cursor (it gets `Move`s, then a `Leave`).
    hovered_surface: Option<Vec<u16>>,
    opened: bool,
    reveal: Option<(u16, u32)>,
    /// Viewports the last update saw.
    seen_viewports: Vec<Viewport>,
    pub(crate) handle: ScreenHandle<W::Vm>,
    /// What the screen asked for (close, navigate, ...), for tests.
    pub(crate) log: Vec<String>,
    /// Calls `yog_api` for close/navigation (off in tests).
    pub(crate) live: bool,
    /// Re-evaluate the bindings every frame: an inventory screen's slots
    /// change by drag-and-drop, which runs no command.
    pub(crate) every_frame: bool,
}

impl<W: View> Host<W> {
    pub(crate) fn new(vm: W::Vm, handle: ScreenHandle<W::Vm>, live: bool) -> Self {
        let (view, content, parts) = W::build();
        let screen = widget::panel(FlexDir::Column).child(content);
        Self {
            vm,
            view,
            ui: UiRoot::new(W::META.id, screen),
            parts,
            size: (W::META.w, W::META.h),
            dirty: true,
            hover: None,
            pressed: None,
            focus: None,
            focus_visible: false,
            outlined: None,
            mouse: None,
            captured: None,
            hovered_surface: None,
            opened: false,
            reveal: None,
            seen_viewports: Vec::new(),
            handle,
            log: Vec::new(),
            live,
            every_frame: false,
        }
    }

    pub(crate) fn content(&self) -> &Widget {
        &self.ui.root.children[0]
    }

    fn node(&self, path: &[u16]) -> Option<&LayoutNode> {
        layout::node_at(&self.ui.layout_root, &with_root(path))
    }

    #[cfg(any(test, feature = "testing"))]
    pub(crate) fn content_node(&self, path: &[u16]) -> Option<&LayoutNode> {
        self.node(path)
    }

    /// Content path of the clickable widget at `(x, y)`.
    fn hit(&self, x: f32, y: f32) -> Option<Vec<u16>> {
        let p = layout::hit_path(&self.ui.layout_root, x, y)?;
        (p.first() == Some(&0)).then(|| p[1..].to_vec())
    }

    // ── view model calls ────────────────────────────────────────────────────

    fn with_cx<R>(&mut self, f: impl FnOnce(&mut W::Vm, &mut W, &mut Cx<'_, W::Vm>) -> R) -> R {
        let mut reqs = Vec::new();
        let r = {
            let mut cx = Cx::new(&mut reqs, &self.handle);
            f(&mut self.vm, &mut self.view, &mut cx)
        };
        self.dirty = true;
        for req in reqs {
            self.request(req);
        }
        r
    }

    fn request(&mut self, req: Request) {
        let id = W::META.id;
        match req {
            Request::Close => {
                self.log.push(LOG_CLOSE.into());
                if self.live {
                    yog_api::close_ui(id);
                }
            }
            Request::Navigate(to) => {
                self.log.push(format!("navigate:{to}"));
                if self.live {
                    screen::navigate(id, &to);
                }
            }
            Request::Back => {
                self.log.push(LOG_BACK.into());
                if !self.live || !screen::go_back() {
                    self.request(Request::Close);
                }
            }
            Request::Focus(wid) => {
                if let Some(p) = find_path(self.content(), &wid) {
                    self.set_focus(Some(p), true);
                }
            }
            Request::Invalidate => self.dirty = true,
        }
    }

    pub(crate) fn dispatch(&mut self, ev: ViewEvent) {
        self.with_cx(|vm, view, cx| view.dispatch(vm, cx, ev));
    }

    fn open_vm(&mut self) {
        self.opened = true;
        self.reset_input();
        self.with_cx(|vm, _, cx| vm.on_open(cx));
    }

    fn close_vm(&mut self) {
        self.opened = false;
        self.reset_input();
        self.with_cx(|vm, _, cx| vm.on_close(cx));
    }

    fn reset_input(&mut self) {
        self.hover = None;
        self.pressed = None;
        self.focus = None;
        self.mouse = None;
        for t in &mut self.parts.text_boxes {
            t.state.blur();
        }
    }

    pub(crate) fn drain_queue(&mut self) {
        for job in self.handle.take_queue() {
            job(&mut self.vm);
            self.dirty = true;
        }
    }

    // ── frame ───────────────────────────────────────────────────────────────

    /// Brings the tree up to date and lays it out for a `sw` × `sh` GUI.
    pub(crate) fn frame(&mut self, sw: f32, sh: f32) {
        self.size = (sw, sh);
        self.drain_queue();
        self.dirty |= self.every_frame;
        // A keyboard reveal can scroll a virtualized list, which needs a
        // second update for the rows that came into view.
        for _ in 0..2 {
            if self.dirty {
                self.run_update();
            }
            self.apply_parts();
            self.apply_states();
            self.layout();
            if !self.do_reveal() && !self.viewports_changed() {
                break;
            }
        }
    }

    fn viewports(&self) -> Vec<Viewport> {
        self.parts
            .scrollers
            .iter()
            .map(|s| Viewport {
                offset: s.state.offset,
                height: self.node(&s.path).map_or(0.0, |n| n.rect.h),
            })
            .collect()
    }

    /// A virtualized list's scroller changed size or offset since the last
    /// update (the first layout; a `scroll_key` reset, which happens after
    /// the update read the old offset) — its window must be recomputed.
    fn viewports_changed(&mut self) -> bool {
        let now = self.viewports();
        let changed = self.parts.lists.iter().any(|l| {
            let (Some(_), Some(s)) = (l.item_height, l.scroller) else {
                return false;
            };
            let (a, b) = (self.seen_viewports.get(s as usize), now.get(s as usize));
            a.map(|v| (v.height, v.offset)) != b.map(|v| (v.height, v.offset))
        });
        if changed {
            self.dirty = true;
        }
        changed
    }

    fn run_update(&mut self) {
        let viewports = self.viewports();
        self.seen_viewports.clone_from(&viewports);
        let mut u = Update {
            viewports,
            ..Update::default()
        };
        let content = &mut self.ui.root.children[0];
        self.view.update(content, &self.vm, &mut u);
        self.vm.customize(content);
        for (slot, text) in u.text_sets {
            if let Some(t) = self.parts.text_boxes.get_mut(slot as usize)
                && t.state.text() != text
            {
                t.state.set_text(text);
            }
        }
        for s in u.scroll_resets {
            if let Some(s) = self.parts.scrollers.get_mut(s as usize) {
                s.state.reset();
            }
        }
        for (owner, len, selected) in u.lists {
            if let Some(l) = self.parts.lists.iter_mut().find(|l| l.owner == owner) {
                l.len = len;
                l.selected = selected;
            }
        }
        self.dirty = false;
    }

    fn apply_parts(&mut self) {
        let content = &mut self.ui.root.children[0];
        for s in &self.parts.scrollers {
            if let Some(w) = content.get_path_mut(&s.path) {
                w.style.scroll_y = s.state.offset;
            }
        }
        for t in &mut self.parts.text_boxes {
            if let Some(w) = content.get_path_mut(&t.path) {
                t.state.apply(w);
            }
        }
    }

    fn apply_states(&mut self) {
        let content = &mut self.ui.root.children[0];
        content.clear_state_deep(StateFlags::HOVER | StateFlags::PRESSED | StateFlags::FOCUSED);
        if let Some(p) = self.outlined.take()
            && let Some(w) = content.get_path_mut(&p)
        {
            w.focused = false;
        }
        for (path, flag) in [
            (&self.hover, StateFlags::HOVER),
            (&self.pressed, StateFlags::PRESSED),
            (&self.focus, StateFlags::FOCUSED),
        ] {
            if let Some(w) = path.as_deref().and_then(|p| content.get_path_mut(p)) {
                w.set_state_inherited(flag, true);
            }
        }
        if self.focus_visible
            && let Some(p) = &self.focus
            && let Some(w) = content.get_path_mut(p)
            && w.is_interactive()
        {
            w.focused = true;
            self.outlined = Some(p.clone());
        }
    }

    fn layout(&mut self) {
        let (sw, sh) = self.size;
        let m = W::META;
        let root = &mut self.ui.root;
        root.style.w = Units::Px(sw);
        root.style.h = Units::Px(sh);
        let c = &mut root.children[0];
        if m.anchor == Anchor::Fill {
            c.style.w = Units::Percent(FULL);
            c.style.h = Units::Percent(FULL);
            c.style.pos = Some((0.0, 0.0));
        } else {
            c.style.w = Units::Px(m.w);
            c.style.h = Units::Px(m.h);
            c.style.pos = Some(m.anchor.place(sw, sh, m.w, m.h));
        }
        self.ui.layout(sw, sh);
    }

    /// Scrolls a list so the item asked for is visible; `true` when the
    /// offset changed.
    fn do_reveal(&mut self) -> bool {
        let Some((owner, index)) = self.reveal.take() else {
            return false;
        };
        let Some(list) = self.parts.lists.iter().find(|l| l.owner == owner) else {
            return false;
        };
        let Some(si) = list.scroller else {
            return false;
        };
        let Some(scroller) = self.parts.scrollers.get(si as usize) else {
            return false;
        };
        let (Some(view), Some(items)) = (self.node(&scroller.path), self.node(&list.path)) else {
            return false;
        };
        let found = items.children.iter().find(|c| {
            c.action
                .is_some_and(|a| a.owner == owner && a.index == index)
        });
        let (top, h) = match (found, list.item_height) {
            (Some(n), _) => (n.rect.y - view.rect.y + view.scroll_y, n.rect.h),
            (None, Some(ih)) => {
                let w = self.content().get_path(&list.path);
                let (gap, pad) = w.map_or((0.0, 0.0), |w| (w.style.gap, w.style.pad[0]));
                (pad + index as f32 * (ih + gap), ih)
            }
            (None, None) => return false,
        };
        let (view_h, max) = (view.rect.h, view.max_scroll());
        let state = &mut self.parts.scrollers[si as usize].state;
        let before = state.offset;
        state.scroll_into_view(top, h, view_h);
        state.offset = state.offset.clamp(0.0, max.max(0.0));
        if (state.offset - before).abs() > f32::EPSILON {
            self.dirty = true;
            true
        } else {
            false
        }
    }

    /// The tooltip of what's under the cursor, drawn on top.
    pub(crate) fn draw_tooltip(&self, gfx: &yog_api::GfxContext) {
        if let Some((x, y)) = self.mouse
            && let Some(text) = self.ui.tooltip_at(x, y)
        {
            UiRoot::render_tooltip(gfx, text, x, y);
        }
    }
}
