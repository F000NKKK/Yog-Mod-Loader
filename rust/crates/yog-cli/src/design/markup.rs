//! A `.yui` tree (as the designer holds it) → the `yog_ui::Widget` tree the
//! generated `.g.rs` builds in game — the same builder calls `yog
//! build` emits (yog-cli `yui.rs`), made directly instead of as source —
//! wrapped like the generated `root_with` (the screen panel, anchored).

use yog_ui::attr::Attr;
use crate::yui_resources::{Entry, Resources, local_name};
use std::collections::HashMap;

use serde::Deserialize;
use yog_ui::widget::{self, Widget};
use yog_ui::{FlexDir, Track, Units};

/// One element of the designer's document (`YuiNode` in `yui.ts`).
#[derive(Debug, Clone, Deserialize)]
pub struct YuiNode {
    pub uid: String,
    pub kind: String,
    #[serde(default)]
    pub attrs: HashMap<String, String>,
    #[serde(default)]
    pub children: Vec<YuiNode>,
}

impl YuiNode {
    pub fn attr(&self, key: &str) -> Option<&str> {
        self.attrs.get(key).map(String::as_str)
    }
}

const CONTAINERS: &[&str] = &[yog_ids::yui::SCREEN, yog_ids::yui::CANVAS, yog_ids::yui::COLUMN, yog_ids::yui::ROW, yog_ids::yui::SCROLL_VIEW, yog_ids::yui::GRID];

pub fn is_container(kind: &str) -> bool {
    CONTAINERS.contains(&kind)
}

/// A property element (`<Grid.RowDefinitions>`) — part of its parent, not a widget.
pub fn is_property_element(node: &YuiNode) -> bool {
    node.kind.contains('.')
}

/// The children that are widgets (property elements skipped).
pub fn widget_children(node: &YuiNode) -> impl Iterator<Item = &YuiNode> {
    node.children.iter().filter(|c| !is_property_element(c))
}

/// One row/column size: `Auto`, `*`, `2*`, `26` (yog-cli's `yui_grid`).
fn track(v: &str) -> Option<Track> {
    let v = v.trim();
    if v.eq_ignore_ascii_case("auto") {
        return Some(Track::auto());
    }
    if let Some(w) = v.strip_suffix('*') {
        let w = if w.trim().is_empty() {
            1.0
        } else {
            w.trim().parse().ok()?
        };
        return Some(Track::star(w));
    }
    v.parse().ok().map(Track::px)
}

/// A grid's rows (`row`) or columns: `rows="Auto,*"` or `<Grid.RowDefinitions>`.
/// Sizes it can't read count as `*`, so the canvas still draws while typing.
fn tracks(node: &YuiNode, row: bool) -> Option<Vec<Track>> {
    let (attr, prop, size, min, max) = if row {
        (
            "rows",
            "Grid.RowDefinitions",
            "height",
            "min_height",
            "max_height",
        )
    } else {
        (
            "columns",
            "Grid.ColumnDefinitions",
            "width",
            "min_width",
            "max_width",
        )
    };
    if let Some(v) = node.attr(attr) {
        return Some(
            v.split(',')
                .map(|p| track(p).unwrap_or(Track::star(1.0)))
                .collect(),
        );
    }
    let defs = node.children.iter().find(|c| c.kind == prop)?;
    Some(
        defs.children
            .iter()
            .map(|d| {
                let mut t = track(d.attr(size).unwrap_or("*")).unwrap_or(Track::star(1.0));
                if let Some(n) = num(d.attr(min)) {
                    t = t.min(n);
                }
                if let Some(n) = num(d.attr(max)) {
                    t = t.max(n);
                }
                t
            })
            .collect(),
    )
}

fn cell_index(v: Option<&str>) -> Option<u16> {
    v?.trim().parse().ok()
}

fn num(v: Option<&str>) -> Option<f32> {
    v?.trim().parse().ok()
}

/// `#RRGGBB`, `#AARRGGBB` or `0xAARRGGBB`: the `.yui` compiler's rule.
pub use crate::yui::color;

/// A translation key's mark in a text: `@lang:key`.
const LANG: &str = yog_ids::concatcp!(yog_ids::typed::LANG, yog_ids::sep::NAMESPACE);

/// What a text attribute shows: `@lang:key` is a translation key (shown as
/// the key — the designer has no game language), `@@x` the literal `@x`.
pub fn text(value: Option<&str>) -> String {
    let v = value.unwrap_or_default();
    match v.strip_prefix('@') {
        Some(rest) if rest.starts_with('@') => rest.to_string(),
        Some(rest) if let Some(key) = rest.strip_prefix(LANG) => key.to_string(),
        _ => v.to_string(),
    }
}

