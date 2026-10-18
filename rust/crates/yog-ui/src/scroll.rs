//! Driving a scrolling container (`widget::scroll_view()`) from the raw UI
//! events a Yog screen receives (`scroll:DY`, `click:X:Y`, `drag:X:Y`,
//! `release:X:Y`).
//!
//! ```ignore
//! static LIST: Mutex<ScrollState> = Mutex::new(ScrollState::new());
//!
//! // render: feed the offset into the tree
//! root.find_mut("list").unwrap().style.scroll_y = LIST.lock().unwrap().offset;
//!
//! // events: with the last frame's layout node of "list"
//! if LIST.lock().unwrap().handle(event, node, WHEEL_STEP) { return; }
//! ```

use crate::event::UiEvent;
use crate::layout::LayoutNode;

/// Pixels a scroll view moves per wheel notch, as the game's lists do.
pub const WHEEL_STEP: f32 = 24.0;

/// Scroll offset plus scrollbar-drag state of one scrolling container.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ScrollState {
    /// Offset in GUI pixels (0 = top) — set it as the widget's `scroll_y`.
    pub offset: f32,
    /// While the knob is dragged: cursor y minus the knob's top.
    grab: Option<f32>,
}

impl ScrollState {
    pub const fn new() -> Self {
        Self {
            offset: 0.0,
            grab: None,
        }
    }

    /// The scrollbar knob is being dragged.
    pub fn is_dragging(&self) -> bool {
        self.grab.is_some()
    }

    /// Back to the top, ending any drag.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Applies one UI event; `view` is the container's node from the frame
    /// the user saw (`UiRoot::find_node`). `wheel_step`: pixels per wheel
    /// notch. Returns `true` when the event was for the scrollbar/wheel —
    /// the caller shouldn't treat it as a click on the content.
    ///
    /// - `scroll:DY` — wheel; positive = up.
    /// - `click:X:Y` on the knob grabs it; on the track jumps there (knob
    ///   centred on the cursor) and keeps dragging.
    /// - `drag:X:Y` moves a grabbed knob; `release:X:Y` lets go.
    pub fn handle(&mut self, event: &str, view: &LayoutNode, wheel_step: f32) -> bool {
        self.handle_event(&UiEvent::parse(event), view, wheel_step)
    }

    /// [`Self::handle`] for a parsed event.
    pub fn handle_event(
        &mut self,
        event: &UiEvent<'_>,
        view: &LayoutNode,
        wheel_step: f32,
    ) -> bool {
        let max = view.max_scroll();
        match *event {
            UiEvent::Scroll { dy } => {
                self.offset = (self.offset.min(max) - dy * wheel_step).clamp(0.0, max);
                true
            }
            UiEvent::Click { x: mx, y: my } => {
                let Some((track, knob)) = view.scrollbar_rects() else {
                    return false;
                };
                if !track.contains(yog_gfx::Vec2::new(mx, my)) {
                    return false;
                }
                let grab = if knob.contains(yog_gfx::Vec2::new(mx, my)) {
                    my - knob.y
                } else {
                    knob.h / 2.0
                };
                self.grab = Some(grab);
                self.drag_to(my, view);
                true
            }
            UiEvent::Drag { y: my, .. } => {
                if self.grab.is_some() {
                    self.drag_to(my, view);
                    return true;
                }
                false
            }
            UiEvent::Release { .. } => self.grab.take().is_some(),
            _ => false,
        }
    }

    /// Scrolls the least amount that shows `top..top + h` (content
    /// coordinates, 0 = the content's top) in a view `view_h` tall.
    pub fn scroll_into_view(&mut self, top: f32, h: f32, view_h: f32) {
        if top < self.offset {
            self.offset = top.max(0.0);
        } else if top + h > self.offset + view_h {
            self.offset = (top + h - view_h).max(0.0);
        }
    }

    /// Moves the knob so its top is at `my - grab`.
    fn drag_to(&mut self, my: f32, view: &LayoutNode) {
        let (Some(grab), Some((track, knob))) = (self.grab, view.scrollbar_rects()) else {
            return;
        };
        let travel = (track.h - knob.h).max(1.0);
        let t = ((my - grab - track.y) / travel).clamp(0.0, 1.0);
        self.offset = t * view.max_scroll();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{self, FlexDir};
    use crate::widget;

    /// A 100px-tall scroll view holding ten 30px rows (content 300px + gaps).
    fn view(offset: f32) -> LayoutNode {
        let list = widget::scroll_view()
            .id("list")
            .w(100.0)
            .h(100.0)
            .gap(0.0)
            .scroll_y(offset)
            .children((0..10).map(|i| {
                widget::panel(FlexDir::Column)
                    .w(widget::Units::Percent(100.0))
                    .h(30.0)
                    .on_click(format!("row:{i}"))
            }));
        let root = widget::panel(FlexDir::Column).child(list);
        let node = layout::compute(&root, 400.0, 400.0);
        layout::find_node(&node, "list").unwrap().clone()
    }

    #[test]
    fn layout_clamps_and_applies_the_offset() {
        let v = view(50.0);
        assert_eq!(v.content_h, 300.0);
        assert_eq!(v.max_scroll(), 200.0);
        assert_eq!(v.scroll_y, 50.0);
        assert_eq!(v.children[0].rect.y, -50.0);
        // The scrollbar strip is reserved: rows stop short of it.
        assert_eq!(v.children[0].rect.w, 100.0 - layout::SCROLLBAR_W);
        assert_eq!(view(1000.0).scroll_y, 200.0);
        // Rows scrolled out of the view can't be clicked.
        let root_like = LayoutNode {
            children: vec![v.clone()],
            rect: v.rect,
            ..Default::default()
        };
        assert!(
            layout::hit_test(&root_like, 10.0, 5.0)
                .and_then(|n| n.on_click.as_deref())
                .is_some_and(|e| e == "row:1")
        );
    }

    #[test]
    fn wheel_and_knob_drag() {
        let v = view(0.0);
        let mut s = ScrollState::new();
        assert!(s.handle("scroll:-2.00", &v, 30.0));
        assert_eq!(s.offset, 60.0);
        assert!(s.handle("scroll:10", &v, 30.0));
        assert_eq!(s.offset, 0.0);
        // Knob: 100 * 100 / 300 = 33.3 px tall at the top of the track (x 94..100).
        assert!(
            !s.handle("click:50:10", &v, 30.0),
            "content click isn't ours"
        );
        assert!(s.handle("click:97:10", &v, 30.0));
        assert!(s.is_dragging());
        assert!(s.handle("drag:97:200", &v, 30.0));
        assert_eq!(s.offset, 200.0);
        assert!(s.handle("release:97:200", &v, 30.0));
        assert!(!s.is_dragging());
        assert!(!s.handle("drag:97:0", &v, 30.0));
    }

    #[test]
    fn no_scrollbar_without_overflow() {
        let list = widget::scroll_view().id("l").w(100.0).h(100.0);
        let node = layout::compute(&list.child(widget::label("x")), 400.0, 400.0);
        assert!(node.scrollbar_rects().is_none());
        assert!(!ScrollState::new().handle("click:97:10", &node, 10.0));
    }
}
