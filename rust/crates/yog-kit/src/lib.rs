//! yog-kit — MVVM screens for Yog mods.
//!
//! A screen is a `.yui` file with `x:ViewModel="MyVm"` plus a plain Rust
//! view model. `yog build` compiles the markup into a [`View`] (widget tree,
//! compiled `{x:Bind ...}` bindings, command dispatch) and this crate hosts
//! it: it owns the view model, re-evaluates bindings after every change,
//! routes input (clicks, hover, scrolling, text boxes, Tab focus, list
//! keyboard selection, Esc) and draws the result.
//!
//! ```ignore
//! // src/counter.rs, next to src/counter.yui
//! yog_api::ui_include!("counter");
//! use yog_kit::prelude::*;
//!
//! #[derive(Default)]
//! pub struct CounterVm { count: u32 }
//! impl ViewModel for CounterVm {}
//! impl CounterVm {
//!     pub fn increment(&mut self) { self.count += 1 }
//! }
//!
//! // src/lib.rs: counter::register(registry); ... counter::open();
//! ```
//!
//! The view model never touches widgets: the markup binds to its fields and
//! methods, and commands call its methods. [`testing::TestHost`] drives a
//! screen without the game.

extern crate self as yog_kit;

pub mod bind;
mod host;
pub mod items;
mod screen;
/// Driving a screen in unit tests: a mod's tests turn it on with the
/// `testing` feature (`yog build` does, in their `[dev-dependencies]`).
#[cfg(any(test, feature = "testing"))]
pub mod testing;

pub use host::{Update, Viewport};
pub use screen::{Cx, Screen, ScreenHandle, handle_of, inventory_of};
pub use yog_api::ui::{Action, StateFlags, Widget};

/// Everything a code-behind file needs.
pub mod prelude {
    pub use crate::{Cx, ScreenHandle, ViewModel};
}

/// A screen's state and logic. Implement it (often with no methods) for the
/// type named by the `.yui` file's `x:ViewModel`.
///
/// All hooks run on the render thread with the screen locked; use
/// [`Cx`] to close the screen, navigate, or get a [`ScreenHandle`] for
/// updates from other threads.
#[allow(unused_variables)]
pub trait ViewModel: Send + 'static {
    /// The screen was opened (every time).
    fn on_open(&mut self, cx: &mut Cx<'_, Self>)
    where
        Self: Sized,
    {
    }
    /// The screen was closed (by any path).
    fn on_close(&mut self, cx: &mut Cx<'_, Self>)
    where
        Self: Sized,
    {
    }
    /// Esc / back. Return `true` when handled; `false` (the default) goes
    /// back to the screen this one was navigated from, else closes it.
    fn on_back(&mut self, cx: &mut Cx<'_, Self>) -> bool
    where
        Self: Sized,
    {
        false
    }
    /// Escape hatch: imperative changes to the content tree after each
    /// binding update. Rarely needed.
    fn customize(&self, root: &mut Widget) {}
}

/// Where a screen's content sits in the GUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Anchor {
    #[default]
    Center,
    TopLeft,
    Top,
    TopRight,
    Left,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
    /// Stretched over the whole GUI.
    Fill,
}

impl Anchor {
    /// Top-left corner of a `w` × `h` box anchored in a `sw` × `sh` GUI —
    /// as the game places its screens: a box bigger than the GUI keeps its
    /// size and is cut at the edges (evenly when centred), and a
    /// container's picture stays over its real slots (the game's
    /// whole-pixel `(width - backgroundWidth) / 2`).
    pub fn place(self, sw: f32, sh: f32, w: f32, h: f32) -> (f32, f32) {
        let cx = ((sw - w) / 2.0).trunc();
        let cy = ((sh - h) / 2.0).trunc();
        let (x, y) = match self {
            Anchor::Center => (cx, cy),
            Anchor::TopLeft | Anchor::Fill => (0.0, 0.0),
            Anchor::Top => (cx, 0.0),
            Anchor::TopRight => (sw - w, 0.0),
            Anchor::Left => (0.0, cy),
            Anchor::Right => (sw - w, cy),
            Anchor::BottomLeft => (0.0, sh - h),
            Anchor::Bottom => (cx, sh - h),
            Anchor::BottomRight => (sw - w, sh - h),
        };
        (x, y)
    }
}

/// Static facts about a screen, from its `<Screen>` attributes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenMeta {
    /// UI id (`register_ui`, `open_ui`).
    pub id: &'static str,
    /// Design size in GUI pixels.
    pub w: f32,
    pub h: f32,
    pub anchor: Anchor,
    pub modal: bool,
    pub pause: bool,
    /// Re-evaluates the bindings every frame (`<Screen live="true">`): for
    /// a screen showing state that changes outside its own commands (a
    /// server tick, a packet). Inventory screens always are.
    pub live: bool,
}

