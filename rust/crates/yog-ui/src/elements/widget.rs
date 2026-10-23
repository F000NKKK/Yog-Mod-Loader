//! Widget types and styling.

use yog_gfx::{Color, Rect};

use crate::layout::{Align, FlexDir};

mod builder;
mod make;

/// The ids of inventory widgets ([`crate::inventory_ids`]).
pub use crate::inventory_ids::{MACHINE_PREFIX, PLAYER_INVENTORY, SLOT_PREFIX};

pub use make::{
    HOTBAR_GAP, button, grid, image, inv_slot, item_slot, label, machine, panel, player_inventory,
    scroll_view, slot, slot_accepting, spacer, surface, text_box,
};

/// A single widget in the UI tree.
#[derive(Debug, Clone)]
pub struct Widget {
    pub kind: WidgetKind,
    pub id: Option<String>,
    pub style: Style,
    pub flex_dir: FlexDir,
    pub children: Vec<Widget>,
    pub on_click: Option<String>,
    pub enabled: bool,
    pub focused: bool,
    /// Typed click target set by generated code (yog-kit) — like
    /// [`Self::on_click`], but no string to parse. See [`Action`].
    pub action: Option<Action>,
    /// `false` = collapsed: takes no space, isn't drawn and can't be hit
    /// (its layout node is a zero rect without children).
    pub visible: bool,
    /// Interaction state (hover, pressed, selected, ...) the host sets each
    /// frame — picks the [`Self::states`] patches applied when drawing.
    pub state: StateFlags,
    /// Paint-only style overrides per state (`hover:bg="..."` in `.yui`).
    pub states: Option<Box<StateStyles>>,
    /// Text shown in a tooltip while the cursor rests on it (`\n` breaks
    /// lines) — see [`crate::UiRoot::tooltip_at`].
    pub tooltip: Option<String>,
}

/// A typed click target: `owner` 0 = the screen, `k` = items control `k`;
/// `slot` = the command (or [`Action::SELECT`]); `index` = the item index
/// inside an items control (0 elsewhere).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Action {
    pub owner: u16,
    pub slot: u16,
    pub index: u32,
}

impl Action {
    /// The `slot` of an items control's item container: clicking selects it.
    pub const SELECT: u16 = u16::MAX;

    /// A screen-level command `slot`.
    pub const fn new(slot: u16) -> Self {
        Self {
            owner: 0,
            slot,
            index: 0,
        }
    }
}

/// Interaction state bits of a widget — see [`Widget::state`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct StateFlags(pub u8);

impl StateFlags {
    pub const NONE: Self = Self(0);
    pub const HOVER: Self = Self(1);
    pub const PRESSED: Self = Self(2);
    pub const FOCUSED: Self = Self(4);
    pub const SELECTED: Self = Self(8);
    pub const CHECKED: Self = Self(16);
    pub const DISABLED: Self = Self(32);

    /// Every bit of `o` is set.
    pub const fn contains(self, o: Self) -> bool {
        self.0 & o.0 == o.0
    }
    pub fn insert(&mut self, o: Self) {
        self.0 |= o.0;
    }
    pub fn remove(&mut self, o: Self) {
        self.0 &= !o.0;
    }
    /// Sets or clears the bits of `o`.
    pub fn set(&mut self, o: Self, on: bool) {
        if on { self.insert(o) } else { self.remove(o) }
    }
    pub const fn union(self, o: Self) -> Self {
        Self(self.0 | o.0)
    }
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl std::ops::BitOr for StateFlags {
    type Output = Self;
    fn bitor(self, o: Self) -> Self {
        self.union(o)
    }
}

/// Paint-only style overrides — never affect layout.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StylePatch {
    pub bg: Option<Color>,
    pub bg_image: Option<String>,
    pub color: Option<Color>,
    pub shadow: Option<bool>,
    pub focus_color: Option<Color>,
}

