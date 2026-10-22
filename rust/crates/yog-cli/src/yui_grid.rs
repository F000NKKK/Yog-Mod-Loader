//! `<Grid>` in `.yui` — WPF's `Grid`: rows and columns, children placed in
//! cells with attached `Grid.Row` / `Grid.Column` / `Grid.RowSpan` /
//! `Grid.ColumnSpan`.
//!
//! ```xml
//! <Grid rows="Auto,*,26" columns="160,*" row_spacing="2" column_spacing="4">
//!   <Label Grid.ColumnSpan="2" text="Title"/>
//!   <ScrollView Grid.Row="1"/>
//!   <Label Grid.Row="1" Grid.Column="1" text="Details" valign="top"/>
//! </Grid>
//! ```
//!
//! Definitions with limits use WPF's property-element form:
//!
//! ```xml
//! <Grid>
//!   <Grid.ColumnDefinitions>
//!     <ColumnDefinition width="2*" min_width="80"/>
//!     <ColumnDefinition width="*" max_width="200"/>
//!   </Grid.ColumnDefinitions>
//! </Grid>
//! ```
//!
//! [`normalize`] runs on the parsed document before any generator: it
//! checks all of this and turns it into builder calls (raw-call attributes),
//! so the screen, view-model and component generators need no Grid logic.

use crate::yui::{Element, RAW_CALL, err};
use yog_data::Diagnostic;
use yog_ids::{yui, yui_attr as at};

const ATTACHED: &[&str] = &[
    at::GRID_ROW,
    at::GRID_COLUMN,
    at::GRID_ROWSPAN,
    at::GRID_COLUMNSPAN,
];

fn num(v: &str) -> Option<f32> {
    v.trim()
        .parse::<f32>()
        .ok()
        .filter(|n| n.is_finite() && *n >= 0.0)
}

/// One row/column size: `Auto`, `*`, `2*`, `26` → a `Track` expression.
fn length(v: &str) -> Option<String> {
    let v = v.trim();
    if v.eq_ignore_ascii_case("auto") {
        return Some("::yog_api::ui::Track::auto()".into());
    }
    if let Some(w) = v.strip_suffix('*') {
        let w = if w.trim().is_empty() {
            Some(1.0)
        } else {
            num(w)
        }?;
        return Some(format!("::yog_api::ui::Track::star({w:?})"));
    }
    num(v).map(|px| format!("::yog_api::ui::Track::px({px:?})"))
}

/// `rows="Auto,*,26"` → track expressions.
fn compact(el: &Element, key: &str, value: &str, what: &str) -> Result<Vec<String>, Diagnostic> {
    if value.trim_start().starts_with('{') {
        return err(
            el,
            format_args!("{key}=\"{value}\": grid definitions can't be bound"),
        );
    }
    value
        .split(',')
        .map(|part| {
            length(part).ok_or_else(|| {
                Diagnostic::from(format!("<{}> {key}=\"{value}\": `{}` isn't a {what} size — expected Auto, *, 2* or pixels (26)", el.name, part.trim())).at_line(el.line)
            })
        })
        .collect()
}

/// `<Grid.RowDefinitions>` → track expressions.
fn elements(prop: &Element, row: bool) -> Result<Vec<String>, Diagnostic> {
    let (def, size, min, max) = if row {
        (
            yui::ROW_DEFINITION,
            at::HEIGHT,
            at::MIN_HEIGHT,
            at::MAX_HEIGHT,
        )
    } else {
        (
            yui::COLUMN_DEFINITION,
            at::WIDTH,
            at::MIN_WIDTH,
            at::MAX_WIDTH,
        )
    };
    if !prop.attrs.is_empty() {
        return err(prop, "takes no attributes");
    }
    let mut out = Vec::new();
    for d in &prop.children {
        if d.name != def {
            return err(
                d,
                format_args!("isn't allowed in <{}> (expected <{def}>)", prop.name),
            );
        }
        if !d.children.is_empty() {
            return err(d, "can't have children");
        }
        for (k, _) in &d.attrs {
            if ![size, min, max].contains(&k.as_str()) {
                let lower = k
                    .chars()
                    .next()
                    .is_some_and(char::is_uppercase)
                    .then(|| crate::yui::snake(k));
                return match lower.filter(|l| [size, min, max].contains(&l.as_str())) {
                    Some(l) => err(
                        d,
                        format_args!("has no attribute `{k}` — .yui attributes are lowercase: {l}"),
                    ),
                    None => err(
                        d,
                        format_args!("has no attribute `{k}` (expected {size}, {min}, {max})"),
                    ),
                };
            }
        }
        let v = d.attr(size).unwrap_or("*");
        let mut t = length(v).ok_or_else(|| {
            Diagnostic::from(format!(
                "<{def}> {size}=\"{v}\": expected Auto, *, 2* or pixels (26)"
            ))
            .at_line(d.line)
        })?;
        for (key, call) in [(min, "min"), (max, "max")] {
            if let Some(v) = d.attr(key) {
                let n = num(v).ok_or_else(|| {
                    Diagnostic::from(format!(
                        "<{def}> {key}=\"{v}\": expected a number of pixels"
                    ))
                    .at_line(d.line)
                })?;
                t.push_str(&format!(".{call}({n:?})"));
            }
        }
        out.push(t);
    }
    Ok(out)
}