/// Input for a view's [`View::dispatch`], produced by the host.
#[derive(Debug, Clone, PartialEq)]
pub enum ViewEvent {
    /// A widget with this [`Action`] was clicked or activated (Enter/Space).
    Action(Action),
    /// Text box `slot` (index in [`Parts`]) was edited; `text` is its text.
    TextChanged { slot: u16, text: String },
    /// Enter in text box `slot`.
    TextSubmitted { slot: u16, text: String },
    /// Keyboard selection in list `list` (its owner id) moved to `index`.
    Select { list: u16, index: u32 },
}

/// A compiled screen — implemented by `.yui` codegen only.
///
/// The host owns the content tree; paths are child-index paths from its
/// root ([`Widget::get_path`]).
pub trait View: Sized + Send + 'static {
    type Vm: ViewModel;
    const META: ScreenMeta;
    /// The view's binding state, its content tree and its interactive parts.
    fn build() -> (Self, Widget, Parts);
    /// Re-evaluates the bindings against `vm`, patching `root`.
    fn update(&mut self, root: &mut Widget, vm: &Self::Vm, u: &mut Update);
    /// Runs the command (or write-back) for `ev`.
    fn dispatch(&mut self, vm: &mut Self::Vm, cx: &mut Cx<'_, Self::Vm>, ev: ViewEvent);
}

/// One scrolling container of a view.
#[derive(Debug, Clone)]
pub struct ScrollerPart {
    pub path: Vec<u16>,
    pub state: yog_api::ui::ScrollState,
}

/// One text box of a view.
#[derive(Debug, Clone)]
pub struct TextBoxPart {
    pub path: Vec<u16>,
    pub state: yog_api::ui::TextBoxState,
}

/// One list (ListView / ItemsControl) of a view.
#[derive(Debug, Clone)]
pub struct ListPart {
    /// The list's owner id (in its items' [`Action::owner`]).
    pub owner: u16,
    /// The items container.
    pub path: Vec<u16>,
    /// Its scroller (index in [`Parts`]), for ListView.
    pub scroller: Option<u16>,
    /// Fixed row height (virtualization, keyboard reveal).
    pub item_height: Option<f32>,
    /// Item count and selected index after the last update.
    pub len: u32,
    pub selected: Option<u32>,
}

/// The interactive parts of a view, created by [`View::build`]: scrollers,
/// text boxes, lists, and the Tab focus order. Indices are the order they
/// were added.
#[derive(Debug, Clone, Default)]
pub struct Parts {
    pub scrollers: Vec<ScrollerPart>,
    pub text_boxes: Vec<TextBoxPart>,
    pub lists: Vec<ListPart>,
    /// Tab order: buttons, text boxes and lists, in document order.
    pub focusables: Vec<Vec<u16>>,
}

impl Parts {
    pub fn new() -> Self {
        Self::default()
    }
    /// Adds a scroll view at `path`; returns its index.
    pub fn scroller(&mut self, path: &[u16]) -> u16 {
        self.scrollers.push(ScrollerPart {
            path: path.to_vec(),
            state: Default::default(),
        });
        (self.scrollers.len() - 1) as u16
    }
    /// Adds a text box at `path` (also focusable); returns its slot.
    pub fn text_box(&mut self, path: &[u16]) -> u16 {
        self.text_boxes.push(TextBoxPart {
            path: path.to_vec(),
            state: Default::default(),
        });
        self.focusables.push(path.to_vec());
        (self.text_boxes.len() - 1) as u16
    }
    /// Adds the list `owner` whose items container is at `path` (focusable
    /// when it has a scroller, i.e. is a ListView).
    pub fn list(
        &mut self,
        owner: u16,
        path: &[u16],
        scroller: Option<u16>,
        item_height: Option<f32>,
    ) {
        self.lists.push(ListPart {
            owner,
            path: path.to_vec(),
            scroller,
            item_height,
            len: 0,
            selected: None,
        });
        if scroller.is_some() {
            self.focusables.push(path.to_vec());
        }
    }
    /// Adds a focusable widget (a button) to the Tab order.
    pub fn focusable(&mut self, path: &[u16]) {
        self.focusables.push(path.to_vec());
    }
}

/// Used by generated code only.
#[doc(hidden)]
pub mod __codegen {
    /// The codegen version this yog-kit understands.
    pub const CODEGEN: u32 = yog_api::yog_ids::versions::UI_CODEGEN;

    /// Compile-time check that generated code matches this yog-kit
    /// (`const _: () = require(N);`: a mismatch fails the mod's build).
    pub const fn require(version: u32) {
        assert!(
            version <= CODEGEN,
            "this .yui code was generated for a newer yog-kit; update the yog-kit dependency"
        );
        assert!(
            version >= CODEGEN,
            "this .yui code was generated by an old yog; regenerate it with `yog build`"
        );
    }
}