impl StylePatch {
    /// `true` when it overrides nothing.
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    fn apply(&self, s: &mut Style) {
        if let Some(v) = self.bg {
            s.bg = v;
        }
        if let Some(v) = &self.bg_image {
            s.bg_image.clone_from(v);
        }
        if let Some(v) = self.color {
            s.color = v;
        }
        if let Some(v) = self.shadow {
            s.text_shadow = v;
        }
        if let Some(v) = self.focus_color {
            s.focus_color = v;
        }
    }
}

/// A widget's state patches: each applies when the widget's state contains
/// its flags; later rules win.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StateStyles {
    pub rules: Vec<(StateFlags, StylePatch)>,
}

#[derive(Debug, Clone)]
pub enum WidgetKind {
    /// Container — arranges children according to `flex_dir`.
    Panel(FlexDir),
    /// Container — WPF's `Grid`: rows and columns sized in pixels, to
    /// content (`Auto`) or as shares of the rest (`*`); each child sits in
    /// the cell(s) its [`Style::cell`] names — see [`grid`] and
    /// [`GridSpec`].
    Grid(Box<GridSpec>),
    /// Static or dynamic text label.
    Label(String),
    /// Clickable button with text.
    Button(String),
    /// Minecraft item icon (static, known at build time).
    ItemSlot(String),
    /// Inventory-slot decoration: an item icon plus a stack-count overlay,
    /// both supplied as plain data at tree-build time (like every other
    /// widget here) — this crate has no notion of "the currently open menu"
    /// or how to query it; that's the caller's job (see `yog-inventory`).
    /// Empty `item_id` draws an empty slot box.
    InvSlot { item_id: String, count: u32 },
    /// An image from a Yog image source (`item:apple`, `sprite:button`,
    /// `mymod:gui/logo`, ...) — see [`Image`] and [`image`].
    Image(Image),
    /// Invisible spacer.
    Spacer,
    /// Drawn by the code: the [`Surface`](crate::Surface) registered under
    /// this id (see [`crate::surface`] and [`surface`]).
    Surface(String),
    /// Single-line text input — see [`TextBox`] and
    /// [`TextBoxState`](crate::TextBoxState), which owns the text and drives it.
    TextBox(TextBox),
}

/// How a grid row or column is sized — WPF's `GridLength`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GridLength {
    /// Fits its content: the largest child in it (`Auto`).
    Auto,
    /// A fixed size in GUI pixels (`26`).
    Px(f32),
    /// A share of the space the other rows/columns leave (`*` = `Star(1.0)`,
    /// `2*` = `Star(2.0)`). Along an axis being measured for content (an
    /// auto-sized grid in a scroll view, ...) it acts as [`Self::Auto`].
    Star(f32),
}

impl GridLength {
    /// `*`.
    pub const STAR: Self = Self::Star(1.0);

    /// A size as `.yui` writes it: `Auto` (any case), `*`, `2*`, `26`.
    pub fn parse(text: &str) -> Option<GridLength> {
        let text = text.trim();
        if text.eq_ignore_ascii_case(AUTO) {
            return Some(GridLength::Auto);
        }
        if let Some(weight) = text.strip_suffix(STAR) {
            let weight = weight.trim();
            return if weight.is_empty() {
                Some(GridLength::STAR)
            } else {
                weight.parse().ok().map(GridLength::Star)
            };
        }
        text.parse().ok().map(GridLength::Px)
    }
}

/// How `.yui` writes [`GridLength::Auto`] and a share ([`GridLength::Star`]).
const AUTO: &str = "auto";
const STAR: char = '*';

/// One row or column of a [`GridSpec`]: its size and the limits on it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Track {
    pub size: GridLength,
    /// Never smaller than this (WPF's `MinHeight`/`MinWidth`).
    pub min: f32,
    /// Never larger than this (`f32::INFINITY` = no limit).
    pub max: f32,
}