/// What `yog build` does to a screen before building it (and the design
/// view's own: `d:` values for bound ones): `<Screen.Resources>`
/// (`@Name` values, `<Style x:Key>` with `based_on`) applied — a style's
/// attributes on every element naming it, its own winning — and yog-kit's
/// elements as the widgets they become (`ToggleButton` → `Button`,
/// `ListView` → `ScrollView`, `ItemsControl`/`ContentPresenter` → a column
/// or row). Unknown names are left as they are (`yog check` reports them).
pub fn apply_resources(root: &mut YuiNode) {
    let read = match root.children.iter().find(|c| c.kind.ends_with(yog_ids::yui_markup::RESOURCES)) {
        // The designer shows what it can: problems are `yog check`'s to report.
        Some(res) => Resources::read(&res.children.iter().map(entry).collect::<Vec<_>>()),
        None => Resources::default(),
    };
    resolve(root, &read);
}

/// A resources' element as [`Resources::read`] takes it: its attributes in
/// name order (a map has none of its own).
fn entry(n: &YuiNode) -> Entry<'_> {
    let mut attrs: Vec<(&str, &str)> = n.attrs.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    attrs.sort();
    Entry { kind: &n.kind, line: 0, attrs }
}

/// `n` and its children with the resources applied, bound values as their
/// design-time ones, and yog-kit's elements as the widgets they become.
fn resolve(n: &mut YuiNode, res: &Resources) {
    use yog_ids::yui_markup::{BIND, BINDING, DESIGN};
    // Bound values are the game's to compute: in the designer an attribute
    // shows its design-time value (`d:text="Sample"`), else none — as if
    // unset (a text box shows its placeholder).
    let design: Vec<(String, String)> =
        n.attrs.iter().filter_map(|(k, v)| Some((k.strip_prefix(DESIGN)?.to_string(), v.clone()))).collect();
    n.attrs.retain(|k, v| !k.starts_with(DESIGN) && !(v.trim_start().starts_with(BIND) || v.trim_start().starts_with(BINDING)));
    n.attrs.extend(design);
    for v in n.attrs.values_mut() {
        if let Some(r) = local_name(v).and_then(|name| res.values.get(name)) {
            *v = r.clone();
        }
    }
    if let Some(name) = n.attrs.remove(yog_ids::yui_attr::STYLE)
        && let Some(attrs) = res.styles.get(name.trim().trim_start_matches(yog_ids::sep::TYPED))
    {
        for (k, v) in attrs {
            n.attrs.entry(k.clone()).or_insert_with(|| v.clone());
        }
    }
    if let Some(name) = n.attrs.get(yog_ids::yui_attr::X_NAME).cloned() {
        n.attrs.entry(yog_ids::yui_attr::ID.into()).or_insert(name);
    }
    n.kind = as_widget(n).to_string();
    for c in &mut n.children {
        resolve(c, res);
    }
}

/// What yog-kit's element `n` becomes: `ToggleButton` → `Button`, `ListView`
/// → `ScrollView`, `ItemsControl`/`ContentPresenter` → a column or row.
fn as_widget(n: &YuiNode) -> &str {
    use yog_ids::yui;
    let horizontal = n.attr(yog_ids::yui_attr::ORIENTATION).is_some_and(|o| o.trim() == HORIZONTAL);
    match n.kind.as_str() {
        yui::TOGGLE_BUTTON => yui::BUTTON,
        yui::LIST_VIEW => yui::SCROLL_VIEW,
        yui::ITEMS_CONTROL if horizontal => yui::ROW,
        yui::ITEMS_CONTROL | yui::CONTENT_PRESENTER => yui::COLUMN,
        k => k,
    }
}

/// An `ItemsControl`'s `orientation` laying its items in a row.
const HORIZONTAL: &str = "horizontal";

/// `<Screen theme>`'s defaults on every node that doesn't set the attribute
/// itself — as `yog build` applies them.
pub fn apply_theme(root: &mut YuiNode) {
    let Some(theme) = root.attr(yog_ids::yui_attr::THEME).and_then(yog_ui::theme::Theme::builtin) else {
        return;
    };
    fn walk(n: &mut YuiNode, theme: &yog_ui::theme::Theme) {
        for (k, v) in theme.attrs(&n.kind) {
            n.attrs.entry(k.clone()).or_insert_with(|| v.clone());
        }
        for c in &mut n.children {
            walk(c, theme);
        }
    }
    walk(root, &theme);
}

