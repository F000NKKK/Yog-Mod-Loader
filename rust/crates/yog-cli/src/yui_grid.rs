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

const ATTACHED: &[&str] = &["Grid.Row", "Grid.Column", "Grid.RowSpan", "Grid.ColumnSpan"];

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
fn compact(el: &Element, key: &str, value: &str, what: &str) -> Result<Vec<String>, String> {
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
                format!(
                    "line {}: <{}> {key}=\"{value}\": `{}` isn't a {what} size — expected Auto, *, 2* or pixels (26)",
                    el.line,
                    el.name,
                    part.trim()
                )
            })
        })
        .collect()
}

/// `<Grid.RowDefinitions>` → track expressions.
fn elements(prop: &Element, row: bool) -> Result<Vec<String>, String> {
    let (def, size, min, max) = if row {
        ("RowDefinition", "height", "min_height", "max_height")
    } else {
        ("ColumnDefinition", "width", "min_width", "max_width")
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
            format!(
                "line {}: <{def}> {size}=\"{v}\": expected Auto, *, 2* or pixels (26)",
                d.line
            )
        })?;
        for (key, call) in [(min, "min"), (max, "max")] {
            if let Some(v) = d.attr(key) {
                let n = num(v).ok_or_else(|| {
                    format!(
                        "line {}: <{def}> {key}=\"{v}\": expected a number of pixels",
                        d.line
                    )
                })?;
                t.push_str(&format!(".{call}({n:?})"));
            }
        }
        out.push(t);
    }
    Ok(out)
}

fn index(el: &Element, key: &str, v: &str, min: u16) -> Result<u16, String> {
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
            format!(
                "line {}: <{}> {key}=\"{v}\": expected {what}",
                el.line, el.name
            )
        })
}

fn plural(n: usize, what: &str) -> String {
    if n == 1 {
        format!("1 {what}")
    } else {
        format!("{n} {what}s")
    }
}