impl Track {
    pub const fn new(size: GridLength) -> Self {
        Self {
            size,
            min: 0.0,
            max: f32::INFINITY,
        }
    }
    /// `Auto`.
    pub const fn auto() -> Self {
        Self::new(GridLength::Auto)
    }
    /// A fixed `px`.
    pub const fn px(px: f32) -> Self {
        Self::new(GridLength::Px(px))
    }
    /// `weight*`.
    pub const fn star(weight: f32) -> Self {
        Self::new(GridLength::Star(weight))
    }
    pub const fn min(mut self, v: f32) -> Self {
        self.min = v;
        self
    }
    pub const fn max(mut self, v: f32) -> Self {
        self.max = v;
        self
    }
}

impl From<GridLength> for Track {
    fn from(size: GridLength) -> Self {
        Self::new(size)
    }
}

/// A grid's rows, columns and the space between them. No rows (columns)
/// = one `*` row (column), like WPF.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GridSpec {
    pub rows: Vec<Track>,
    pub cols: Vec<Track>,
    /// Space between rows (WinUI's `RowSpacing`).
    pub row_spacing: f32,
    /// Space between columns (WinUI's `ColumnSpacing`).
    pub col_spacing: f32,
}

/// The grid cell(s) a widget occupies — WPF's `Grid.Row`, `Grid.Column`,
/// `Grid.RowSpan`, `Grid.ColumnSpan`. Only a [`grid`] reads it; out-of-range
/// values are clamped to the grid (like WPF).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    pub row: u16,
    pub col: u16,
    /// Rows spanned, at least 1.
    pub row_span: u16,
    /// Columns spanned, at least 1.
    pub col_span: u16,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            row: 0,
            col: 0,
            row_span: 1,
            col_span: 1,
        }
    }
}

/// Where a widget sits horizontally in the space its parent gives it — WPF's
/// `HorizontalAlignment`. In a grid cell `None` means `Stretch`; in a
/// Column it overrides the parent's `align` for this child.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HAlign {
    Left,
    Center,
    Right,
    /// Fill the width (unless the widget has an explicit `w`).
    Stretch,
}

/// Where a widget sits vertically — WPF's `VerticalAlignment`; see [`HAlign`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VAlign {
    Top,
    Center,
    Bottom,
    /// Fill the height (unless the widget has an explicit `h`).
    Stretch,
}

/// An image widget's data — see [`image`].
#[derive(Debug, Clone, PartialEq)]
pub struct Image {
    /// A Yog image source (see [`yog_gfx::image`]).
    pub source: String,
    /// How the image fills the widget (default [`Stretch::Uniform`]).
    pub stretch: Stretch,
    /// The part of the image drawn, `[u, v, w, h]` in the image's pixels
    /// (`None` = all of it). Textures only — items and sprites are whole.
    pub region: Option<Rect>,
    /// Color multiplier, `0xAARRGGBB` (`0xFFFFFFFF` = none; items ignore it).
    pub tint: Color,
}

pub use yog_gfx::image::Stretch;

/// A text box's content and editing state, as drawn this frame. Built by
/// [`text_box`] (text, placeholder, max length, event names); the caret,
/// selection and horizontal scroll are filled in each frame by
/// [`TextBoxState::apply`](crate::TextBoxState::apply).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TextBox {
    pub text: String,
    /// Shown greyed while the text is empty.
    pub placeholder: String,
    /// Longest text allowed, in characters; 0 = unlimited.
    pub max_length: usize,
    /// Event name for text changes (see `TextBoxState::event_name`).
    pub on_change: Option<String>,
    /// Event name for Enter (see `TextBoxState::event_name`).
    pub on_submit: Option<String>,
    /// Caret position in characters — `Some` while the box has focus.
    pub caret: Option<usize>,
    /// Draw the caret this frame (it blinks).
    pub caret_visible: bool,
    /// Selected characters `start..end` (empty = none).
    pub selection: (usize, usize),
    /// First visible character (horizontal scroll).
    pub first: usize,
}