fn index(el: &Element, key: &str, v: &str, min: u16) -> Result<u16, Diagnostic> {
    v.trim()
        .parse::<u16>()
        .ok()
        .filter(|n| *n >= min)
        .ok_or_else(|| {
            let what = if min == 0 {
                "a whole number (0 = the first)"
            } else {
                "a whole number, at least 1"
            };
            Diagnostic::from(format!("<{}> {key}=\"{v}\": expected {what}", el.name))
                .at_line(el.line)
        })
}

fn plural(n: usize, what: &str) -> String {
    if n == 1 {
        format!("1 {what}")
    } else {
        format!("{n} {what}s")
    }
}

/// A grid's rows and columns as builder arguments, when it says them.
#[derive(Default)]
struct Tracks {
    rows: Option<Vec<String>>,
    cols: Option<Vec<String>>,
}

impl Tracks {
    fn slot(&mut self, row: bool) -> &mut Option<Vec<String>> {
        if row { &mut self.rows } else { &mut self.cols }
    }
}

/// What a bound value starts with: `{x:Bind …}`.
const BINDING_START: char = '{';
/// What a grid's member properties start with: `Grid.`.
const GRID_MEMBER: &str = yog_ids::concatcp!(yui::GRID, yog_ids::sep::MEMBER);

/// Checks and lowers every `<Grid>` and `Grid.*` attached property under
/// `el` (see the module docs).
pub fn normalize(el: &mut Element) -> Result<(), Diagnostic> {
    let is_grid = el.name == yui::GRID;
    let mut tracks = definitions(el, is_grid)?;
    if is_grid {
        let calls = grid_calls(el, &mut tracks)?;
        push_calls(el, calls, "g");
    }
    let size = (
        tracks.rows.as_ref().map_or(1, Vec::len),
        tracks.cols.as_ref().map_or(1, Vec::len),
    );
    let name = el.name.clone();
    for child in &mut el.children {
        cell(child, is_grid.then_some(size), &name)?;
    }
    for child in &mut el.children {
        normalize(child)?;
    }
    Ok(())
}

/// `<Grid.RowDefinitions>` / `<Grid.ColumnDefinitions>` taken out of
/// `el`'s children (only a grid has them, once each).
fn definitions(el: &mut Element, is_grid: bool) -> Result<Tracks, Diagnostic> {
    let mut tracks = Tracks::default();
    let mut kept = Vec::with_capacity(el.children.len());
    for child in std::mem::take(&mut el.children) {
        let row = match child.name.as_str() {
            yui::ROW_DEFINITIONS => true,
            yui::COLUMN_DEFINITIONS => false,
            n if n.starts_with(GRID_MEMBER) => {
                return err(
                    &child,
                    "isn't a Grid property (expected <Grid.RowDefinitions> or <Grid.ColumnDefinitions>)",
                );
            }
            _ => {
                kept.push(child);
                continue;
            }
        };
        if !is_grid {
            return err(
                &child,
                format_args!("only goes directly inside a <Grid>, not <{}>", el.name),
            );
        }
        if tracks.slot(row).is_some() {
            return err(&child, "is given twice");
        }
        *tracks.slot(row) = Some(elements(&child, row)?);
    }
    el.children = kept;
    Ok(tracks)
}