/// Checks and lowers every `<Grid>` and `Grid.*` attached property under
/// `el` (see the module docs).
pub fn normalize(el: &mut Element) -> Result<(), String> {
    let is_grid = el.name == yog_ids::yui::GRID;
    let (mut rows, mut cols): (Option<Vec<String>>, Option<Vec<String>>) = (None, None);
    let mut kept = Vec::with_capacity(el.children.len());
    for child in std::mem::take(&mut el.children) {
        let which = match child.name.as_str() {
            "Grid.RowDefinitions" => Some(true),
            "Grid.ColumnDefinitions" => Some(false),
            n if n.starts_with("Grid.") => {
                return err(
                    &child,
                    "isn't a Grid property (expected <Grid.RowDefinitions> or <Grid.ColumnDefinitions>)",
                );
            }
            _ => None,
        };
        let Some(row) = which else {
            kept.push(child);
            continue;
        };
        if !is_grid {
            return err(
                &child,
                format_args!("only goes directly inside a <Grid>, not <{}>", el.name),
            );
        }
        let slot = if row { &mut rows } else { &mut cols };
        if slot.is_some() {
            return err(&child, "is given twice");
        }
        *slot = Some(elements(&child, row)?);
    }
    el.children = kept;

    let mut calls = Vec::new();
    if is_grid {
        let mut attrs = Vec::new();
        for (k, v) in std::mem::take(&mut el.attrs) {
            match k.as_str() {
                "rows" | "columns" => {
                    let row = k == "rows";
                    let slot = if row { &mut rows } else { &mut cols };
                    if slot.is_some() {
                        let prop = if row {
                            "Grid.RowDefinitions"
                        } else {
                            "Grid.ColumnDefinitions"
                        };
                        return err(
                            el,
                            format_args!("{k}=\"...\" and <{prop}> both define the {k} — keep one"),
                        );
                    }
                    *slot = Some(compact(el, &k, &v, if row { "row" } else { "column" })?);
                }
                "row_spacing" | "column_spacing" => {
                    let n = num(&v).ok_or_else(|| {
                        format!(
                            "line {}: <Grid> {k}=\"{v}\": expected a number of pixels",
                            el.line
                        )
                    })?;
                    let m = if k == "row_spacing" {
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
        if let Some(r) = &rows {
            calls.push(format!("rows([{}])", r.join(", ")));
        }
        if let Some(c) = &cols {
            calls.push(format!("cols([{}])", c.join(", ")));
        }
    }
    push_calls(el, calls, "g");

    let (n_rows, n_cols) = (
        rows.as_ref().map_or(1, Vec::len),
        cols.as_ref().map_or(1, Vec::len),
    );
    for child in &mut el.children {
        let mut cell = [0u16, 0, 1, 1];
        let mut any = false;
        let mut attrs = Vec::new();
        for (k, v) in std::mem::take(&mut child.attrs) {
            let Some(i) = ATTACHED.iter().position(|a| *a == k) else {
                if k.starts_with("Grid.") {
                    return err(
                        child,
                        format_args!("has no attribute `{k}` (attached: {})", ATTACHED.join(", ")),
                    );
                }
                attrs.push((k, v));
                continue;
            };
            if !is_grid {
                return err(
                    child,
                    format_args!(
                        "{k}: only children of a <Grid> have grid cells (this is in a <{}>)",
                        el.name
                    ),
                );
            }
            if v.trim_start().starts_with('{') {
                return err(
                    child,
                    format_args!("{k}=\"{v}\": grid cells can't be bound"),
                );
            }
            cell[i] = index(child, &k, &v, if i < 2 { 0 } else { 1 })?;
            any = true;
        }
        child.attrs = attrs;
        if !is_grid {
            continue;
        }
        let [r, c, rs, cs] = cell;
        for (start, span, n, what, key) in [
            (r, rs, n_rows, "row", "Grid.Row"),
            (c, cs, n_cols, "column", "Grid.Column"),
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
    }
    for child in &mut el.children {
        normalize(child)?;
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

    fn codegen(body: &str) -> Result<String, String> {
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
        assert!(e(r#"<Grid rows="abc"/>"#).contains("`abc` isn't a row size"));
        assert!(e(r#"<Grid><Grid.RowDefinitions><RowDefinition Height="Auto"/></Grid.RowDefinitions></Grid>"#)
            .contains("lowercase: height"));
        assert!(e(
            r#"<Grid><Grid.RowDefinitions><RowDefinition height="x"/></Grid.RowDefinitions></Grid>"#
        )
        .contains("height=\"x\""));
        assert!(
            e(r#"<Grid rows="*,*"><Label Grid.Row="2"/></Grid>"#)
                .contains("past the grid's last row (it has 2 rows")
        );
        assert!(
            e(r#"<Grid columns="*,*"><Label Grid.Column="1" Grid.ColumnSpan="2"/></Grid>"#)
                .contains("Grid.ColumnSpan=\"2\" from column 1")
        );
        assert!(e(r#"<Grid><Label Grid.Row="1"/></Grid>"#).contains("it has 1 row,"));
        assert!(
            e(r#"<Column><Label Grid.Row="0"/></Column>"#).contains("only children of a <Grid>")
        );
        assert!(e(r#"<Grid rows="*"><Grid.RowDefinitions/></Grid>"#).contains("keep one"));
        assert!(
            e(r#"<Column><Grid.RowDefinitions/></Column>"#).contains("directly inside a <Grid>")
        );
        assert!(e(r#"<Grid><Label Grid.RowSpan="0"/></Grid>"#).contains("at least 1"));
        assert!(e(r#"<Grid><Label HorizontalAlignment="Left"/></Grid>"#).contains("halign"));
        assert!(
            e(r#"<Grid><Label halign="middle"/></Grid>"#).contains("left, center, right, stretch")
        );
    }
}