/// Side of an item / inventory slot box in GUI pixels (vanilla's 18).
pub const SLOT_SIZE: f32 = 18.0;
/// A `<Screen>`'s size when it doesn't give one: the game's container screens' (176 × 166).
pub const SCREEN_W: f32 = 176.0;
pub const SCREEN_H: f32 = 166.0;
/// Focus indicator color when `focus_color` is 0 (amber).
pub const DEFAULT_FOCUS_COLOR: Color = Color(0xFF_FFE040);

/// A text box's default left/right padding.
pub(crate) const TEXT_BOX_PAD_X: f32 = 4.0;
/// Placeholder text color of a text box.
pub const PLACEHOLDER_COLOR: Color = Color(0xFF_707070);
/// Border of an unfocused text box (vanilla `EditBox`).
pub const TEXT_BOX_BORDER: Color = Color(0xFF_A0A0A0);
/// A text box's text, as vanilla's `EditBox` draws it.
pub const TEXT_BOX_TEXT: Color = Color(0xFF_E0E0E0);
/// Border of a focused text box unless `focus_color` is set.
pub const TEXT_BOX_FOCUS_BORDER: Color = Color(0xFF_FFFFFF);
/// Selection highlight behind selected text.
pub const SELECTION_COLOR: Color = Color(0xFF_2850A0);

/// WinForms-style dock — which edge(s) the widget attaches to inside its parent.
///
/// - `None`   — normal flex positioning (default)
/// - `Fill`   — stretch to fill all remaining space (both axes)
/// - `Left`   — full height, natural width, hugs left edge; other children flow right
/// - `Right`  — full height, natural width, hugs right edge
/// - `Top`    — full width, natural height, hugs top edge; other children flow down
/// - `Bottom` — full width, natural height, hugs bottom edge
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Dock {
    #[default]
    None,
    Fill,
    Left,
    Right,
    Top,
    Bottom,
}

/// How a focused widget shows its focus indicator.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum FocusStyle {
    /// 1px outline using `focus_color` (default, amber).
    #[default]
    Outline,
    /// Solid background fill using `focus_color`.
    Fill,
    /// No visual indicator.
    None,
}

/// A size value — either a fixed pixel amount, a CSS-style percentage of the
/// parent's own resolved content size (0.0–100.0), or `Auto` (size to
/// content/available space, this crate's original behaviour).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Units {
    #[default]
    Auto,
    Px(f32),
    /// Percent of the parent's resolved content size along the same axis.
    Percent(f32),
}

impl From<f32> for Units {
    /// `0.0` maps to `Auto` (matches this crate's original `0.0 = auto` convention,
    /// so every existing `.w(18.0)`/`.h(0.0)` call keeps meaning exactly what it did).
    fn from(v: f32) -> Self {
        if v <= 0.0 { Units::Auto } else { Units::Px(v) }
    }
}

/// A whole, in [`Units::Percent`].
const PERCENT: f32 = 100.0;
/// A widget's least size when its style gives none.
const MIN_SIZE: f32 = 4.0;
/// Text's color when its style gives none.
pub const DEFAULT_TEXT_COLOR: Color = Color(0xFF_CCCCAA);

impl Units {
    /// Resolve against `avail` (the parent's resolved content size on this
    /// axis). `None` for `Auto` — caller falls back to its own auto-sizing.
    pub fn resolve(self, avail: f32) -> Option<f32> {
        match self {
            Units::Auto => None,
            Units::Px(v) => Some(v),
            Units::Percent(p) => Some(avail * p / PERCENT),
        }
    }

    pub fn is_auto(self) -> bool {
        matches!(self, Units::Auto)
    }
}

