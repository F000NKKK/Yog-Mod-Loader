//! Widget types and styling.

use crate::layout::{Align, FlexDir};

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
        if on {
            self.insert(o)
        } else {
            self.remove(o)
        }
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
    pub bg: Option<u32>,
    pub color: Option<u32>,
    pub shadow: Option<bool>,
    pub focus_color: Option<u32>,
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
    /// Single-line text input — see [`TextBox`] and
    /// [`TextBoxState`](crate::TextBoxState), which owns the text and drives it.
    TextBox(TextBox),
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
    pub region: Option<[f32; 4]>,
    /// Color multiplier, `0xAARRGGBB` (`0xFFFFFFFF` = none; items ignore it).
    pub tint: u32,
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

/// Placeholder text color of a text box.
pub const PLACEHOLDER_COLOR: u32 = 0xFF_707070;
/// Border of an unfocused text box (vanilla `EditBox`).
pub const TEXT_BOX_BORDER: u32 = 0xFF_A0A0A0;
/// Border of a focused text box unless `focus_color` is set.
pub const TEXT_BOX_FOCUS_BORDER: u32 = 0xFF_FFFFFF;
/// Selection highlight behind selected text.
pub const SELECTION_COLOR: u32 = 0xFF_2850A0;

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
        if v <= 0.0 {
            Units::Auto
        } else {
            Units::Px(v)
        }
    }
}