/// Builds the widget for `node`. `in_canvas`: the parent places children
/// absolutely. `scroll`: preview scroll offsets by uid.
pub fn widget(node: &YuiNode, in_canvas: bool, scroll: &HashMap<String, f32>) -> Widget {
    let a = |k: &str| node.attr(k);
    let mut w = grid_and_cell(of_kind(node), node);
    let (x, y) = (num(a(yog_ids::yui_attr::X)), num(a(yog_ids::yui_attr::Y)));
    if in_canvas || x.is_some() || y.is_some() {
        w = w.at(x.unwrap_or(0.0), y.unwrap_or(0.0));
    }
    // Attribute order doesn't matter for these builder calls but follows the file's, like the generator.
    let mut keys: Vec<&String> = node.attrs.keys().collect();
    keys.sort();
    for key in keys {
        w = attribute(w, node, key);
    }
    if node.kind == yog_ids::yui::SCROLL_VIEW
        && let Some(offset) = scroll.get(&node.uid)
    {
        w = w.scroll_y(*offset);
    }
    if is_container(&node.kind) {
        let canvas = matches!(node.kind.as_str(), yog_ids::yui::SCREEN | yog_ids::yui::CANVAS);
        for child in widget_children(node) {
            w = w.child(widget(child, canvas, scroll));
        }
    }
    w
}

/// The widget `node`'s kind makes, before its attributes.
fn of_kind(node: &YuiNode) -> Widget {
    let a = |k: &str| node.attr(k);
    match node.kind.as_str() {
        yog_ids::yui::SCREEN | yog_ids::yui::CANVAS | yog_ids::yui::COLUMN => widget::panel(FlexDir::Column),
        yog_ids::yui::ROW => widget::panel(FlexDir::Row),
        yog_ids::yui::GRID => widget::grid(),
        yog_ids::yui::SCROLL_VIEW => widget::scroll_view(),
        yog_ids::yui::LABEL => widget::label(text(a(yog_ids::yui_attr::TEXT))),
        yog_ids::yui::BUTTON => widget::button(text(a(yog_ids::yui_attr::TEXT))),
        yog_ids::yui::ITEM_SLOT => widget::item_slot(a(yog_ids::yui_attr::ITEM).unwrap_or_default()),
        // A recipe layout's places (yog_formats::book::layout): a slot showing the
        // design-time sample (`d:item`), the count `d:text` or 2.
        yog_ids::yui::RECIPE_SLOT | yog_ids::yui::RECIPE_RESULT => widget::item_slot(a(yog_ids::yui_attr::ITEM).unwrap_or_default()),
        yog_ids::yui::RECIPE_COUNT => widget::label(a(yog_ids::yui_attr::TEXT).unwrap_or("2")),
        yog_ids::yui::INV_SLOT => widget::inv_slot(
            a(yog_ids::yui_attr::ITEM).unwrap_or_default(),
            a(yog_ids::yui_attr::COUNT).and_then(|c| c.trim().parse().ok()).unwrap_or(1),
        ),
        yog_ids::yui::SLOT => widget::slot(a(yog_ids::yui_attr::INDEX).and_then(|i| i.trim().parse().ok()).unwrap_or(0)),
        yog_ids::yui::PLAYER_INVENTORY => widget::player_inventory(),
        yog_ids::yui::IMAGE => super::images::widget(node),
        // Drawn by the mod's code in game: an empty box here.
        yog_ids::yui::SURFACE => widget::surface(a(yog_ids::yui_attr::SOURCE).unwrap_or_default()),
        yog_ids::yui::TEXT_BOX => widget::text_box(text(a(yog_ids::yui_attr::TEXT))),
        _ => widget::spacer(),
    }
}

/// A screen's size, a grid's tracks and spacing, a grid child's cell and span.
fn grid_and_cell(mut w: Widget, node: &YuiNode) -> Widget {
    let a = |k: &str| node.attr(k);
    if node.kind == yog_ids::yui::SCREEN {
        if let Some(v) = num(a(yog_ids::yui_attr::WIDTH)) {
            w = w.w(Units::Px(v));
        }
        if let Some(v) = num(a(yog_ids::yui_attr::HEIGHT)) {
            w = w.h(Units::Px(v));
        }
    }
    if node.kind == yog_ids::yui::GRID {
        if let Some(rows) = tracks(node, true) {
            w = w.rows(rows);
        }
        if let Some(cols) = tracks(node, false) {
            w = w.cols(cols);
        }
        if let Some(n) = num(a(yog_ids::yui_attr::ROW_SPACING)) {
            w = w.row_spacing(n);
        }
        if let Some(n) = num(a(yog_ids::yui_attr::COLUMN_SPACING)) {
            w = w.col_spacing(n);
        }
    }
    let (row, col) = (cell_index(a(yog_ids::yui_attr::GRID_ROW)), cell_index(a(yog_ids::yui_attr::GRID_COLUMN)));
    if row.is_some() || col.is_some() {
        w = w.cell(row.unwrap_or(0), col.unwrap_or(0));
    }
    let (rs, cs) = (
        cell_index(a(yog_ids::yui_attr::GRID_ROWSPAN)),
        cell_index(a(yog_ids::yui_attr::GRID_COLUMNSPAN)),
    );
    if rs.is_some() || cs.is_some() {
        w = w.span(rs.unwrap_or(1), cs.unwrap_or(1));
    }
    w
}