/// A grid's own attributes (`rows`, `columns`, spacings) as builder calls.
fn grid_calls(el: &mut Element, tracks: &mut Tracks) -> Result<Vec<String>, Diagnostic> {
    let mut calls = Vec::new();
    let mut attrs = Vec::new();
    for (k, v) in std::mem::take(&mut el.attrs) {
        match k.as_str() {
            at::ROWS | at::COLUMNS => {
                let row = k == at::ROWS;
                if tracks.slot(row).is_some() {
                    let prop = if row {
                        yui::ROW_DEFINITIONS
                    } else {
                        yui::COLUMN_DEFINITIONS
                    };
                    return err(
                        el,
                        format_args!("{k}=\"...\" and <{prop}> both define the {k} — keep one"),
                    );
                }
                *tracks.slot(row) = Some(compact(el, &k, &v, if row { "row" } else { "column" })?);
            }
            at::ROW_SPACING | at::COLUMN_SPACING => {
                let n = num(&v).ok_or_else(|| {
                    Diagnostic::from(format!("<Grid> {k}=\"{v}\": expected a number of pixels"))
                        .at_line(el.line)
                })?;
                let m = if k == at::ROW_SPACING {
                    "row_spacing"
                } else {
                    "col_spacing"
                };
                calls.push(format!("{m}({n:?})"));
            }
            _ => attrs.push((k, v)),
        }
    }
    el.attrs = attrs;
    if let Some(r) = &tracks.rows {
        calls.push(format!("rows([{}])", r.join(", ")));
    }
    if let Some(c) = &tracks.cols {
        calls.push(format!("cols([{}])", c.join(", ")));
    }
    Ok(calls)
}

/// A child's attached `Grid.*` properties: its cell (`grid`: the grid's
/// rows and columns; none: its parent isn't one, and the child can't say them).
fn cell(child: &mut Element, grid: Option<(usize, usize)>, parent: &str) -> Result<(), Diagnostic> {
    let mut cell = [0u16, 0, 1, 1];
    let mut any = false;
    let mut attrs = Vec::new();
    for (k, v) in std::mem::take(&mut child.attrs) {
        let Some(i) = ATTACHED.iter().position(|a| *a == k) else {
            if k.starts_with(GRID_MEMBER) {
                return err(
                    child,
                    format_args!("has no attribute `{k}` (attached: {})", ATTACHED.join(", ")),
                );
            }
            attrs.push((k, v));
            continue;
        };
        if grid.is_none() {
            return err(
                child,
                format_args!(
                    "{k}: only children of a <Grid> have grid cells (this is in a <{parent}>)"
                ),
            );
        }
        if v.trim_start().starts_with(BINDING_START) {
            return err(
                child,
                format_args!("{k}=\"{v}\": grid cells can't be bound"),
            );
        }
        cell[i] = index(child, &k, &v, if i < 2 { 0 } else { 1 })?;
        any = true;
    }
    child.attrs = attrs;
    let Some((n_rows, n_cols)) = grid else {
        return Ok(());
    };
    let [r, c, rs, cs] = cell;
    for (start, span, n, what, key) in [
        (r, rs, n_rows, "row", at::GRID_ROW),
        (c, cs, n_cols, "column", at::GRID_COLUMN),
    ] {
        let (start, span) = (start as usize, span as usize);
        if start >= n {
            return err(
                child,
                format_args!(
                    "{key}=\"{start}\" is past the grid's last {what} (it has {}, numbered from 0)",
                    plural(n, what)
                ),
            );
        }
        if start + span > n {
            return err(
                child,
                format_args!(
                    "{key}Span=\"{span}\" from {what} {start} runs past the grid's last {what} (it has {})",
                    plural(n, what)
                ),
            );
        }
    }
    if any {
        let mut calls = vec![format!("cell({r}, {c})")];
        if (rs, cs) != (1, 1) {
            calls.push(format!("span({rs}, {cs})"));
        }
        push_calls(child, calls, "c");
    }
    Ok(())
}