impl Units {
    /// Resolve against `avail` (the parent's resolved content size on this
    /// axis). `None` for `Auto` — caller falls back to its own auto-sizing.
    pub fn resolve(self, avail: f32) -> Option<f32> {
        match self {
            Units::Auto => None,
            Units::Px(v) => Some(v),
            Units::Percent(p) => Some(avail * p / 100.0),
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
    pub bg: u32,          // background colour 0xAARRGGBB; 0 = transparent
    pub color: u32,       // text colour
    pub align: Align,
    pub font_scale: f32,
    pub text_shadow: bool, // MC drop-shadow behind text (HUD default; books disable it)
    pub focus_style: FocusStyle,
    pub focus_color: u32, // 0 = default amber 0xFF_FFE040
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
    /// Font id for text — empty (default) = Minecraft's default font
    /// (`minecraft:default`); also `minecraft:uniform`, `minecraft:alt`,
    /// `minecraft:illageralt` or a resource-pack font such as `mymod:fancy`.
    /// See [`crate::text`] for how non-default fonts are measured.
    pub font: String,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            w: Units::Auto,
            h: Units::Auto,
            min_w: 4.0,
            min_h: 4.0,
            flex: 0.0,
            dock: Dock::None,
            gap: 2.0,
            pad: [0.0; 4],
            margin: [0.0; 4],
            bg: 0,
            color: 0xFF_CCCCAA,
            align: Align::Start,
            font_scale: 1.0,
            text_shadow: true,
            focus_style: FocusStyle::default(),
            focus_color: 0,
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

// ── Builder API ───────────────────────────────────────────────────────────────

impl Widget {
    fn new(kind: WidgetKind) -> Self {
        let flex_dir = if let WidgetKind::Panel(dir) = &kind {
            *dir
        } else {
            FlexDir::Column
        };
        Self {
            kind,
            id: None,
            style: Style::default(),
            flex_dir,
            children: Vec::new(),
            on_click: None,
            enabled: true,
            focused: false,
            action: None,
            visible: true,
            state: StateFlags::NONE,
            states: None,
        }
    }

    /// Typed click target — see [`Action`].
    pub fn action(mut self, a: Action) -> Self {
        self.action = Some(a);
        self
    }
    /// `false` = collapsed — see [`Widget::visible`].
    pub fn visible(mut self, v: bool) -> Self {
        self.visible = v;
        self
    }
    /// Adds a paint-only override applied while the state contains `when`
    /// (later calls win over earlier ones).
    pub fn state_style(mut self, when: StateFlags, patch: StylePatch) -> Self {
        self.states
            .get_or_insert_with(Default::default)
            .rules
            .push((when, patch));
        self
    }

    /// The descendant at child-index `path` (`&[]` = self); `None` past the
    /// tree.
    pub fn get_path(&self, path: &[u16]) -> Option<&Widget> {
        let mut w = self;
        for &i in path {
            w = w.children.get(i as usize)?;
        }
        Some(w)
    }
    /// [`Self::get_path`], mutably.
    pub fn get_path_mut(&mut self, path: &[u16]) -> Option<&mut Widget> {
        let mut w = self;
        for &i in path {
            w = w.children.get_mut(i as usize)?;
        }
        Some(w)
    }
    /// The descendant at `path`. Panics on a bad path — for generated code,
    /// whose paths are fixed when it's generated.
    pub fn at_path(&self, path: &[u16]) -> &Widget {
        self.get_path(path)
            .unwrap_or_else(|| panic!("yog-ui: no widget at path {path:?}"))
    }
    /// [`Self::at_path`], mutably.
    pub fn at_path_mut(&mut self, path: &[u16]) -> &mut Widget {
        self.get_path_mut(path)
            .unwrap_or_else(|| panic!("yog-ui: no widget at path {path:?}"))
    }

    /// Whether clicking it does something (an [`Action`] or `on_click`).
    pub fn is_interactive(&self) -> bool {
        self.action.is_some() || self.on_click.is_some()
    }

    /// Sets (or clears) `mask` on this widget and its descendants, stopping
    /// at descendants that are interactive themselves (a button in a row
    /// keeps its own hover) — so a row's labels follow the row's state.
    pub fn set_state_inherited(&mut self, mask: StateFlags, on: bool) {
        self.state.set(mask, on);
        for c in &mut self.children {
            if !c.is_interactive() {
                c.set_state_inherited(mask, on);
            }
        }
    }

    /// Clears `mask` on the whole subtree (interactive descendants too).
    pub fn clear_state_deep(&mut self, mask: StateFlags) {
        self.state.remove(mask);
        for c in &mut self.children {
            c.clear_state_deep(mask);
        }
    }

    /// The style drawn: [`Self::style`] plus the [`Self::states`] patches
    /// matching [`Self::state`] (with DISABLED while `enabled` is false).
    pub fn effective_style(&self) -> std::borrow::Cow<'_, Style> {
        let mut state = self.state;
        if !self.enabled {
            state.insert(StateFlags::DISABLED);
        }
        self.effective_style_in(state)
    }

    /// [`Self::effective_style`] for an explicit `state`.
    pub fn effective_style_in(&self, state: StateFlags) -> std::borrow::Cow<'_, Style> {
        let Some(states) = &self.states else {
            return std::borrow::Cow::Borrowed(&self.style);
        };
        let mut matching = states
            .rules
            .iter()
            .filter(|(when, _)| !when.is_empty() && state.contains(*when))
            .peekable();
        if matching.peek().is_none() {
            return std::borrow::Cow::Borrowed(&self.style);
        }
        let mut s = self.style.clone();
        for (_, patch) in matching {
            patch.apply(&mut s);
        }
        std::borrow::Cow::Owned(s)
    }

    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }
    pub fn on_click(mut self, ev: impl Into<String>) -> Self {
        self.on_click = Some(ev.into());
        self
    }
    pub fn child(mut self, w: Widget) -> Self {
        self.children.push(w);
        self
    }