/// Visual and layout style for a widget.
#[derive(Debug, Clone)]
pub struct Style {
    pub w: Units, // explicit width; Auto = size to content/available space
    pub h: Units, // explicit height; Auto = size to content/available space
    pub min_w: f32,
    pub min_h: f32,
    pub flex: f32,        // grow factor inside flex container (main axis)
    pub dock: Dock,       // WinForms-style edge attachment
    pub gap: f32,         // spacing between children
    pub pad: [f32; 4],    // top, right, bottom, left — space INSIDE the border
    pub margin: [f32; 4], // top, right, bottom, left — space OUTSIDE the border
    /// The grid cell(s) it occupies inside a [`grid`] — see [`Cell`].
    pub cell: Cell,
    /// Horizontal placement in its slot (grid cell, or a Column's width) —
    /// see [`HAlign`]. `None` = the parent's default.
    pub halign: Option<HAlign>,
    /// Vertical placement in its slot (grid cell, or a Row's height) — see
    /// [`VAlign`]. `None` = the parent's default.
    pub valign: Option<VAlign>,
    pub bg: Color, // background color; transparent = none
    /// Background image, drawn over `bg` and stretched to the box — a Yog
    /// image source (`sprite:button`, `sprite:panel`, `mymod:gui/frame`);
    /// sprites are nine-sliced, so frames keep their edges. Empty = none.
    pub bg_image: String,
    pub color: Color, // text color
    pub align: Align,
    pub font_scale: f32,
    pub text_shadow: bool, // MC drop-shadow behind text (HUD default; books disable it)
    pub focus_style: FocusStyle,
    pub focus_color: Color, // transparent = DEFAULT_FOCUS_COLOR
    /// Labels/buttons: never word-wrap, even if narrower than the text.
    /// For single-line titles/headers that must stay on one line — matches
    /// Patchouli, where such text overflows its box horizontally rather than
    /// wrapping onto a second line.
    pub no_wrap: bool,
    /// Absolute position `(x, y)` inside the parent's content box (inside its
    /// padding) — WPF `Canvas` / WinForms `Location` style. A positioned
    /// widget is taken out of the parent's flex flow: it takes no main-axis
    /// space, ignores `flex`/`dock`, and is drawn in tree order (later
    /// siblings on top). `None` (default) = normal flex positioning.
    pub pos: Option<(f32, f32)>,
    /// Labels/buttons: a single line (implies `no_wrap`) cut with `...` when
    /// wider than the box, instead of overflowing it.
    pub ellipsis: bool,
    /// Containers: children are drawn (and hit) only inside this widget's
    /// rect — anything scrolled or overflowing past it is cut off.
    pub clip: bool,
    /// Containers: vertical scroll offset of the children in GUI pixels
    /// (0 = top). Layout clamps it to the content — the effective value is
    /// [`LayoutNode::scroll_y`](crate::LayoutNode::scroll_y).
    pub scroll_y: f32,
    /// Containers: reserve a strip on the right edge and draw a vertical
    /// scrollbar there whenever the content overflows (see
    /// [`ScrollState`](crate::ScrollState) for driving it).
    pub scrollbar: bool,
    /// Font id for text — empty (default) = the game's default font
    /// (`yog:font/default`); a Yog font such as `mymod:fancy`, or another
    /// game font by Yog's alias: `yog:font/uniform`, `yog:font/alt`,
    /// `yog:font/illageralt`.
    /// See [`crate::text`] for how non-default fonts are measured.
    pub font: String,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            w: Units::Auto,
            h: Units::Auto,
            min_w: MIN_SIZE,
            min_h: MIN_SIZE,
            flex: 0.0,
            dock: Dock::None,
            gap: 2.0,
            pad: [0.0; 4],
            margin: [0.0; 4],
            cell: Cell::default(),
            halign: None,
            valign: None,
            bg: Color::TRANSPARENT,
            bg_image: String::new(),
            color: DEFAULT_TEXT_COLOR,
            align: Align::Start,
            font_scale: 1.0,
            text_shadow: true,
            focus_style: FocusStyle::default(),
            focus_color: Color::TRANSPARENT,
            no_wrap: false,
            pos: None,
            ellipsis: false,
            clip: false,
            scroll_y: 0.0,
            scrollbar: false,
            font: String::new(),
        }
    }
}