/// Adds builder calls as raw-call attributes (see [`RAW_CALL`]).
fn push_calls(el: &mut Element, calls: Vec<String>, tag: &str) {
    for (i, c) in calls.into_iter().enumerate() {
        el.attrs.push((format!("{RAW_CALL}{tag}{i}"), c));
    }
}

#[cfg(test)]
mod tests {
    use crate::yui;
    use yog_data::Diagnostic;

    fn codegen(body: &str) -> Result<String, Diagnostic> {
        yui::generate(
            &format!("<Screen id=\"m:s\" width=\"100\" height=\"100\">{body}</Screen>"),
            "s",
        )
    }

    #[test]
    fn compact_and_element_forms() {
        let code = codegen(
            r#"<Grid rows="Auto, *, 26" row_spacing="2" column_spacing="4">
                 <Grid.ColumnDefinitions>
                   <ColumnDefinition width="2*" min_width="80"/>
                   <ColumnDefinition max_width="200"/>
                 </Grid.ColumnDefinitions>
                 <Label text="a" Grid.Row="1" Grid.Column="1" halign="right" margin="2 4"/>
                 <Label text="b" Grid.RowSpan="3"/>
                 <Label text="c"/>
               </Grid>"#,
        )
        .unwrap();
        assert!(code.contains("widget::grid()"), "{code}");
        assert!(code.contains(".row_spacing(2.0)"), "{code}");
        assert!(code.contains(".col_spacing(4.0)"), "{code}");
        assert!(
            code.contains(".rows([::yog_api::ui::Track::auto(), ::yog_api::ui::Track::star(1.0), ::yog_api::ui::Track::px(26.0)])"),
            "{code}"
        );
        assert!(
            code.contains(".cols([::yog_api::ui::Track::star(2.0).min(80.0), ::yog_api::ui::Track::star(1.0).max(200.0)])"),
            "{code}"
        );
        assert!(code.contains(".cell(1, 1)"), "{code}");
        assert!(
            code.contains(".halign(::yog_api::ui::HAlign::Right)"),
            "{code}"
        );
        assert!(code.contains(".margin(2.0, 4.0, 2.0, 4.0)"), "{code}");
        assert!(
            code.contains(".cell(0, 0)\n") && code.contains(".span(3, 1)"),
            "{code}"
        );
    }

    #[test]
    fn errors() {
        let e = |body: &str| codegen(body).unwrap_err();
        assert!(
            e(r#"<Grid rows="abc"/>"#)
                .message
                .contains("`abc` isn't a row size")
        );
        assert!(e(r#"<Grid><Grid.RowDefinitions><RowDefinition Height="Auto"/></Grid.RowDefinitions></Grid>"#)
            .message.contains("lowercase: height"));
        assert!(e(
            r#"<Grid><Grid.RowDefinitions><RowDefinition height="x"/></Grid.RowDefinitions></Grid>"#
        )
        .message.contains("height=\"x\""));
        assert!(
            e(r#"<Grid rows="*,*"><Label Grid.Row="2"/></Grid>"#)
                .message
                .contains("past the grid's last row (it has 2 rows")
        );
        assert!(
            e(r#"<Grid columns="*,*"><Label Grid.Column="1" Grid.ColumnSpan="2"/></Grid>"#)
                .message
                .contains("Grid.ColumnSpan=\"2\" from column 1")
        );
        assert!(
            e(r#"<Grid><Label Grid.Row="1"/></Grid>"#)
                .message
                .contains("it has 1 row,")
        );
        assert!(
            e(r#"<Column><Label Grid.Row="0"/></Column>"#)
                .message
                .contains("only children of a <Grid>")
        );
        assert!(
            e(r#"<Grid rows="*"><Grid.RowDefinitions/></Grid>"#)
                .message
                .contains("keep one")
        );
        assert!(
            e(r#"<Column><Grid.RowDefinitions/></Column>"#)
                .message
                .contains("directly inside a <Grid>")
        );
        assert!(
            e(r#"<Grid><Label Grid.RowSpan="0"/></Grid>"#)
                .message
                .contains("at least 1")
        );
        assert!(
            e(r#"<Grid><Label HorizontalAlignment="Left"/></Grid>"#)
                .message
                .contains("halign")
        );
        assert!(
            e(r#"<Grid><Label halign="middle"/></Grid>"#)
                .message
                .contains("left, center, right, stretch")
        );
    }
}