    /// Accepts a plain px value (`0.0` = auto) or a [`Units`] directly —
    /// e.g. `.w(18.0)` or `.w(Units::Percent(60.0))`.
    pub fn w(mut self, w: impl Into<Units>) -> Self {
        self.style.w = w.into();
        self
    }
    pub fn h(mut self, h: impl Into<Units>) -> Self {
        self.style.h = h.into();
        self
    }
    pub fn min_w(mut self, v: f32) -> Self {
        self.style.min_w = v;
        self
    }
    pub fn min_h(mut self, v: f32) -> Self {
        self.style.min_h = v;
        self
    }
    pub fn flex(mut self, v: f32) -> Self {
        self.style.flex = v;
        self
    }
    pub fn flex_dir(mut self, v: FlexDir) -> Self {
        self.flex_dir = v;
        self
    }
    pub fn gap(mut self, v: f32) -> Self {
        self.style.gap = v;
        self
    }
    pub fn bg(mut self, v: u32) -> Self {
        self.style.bg = v;
        self
    }
    pub fn color(mut self, v: u32) -> Self {
        self.style.color = v;
        self
    }
    pub fn align(mut self, v: Align) -> Self {
        self.style.align = v;
        self
    }
    pub fn font_scale(mut self, v: f32) -> Self {
        self.style.font_scale = v;
        self
    }
    pub fn shadow(mut self, v: bool) -> Self {
        self.style.text_shadow = v;
        self
    }
    pub fn no_wrap(mut self) -> Self {
        self.style.no_wrap = true;
        self
    }
    pub fn focus_style(mut self, v: FocusStyle) -> Self {
        self.style.focus_style = v;
        self
    }
    pub fn focus_color(mut self, v: u32) -> Self {
        self.style.focus_color = v;
        self
    }
    pub fn dock(mut self, v: Dock) -> Self {
        self.style.dock = v;
        self
    }
    pub fn enabled(mut self, v: bool) -> Self {
        self.enabled = v;
        self
    }
    pub fn focused(mut self, v: bool) -> Self {
        self.focused = v;
        self
    }
    /// Position absolutely at `(x, y)` inside the parent's content box — see [`Style::pos`].
    pub fn at(mut self, x: f32, y: f32) -> Self {
        self.style.pos = Some((x, y));
        self
    }
    /// Single line, cut with `...` when too wide — see [`Style::ellipsis`].
    pub fn ellipsis(mut self) -> Self {
        self.style.no_wrap = true;
        self.style.ellipsis = true;
        self
    }
    /// Clip children to this widget's rect — see [`Style::clip`].
    pub fn clip(mut self, v: bool) -> Self {
        self.style.clip = v;
        self
    }
    /// Vertical scroll offset of the children — see [`Style::scroll_y`].
    pub fn scroll_y(mut self, v: f32) -> Self {
        self.style.scroll_y = v;
        self
    }
    /// Draw a vertical scrollbar on overflow — see [`Style::scrollbar`].
    pub fn scrollbar(mut self, v: bool) -> Self {
        self.style.scrollbar = v;
        self
    }
    /// Font for this widget's text — see [`Style::font`].
    pub fn font(mut self, font: impl Into<String>) -> Self {
        self.style.font = font.into();
        self
    }
    /// Text box: greyed text shown while it's empty.
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        if let WidgetKind::TextBox(t) = &mut self.kind {
            t.placeholder = text.into();
        }
        self
    }
    /// Text box: longest text allowed, in characters (0 = unlimited).
    pub fn max_length(mut self, n: usize) -> Self {
        if let WidgetKind::TextBox(t) = &mut self.kind {
            t.max_length = n;
        }
        self
    }
    /// Text box: event name raised when the text changes.
    pub fn on_change(mut self, ev: impl Into<String>) -> Self {
        if let WidgetKind::TextBox(t) = &mut self.kind {
            t.on_change = Some(ev.into());
        }
        self
    }
    /// Text box: event name raised by Enter.
    pub fn on_submit(mut self, ev: impl Into<String>) -> Self {
        if let WidgetKind::TextBox(t) = &mut self.kind {
            t.on_submit = Some(ev.into());
        }
        self
    }
    /// Image: how it fills the widget (WPF's `Stretch`).
    pub fn stretch(mut self, stretch: Stretch) -> Self {
        if let WidgetKind::Image(i) = &mut self.kind {
            i.stretch = stretch;
        }
        self
    }
    /// Image: draw only `[u, v, w, h]` of it (texture pixels).
    pub fn region(mut self, region: [f32; 4]) -> Self {
        if let WidgetKind::Image(i) = &mut self.kind {
            i.region = Some(region);
        }
        self
    }
    /// Image: multiply its colors by `tint` (`0xAARRGGBB`).
    pub fn tint(mut self, tint: u32) -> Self {
        if let WidgetKind::Image(i) = &mut self.kind {
            i.tint = tint;
        }
        self
    }
    /// Adds every widget in `children`, in order.
    pub fn children(mut self, children: impl IntoIterator<Item = Widget>) -> Self {
        self.children.extend(children);
        self
    }

    /// The first widget in this subtree (depth-first, self included) whose
    /// [`Widget::id`] is `id` — for changing a built tree before laying it
    /// out (dynamic text, enabled state, ...).
    pub fn find_mut(&mut self, id: &str) -> Option<&mut Widget> {
        if self.id.as_deref() == Some(id) {
            return Some(self);
        }
        self.children.iter_mut().find_map(|c| c.find_mut(id))
    }

    /// Replaces the text of a label, button or text box; `false` for any
    /// other kind.
    pub fn set_text(&mut self, text: impl Into<String>) -> bool {
        match &mut self.kind {
            WidgetKind::Label(t) | WidgetKind::Button(t) => {
                *t = text.into();
                true
            }
            WidgetKind::TextBox(t) => {
                t.text = text.into();
                true
            }
            _ => false,
        }
    }

    /// [`set_text`](Self::set_text) as a builder call.
    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.set_text(text);
        self
    }

    pub fn padding(mut self, top: f32, right: f32, bottom: f32, left: f32) -> Self {
        self.style.pad = [top, right, bottom, left];
        self
    }
    pub fn margin(mut self, top: f32, right: f32, bottom: f32, left: f32) -> Self {
        self.style.margin = [top, right, bottom, left];
        self
    }
}

