//! The builder API: a widget's style and children set call by call.

use super::{
    Action, Dock, FocusStyle, HAlign, StateFlags, Stretch, Style, StylePatch, Track, Units, VAlign,
    Widget, WidgetKind,
};
use yog_gfx::{Color, Rect};

use crate::layout::{Align, FlexDir};

impl Widget {
    pub(crate) fn new(kind: WidgetKind) -> Self {
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
            tooltip: None,
        }
    }

    /// See [`Self::tooltip`]; empty = none.
    pub fn tooltip(mut self, text: impl Into<String>) -> Self {
        self.set_tooltip(text);
        self
    }
    /// Sets (empty: clears) [`Self::tooltip`].
    pub fn set_tooltip(&mut self, text: impl Into<String>) {
        let t = text.into();
        self.tooltip = (!t.is_empty()).then_some(t);
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
    pub fn bg(mut self, v: Color) -> Self {
        self.style.bg = v;
        self
    }
    /// See [`Style::bg_image`].
    pub fn bg_image(mut self, source: impl Into<String>) -> Self {
        self.style.bg_image = source.into();
        self
    }
    pub fn color(mut self, v: Color) -> Self {
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
    pub fn focus_color(mut self, v: Color) -> Self {
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
    /// Image: draw only `region` of it (texture pixels).
    pub fn region(mut self, region: Rect) -> Self {
        if let WidgetKind::Image(i) = &mut self.kind {
            i.region = Some(region);
        }
        self
    }
    /// Image: multiply its colors by `tint`.
    pub fn tint(mut self, tint: Color) -> Self {
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

    /// Replaces an image's source or a slot's item id (other kinds: no-op).
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        match &mut self.kind {
            WidgetKind::Image(i) => i.source = source.into(),
            WidgetKind::Surface(id) => *id = source.into(),
            WidgetKind::ItemSlot(id) | WidgetKind::InvSlot { item_id: id, .. } => {
                *id = source.into()
            }
            _ => {}
        }
        self
    }

    /// Replaces an inventory slot's stack count (other kinds: no-op).
    pub fn with_count(mut self, n: u32) -> Self {
        if let WidgetKind::InvSlot { count, .. } = &mut self.kind {
            *count = n;
        }
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

    /// Puts it in grid cell `(row, col)` (spanning one row and column) —
    /// WPF's `Grid.Row` / `Grid.Column`. See [`Self::span`].
    pub fn cell(mut self, row: u16, col: u16) -> Self {
        self.style.cell.row = row;
        self.style.cell.col = col;
        self
    }
    /// Spans `rows` × `cols` grid cells from its [`Self::cell`] — WPF's
    /// `Grid.RowSpan` / `Grid.ColumnSpan` (0 counts as 1).
    pub fn span(mut self, rows: u16, cols: u16) -> Self {
        self.style.cell.row_span = rows.max(1);
        self.style.cell.col_span = cols.max(1);
        self
    }
    /// Horizontal placement in its slot — see [`HAlign`].
    pub fn halign(mut self, v: HAlign) -> Self {
        self.style.halign = Some(v);
        self
    }
    /// Vertical placement in its slot — see [`VAlign`].
    pub fn valign(mut self, v: VAlign) -> Self {
        self.style.valign = Some(v);
        self
    }

    /// Grid: its rows (no rows = one `*` row).
    pub fn rows(mut self, rows: impl IntoIterator<Item = impl Into<Track>>) -> Self {
        if let WidgetKind::Grid(g) = &mut self.kind {
            g.rows = rows.into_iter().map(Into::into).collect();
        }
        self
    }
    /// Grid: its columns (no columns = one `*` column).
    pub fn cols(mut self, cols: impl IntoIterator<Item = impl Into<Track>>) -> Self {
        if let WidgetKind::Grid(g) = &mut self.kind {
            g.cols = cols.into_iter().map(Into::into).collect();
        }
        self
    }
    /// Grid: space between rows.
    pub fn row_spacing(mut self, v: f32) -> Self {
        if let WidgetKind::Grid(g) = &mut self.kind {
            g.row_spacing = v;
        }
        self
    }
    /// Grid: space between columns.
    pub fn col_spacing(mut self, v: f32) -> Self {
        if let WidgetKind::Grid(g) = &mut self.kind {
            g.col_spacing = v;
        }
        self
    }
}