/// Attribute `key` of `node` on its widget: the common ones as yog-ui reads
/// them, then an id, events and texts (malformed values are left out:
/// `yog check` reports them).
fn attribute(w: Widget, node: &YuiNode, key: &str) -> Widget {
    use yog_ids::yui_attr as at;
    let v = node.attrs[key].as_str();
    if let Ok(Some(attr)) = Attr::parse(key, v) {
        return attr.apply(w);
    }
    match key {
        at::ID if node.kind != yog_ids::yui::SCREEN => w.id(v),
        at::ON_CLICK => w.on_click(v),
        at::ON_CHANGE => w.on_change(v),
        at::ON_SUBMIT => w.on_submit(v),
        at::FONT if !v.trim().is_empty() => w.font(v.trim()),
        at::PLACEHOLDER => w.placeholder(text(Some(v))),
        _ => w,
    }
}

/// The anchored screen (`root_with`): a `screen_w × screen_h` panel with the
/// content placed per `anchor`.
/// All of the window, as a percentage.
const WHOLE: f32 = 100.0;

pub fn screen(
    root: &YuiNode,
    screen_w: f32,
    screen_h: f32,
    scroll: &HashMap<String, f32>,
) -> Widget {
    let width = num(root.attr(yog_ids::yui_attr::WIDTH)).unwrap_or(widget::SCREEN_W);
    let height = num(root.attr(yog_ids::yui_attr::HEIGHT)).unwrap_or(widget::SCREEN_H);
    let content = widget(root, false, scroll);
    let cx = ((screen_w - width) / 2.0).floor();
    let cy = ((screen_h - height) / 2.0).floor();
    let anchor = root.attr(yog_ids::yui_attr::ANCHOR).unwrap_or("center").trim();
    let content = if anchor == "fill" {
        content
            .w(Units::Percent(WHOLE))
            .h(Units::Percent(WHOLE))
            .at(0.0, 0.0)
    } else {
        let (ax, ay) = match anchor {
            "top_left" => (0.0, 0.0),
            "top" => (cx, 0.0),
            "top_right" => (screen_w - width, 0.0),
            "left" => (0.0, cy),
            "right" => (screen_w - width, cy),
            "bottom_left" => (0.0, screen_h - height),
            "bottom" => (cx, screen_h - height),
            "bottom_right" => (screen_w - width, screen_h - height),
            _ => (cx, cy),
        };
        content.at(ax.max(0.0), ay.max(0.0))
    };
    widget::panel(FlexDir::Column)
        .w(Units::Px(screen_w))
        .h(Units::Px(screen_h))
        .child(content)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(kind: &str, attrs: &[(&str, &str)], children: Vec<YuiNode>) -> YuiNode {
        YuiNode {
            uid: kind.into(),
            kind: kind.into(),
            attrs: attrs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
            children,
        }
    }

    #[test]
    fn styles_resources_and_kit_elements_apply_as_yog_build_does() {
        let mut root = node(
            "Screen",
            &[],
            vec![
                node(
                    "Screen.Resources",
                    &[],
                    vec![
                        node("Color", &[("x:Key", "Accent"), ("value", "#FF112233")], vec![]),
                        node("Style", &[("x:Key", "Base"), ("target", "Button"), ("h", "16")], vec![]),
                        node("Style", &[("x:Key", "Filter"), ("based_on", "@Base"), ("w", "58"), ("color", "@Accent")], vec![]),
                    ],
                ),
                node("ToggleButton", &[("style", "@Filter"), ("w", "64"), ("text", "{x:Bind label()}"), ("d:text", "Yog")], vec![]),
                node("TextBox", &[("text", "{x:Bind search, Mode=TwoWay}")], vec![]),
            ],
        );
        apply_resources(&mut root);
        let b = &root.children[1];
        assert_eq!(b.kind, "Button");
        assert_eq!(b.attr("w"), Some("64"), "its own attribute wins");
        assert_eq!(b.attr("h"), Some("16"), "from the base style");
        assert_eq!(b.attr("color"), Some("#FF112233"));
        assert_eq!(b.attr("text"), Some("Yog"), "the design-time value");
        assert_eq!(root.children[2].attr("text"), None, "a bound value shows nothing");
    }
}