// ── Widget constructors ───────────────────────────────────────────────────────

pub fn panel(dir: FlexDir) -> Widget {
    Widget::new(WidgetKind::Panel(dir))
}
/// A vertically scrolling column: clips its children, scrolls them by
/// [`Widget::scroll_y`] and draws a scrollbar when they overflow — WPF's
/// `ScrollViewer` around a `StackPanel`. Give it a fixed (or flex) height.
pub fn scroll_view() -> Widget {
    panel(FlexDir::Column).clip(true).scrollbar(true)
}
pub fn label(text: impl Into<String>) -> Widget {
    Widget::new(WidgetKind::Label(text.into()))
}
pub fn button(text: impl Into<String>) -> Widget {
    Widget::new(WidgetKind::Button(text.into()))
}
pub fn item_slot(item_id: impl Into<String>) -> Widget {
    Widget::new(WidgetKind::ItemSlot(item_id.into()))
}
/// An inventory-slot decoration (icon + stack-count overlay) for `item_id`
/// (empty string = empty slot) with `count` items. Purely decorative — real
/// drag-and-drop still goes through the vanilla `Slot` this overlays.
pub fn inv_slot(item_id: impl Into<String>, count: u32) -> Widget {
    Widget::new(WidgetKind::InvSlot {
        item_id: item_id.into(),
        count,
    })
    .w(18.0)
    .h(18.0)
}
/// An image from a Yog image source — `"item:apple"`, `"block:stone"`,
/// `"sprite:button"`, `"mymod:gui/logo"` (see [`yog_gfx::image`]) — the
/// host resolves it for its Minecraft version. Sized to the image's natural
/// size (16×16 until the host told it) unless given `w`/`h`; scaled into the
/// widget per [`Widget::stretch`] (default `Uniform`), nearest-neighbour.
pub fn image(source: impl Into<String>) -> Widget {
    Widget::new(WidgetKind::Image(Image {
        source: source.into(),
        stretch: Stretch::Uniform,
        region: None,
        tint: 0xFFFF_FFFF,
    }))
}
pub fn spacer() -> Widget {
    Widget::new(WidgetKind::Spacer)
}
/// A single-line text input holding `text` — black box, grey border (white
/// while focused), like vanilla's `EditBox`. Its text, caret and focus live
/// in a [`TextBoxState`](crate::TextBoxState) that the code-behind keeps and
/// applies to it each frame.
pub fn text_box(text: impl Into<String>) -> Widget {
    Widget::new(WidgetKind::TextBox(TextBox {
        text: text.into(),
        ..TextBox::default()
    }))
    .bg(0xFF_000000)
    .color(0xFF_E0E0E0)
    .padding(0.0, 4.0, 0.0, 4.0)
}
