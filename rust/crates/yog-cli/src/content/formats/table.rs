//! `.ydef` tables (`kind: "table"`): a mod's own rows of declared columns,
//! checked here and made an enum in the mod's code (`yog_formats::def::TableDef`).

use std::fmt::Write as _;
use yog_data::Diagnostic;

use crate::content::refs::{self, RefKind};
use yog_data::{Path as At, Value};
use yog_formats::def::TableDef;

use crate::yres;

/// A value a column holds.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Scalar {
    Bool,
    /// An integer of this Rust type.
    Int(&'static str),
    Float(&'static str),
    Str,
    /// A `.yres` string: its translation.
    Text,
    /// A `.yres` color.
    Color,
    Item,
    Block,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Column {
    One(Scalar),
    List(Scalar),
    Map(Scalar, Scalar),
}

/// A table row: its id, its variant name, its values.
type TableRow = (String, String, Vec<String>);

fn scalar(s: &str) -> Option<Scalar> {
    Some(match s.trim() {
        "bool" => Scalar::Bool,
        "u8" => Scalar::Int("u8"),
        "u16" => Scalar::Int("u16"),
        "u32" => Scalar::Int("u32"),
        "u64" => Scalar::Int("u64"),
        "i32" => Scalar::Int("i32"),
        "i64" => Scalar::Int("i64"),
        "usize" => Scalar::Int("usize"),
        "f32" => Scalar::Float("f32"),
        "f64" => Scalar::Float("f64"),
        "string" => Scalar::Str,
        "text" => Scalar::Text,
        "color" => Scalar::Color,
        yog_ids::kind::ITEM => Scalar::Item,
        yog_ids::kind::BLOCK => Scalar::Block,
        _ => return None,
    })
}

fn column(s: &str) -> Result<Column, String> {
    let s = s.trim();
    let bad = || {
        format!(
            "`{s}`: a column is bool, u8…u64, i32, i64, usize, f32, f64, string, text, color, item, block, [<one>] or {{<one>: <one>}}"
        )
    };
    let col = if let Some(inner) = s.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
        Column::List(scalar(inner).ok_or_else(bad)?)
    } else if let Some(inner) = s.strip_prefix('{').and_then(|r| r.strip_suffix('}')) {
        let (k, v) = inner.split_once(':').ok_or_else(bad)?;
        Column::Map(scalar(k).ok_or_else(bad)?, scalar(v).ok_or_else(bad)?)
    } else {
        Column::One(scalar(s).ok_or_else(bad)?)
    };
    if matches!(
        col,
        Column::List(Scalar::Text) | Column::Map(Scalar::Text, _) | Column::Map(_, Scalar::Text)
    ) {
        return Err(format!(
            "`{s}`: a text is translated when asked for — one per column, not in a list"
        ));
    }
    Ok(col)
}

impl Scalar {
    fn rust(self) -> &'static str {
        match self {
            Scalar::Bool => "bool",
            Scalar::Int(t) | Scalar::Float(t) => t,
            Scalar::Str | Scalar::Item | Scalar::Block => "&'static str",
            Scalar::Text => "::std::string::String",
            Scalar::Color => "u32",
        }
    }

    /// `v` as a Rust expression of [`Self::rust`].
    fn expr(self, v: &Value, refs: &yres::Refs, known: &refs::Known) -> Result<String, String> {
        // `"@item:…"`, `"@Strings.…"`: written as strings, read as what they name.
        let typed;
        let v = match v {
            Value::Str(s) if s.starts_with('@') => {
                typed = yog_data::typed::classify(s)?;
                &typed
            }
            v => v,
        };
        let want = |what: &str| Err(format!("{}: want {what}", short(v)));
        match self {
            Scalar::Bool => v
                .as_bool()
                .map(|b| b.to_string())
                .map_or_else(|| want("true or false"), Ok),
            Scalar::Int(t) => {
                let Some(n) = v.as_i64() else {
                    return want("an integer");
                };
                let fits = match t {
                    "u8" => u8::try_from(n).is_ok(),
                    "u16" => u16::try_from(n).is_ok(),
                    "u32" => u32::try_from(n).is_ok(),
                    "u64" | "usize" => n >= 0,
                    "i32" => i32::try_from(n).is_ok(),
                    _ => true,
                };
                if fits {
                    Ok(format!("{n}"))
                } else {
                    Err(format!("{n} doesn't fit a {t}"))
                }
            }
            Scalar::Float(_) => v
                .as_f64()
                .map(|f| format!("{f:?}"))
                .map_or_else(|| want("a number"), Ok),
            Scalar::Str => v
                .as_str()
                .map(|s| format!("{s:?}"))
                .map_or_else(|| want("a string"), Ok),
            Scalar::Text => match v {
                Value::Res(r) => refs
                    .text(r)
                    .map(|(key, _)| format!("::yog_api::tr({key:?})")),
                _ => want("a text, `@Strings.<key>`"),
            },
            Scalar::Color => match v {
                Value::Res(r) => refs.color(r).map(|c| format!("0x{c:08X}")),
                _ => want("a color, `@Colors.<key>`"),
            },
            Scalar::Item | Scalar::Block => {
                let (kind, word) = if self == Scalar::Item {
                    (RefKind::Item, yog_ids::kind::ITEM)
                } else {
                    (RefKind::Block, yog_ids::kind::BLOCK)
                };
                match v {
                    Value::Ref(r) if r.kind == word => {
                        known.check(kind, &r.id).map(|()| format!("{:?}", r.id))
                    }
                    _ => want(&format!("`@{word}:<id>`")),
                }
            }
        }
    }
}

/// A value, short, for an error.
fn short(v: &Value) -> String {
    let s = format!("{v:?}");
    if s.chars().count() > 40 {
        format!("{}…", s.chars().take(40).collect::<String>())
    } else {
        s
    }
}

/// `snake_case` (or `kebab-case`) → `PascalCase`.
fn pascal(s: &str) -> String {
    s.split(['_', '-', '/', '.'])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map(|f| f.to_ascii_uppercase().to_string() + c.as_str())
                .unwrap_or_default()
        })
        .collect()
}

/// A table, checked: what its enum is made of.
#[derive(Debug, Clone)]
pub(crate) struct Table {
    file: String,
    name: String,
    columns: Vec<(String, Column)>,
    /// (row id, variant, each column's expression).
    rows: Vec<(String, String, Vec<String>)>,
}

/// Column names every table's code has already (its id, index, lookup, list).
const RESERVED_COLUMNS: [&str; 4] = ["id", "index", "from_id", "all"];

/// A problem with the table's value at `keys` (`rows.wood.size`).
fn problem_at(keys: &[&str], message: impl std::fmt::Display) -> Diagnostic {
    let path = keys.iter().fold(At::root(), |at, k| at.key(k));
    Diagnostic::error(yog_ids::build_problem::CONTENT, message).at_path(path)
}

/// The table's fields problems are at.
const ID: &str = yog_data::field!(TableDef, id);
const COLUMNS: &str = yog_data::field!(TableDef, columns);
const ROWS: &str = yog_data::field!(TableDef, rows);

/// A table's row: its id, its variant's name, each column's expression.
type Row = (String, String, Vec<String>);

/// Checks a table; its problems, each at its path.
pub(crate) fn check(
    file: &str,
    t: TableDef,
    refs: &yres::Refs,
    known: &refs::Known,
) -> Result<Table, Vec<Diagnostic>> {
    let mut errors = Vec::new();
    let name = pascal(&t.id);
    if name.is_empty()
        || !name.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        || !name.chars().all(|c| c.is_ascii_alphanumeric())
    {
        errors.push(problem_at(
            &[ID],
            format!("`{}`: a name like `tier` or `power_level`", t.id),
        ));
    }
    let columns = check_columns(&t, &mut errors);
    let mut rows: Vec<Row> = Vec::new();
    for (id, v) in &t.rows.0 {
        if let Some(row) = check_row(id, v, &columns, (refs, known), &mut errors) {
            if rows.iter().any(|(_, variant, _)| *variant == row.1) {
                errors.push(problem_at(&[ROWS, id], "twice"));
            }
            rows.push(row);
        }
    }
    if rows.is_empty() {
        errors.push(problem_at(&[ROWS], "none"));
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    Ok(Table {
        file: file.to_string(),
        name,
        columns,
        rows,
    })
}

/// The table's columns, each with its type; a bad name or type is an error.
fn check_columns(t: &TableDef, errors: &mut Vec<Diagnostic>) -> Vec<(String, Column)> {
    let mut columns = Vec::new();
    for (c, ty) in &t.columns.0 {
        if !crate::content::valid_path(c)
            || c.contains('/')
            || c.starts_with(|ch: char| ch.is_ascii_digit())
        {
            errors.push(problem_at(&[COLUMNS, c], "a name like `world_size`"));
        } else if RESERVED_COLUMNS.contains(&c.as_str()) {
            errors.push(problem_at(&[COLUMNS, c], "taken (every table has it)"));
        }
        match column(ty) {
            Ok(col) => columns.push((c.clone(), col)),
            Err(e) => errors.push(problem_at(&[COLUMNS, c], e)),
        }
    }
    columns
}

/// Row `id`: its variant and each column's expression; `None` (an error
/// said) when it can't be a row at all.
fn check_row(
    id: &str,
    v: &Value,
    columns: &[(String, Column)],
    cx: (&yres::Refs, &refs::Known),
    errors: &mut Vec<Diagnostic>,
) -> Option<Row> {
    let variant = pascal(id);
    if !variant
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic())
    {
        errors.push(problem_at(&[ROWS, id], "a name like `wood`"));
        return None;
    }
    let Some(map) = v.as_map() else {
        errors.push(problem_at(&[ROWS, id], "a row is a map of its columns"));
        return None;
    };
    for (k, _) in map.iter() {
        if !columns.iter().any(|(c, _)| c == k) {
            errors.push(problem_at(&[ROWS, id, k], "no such column"));
        }
    }
    let mut exprs = Vec::new();
    for (c, col) in columns {
        let Some(cell) = map.get(c) else {
            errors.push(problem_at(&[ROWS, id, c], "missing"));
            continue;
        };
        match cell_expr(col, cell, cx) {
            Ok(e) => exprs.push(e),
            Err(e) => errors.push(problem_at(&[ROWS, id, c], e)),
        }
    }
    Some((id.to_string(), variant, exprs))
}

/// A cell of column `col` as Rust: a value, a list `&[…]`, or `&[(key, value)…]`.
fn cell_expr(
    col: &Column,
    cell: &Value,
    (refs, known): (&yres::Refs, &refs::Known),
) -> Result<String, String> {
    let list = |items: Vec<String>| format!("&[{}]", items.join(", "));
    match col {
        Column::One(s) => s.expr(cell, refs, known),
        Column::List(s) => {
            let items = cell.as_list().ok_or("want a list")?;
            items
                .iter()
                .map(|i| s.expr(i, refs, known))
                .collect::<Result<Vec<_>, _>>()
                .map(list)
        }
        Column::Map(k, val) => {
            let m = cell.as_map().ok_or("want a map")?;
            m.iter()
                .map(|(key, x)| {
                    Ok(format!(
                        "({}, {})",
                        k.expr(&map_key(k, key), refs, known)?,
                        val.expr(x, refs, known)?
                    ))
                })
                .collect::<Result<Vec<_>, String>>()
                .map(list)
        }
    }
}

/// A map cell's key as the value it stands for — written bare:
/// `minecraft:redstone`, not `@item:…`.
fn map_key(k: &Scalar, key: &str) -> Value {
    match k {
        Scalar::Item => Value::Ref(yog_data::Ref::new(yog_ids::kind::ITEM, key)),
        Scalar::Block => Value::Ref(yog_data::Ref::new(yog_ids::kind::BLOCK, key)),
        _ => yog_data::text::parse_value(key).unwrap_or_else(|_| Value::Str(key.to_string())),
    }
}

/// The enum a table makes.
pub(crate) fn rust(t: &Table) -> String {
    let mut code = String::new();
    let n = &t.name;
    // Read and written as Yog Data (a mod's stores, packets): by its rows' ids.
    let derive = ", ::yog_api::Schema";
    let _ = writeln!(
        code,
        "\n/// The `{}` table: a variant per row.\n#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord{derive})]\n#[allow(dead_code)]\npub enum {n} {{",
        t.file
    );
    for (_, v, _) in &t.rows {
        let _ = writeln!(code, "    {v},");
    }
    // Generated for every table: a mod uses some of it, so what it doesn't is no warning.
    code.push_str("}\n\n#[allow(dead_code, clippy::all)]\nimpl ");
    let _ = writeln!(
        code,
        "{n} {{\n    /// Every row, in order.\n    pub const ALL: [{n}; {}] = [{}];\n",
        t.rows.len(),
        t.rows
            .iter()
            .map(|(_, v, _)| format!("{n}::{v}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let arms = |f: &dyn Fn(usize, &TableRow) -> String| -> String {
        t.rows
            .iter()
            .enumerate()
            .map(|(i, r)| format!("            {n}::{} => {},\n", r.1, f(i, r)))
            .collect()
    };
    let _ = writeln!(
        code,
        "    /// Its row's id.\n    pub fn id(self) -> &'static str {{\n        match self {{\n{}        }}\n    }}\n",
        arms(&|_, r| format!("{:?}", r.0))
    );
    let _ = writeln!(
        code,
        "    pub fn from_id(id: &str) -> ::std::option::Option<{n}> {{\n        Self::ALL.into_iter().find(|r| r.id() == id)\n    }}\n"
    );
    let _ = writeln!(
        code,
        "    /// Its place in [`Self::ALL`].\n    pub fn index(self) -> usize {{\n        self as usize\n    }}"
    );
    for (ci, (c, col)) in t.columns.iter().enumerate() {
        let ty = match col {
            Column::One(s) => s.rust().to_string(),
            Column::List(s) => format!("&'static [{}]", s.rust()),
            Column::Map(k, v) => format!("&'static [({}, {})]", k.rust(), v.rust()),
        };
        let _ = writeln!(
            code,
            "\n    pub fn {c}(self) -> {ty} {{\n        match self {{\n{}        }}\n    }}",
            arms(&|_, r| r.2[ci].clone())
        );
        if let Column::One(Scalar::Item | Scalar::Block | Scalar::Str) = col {
            let _ = writeln!(
                code,
                "\n    /// The row whose `{c}` is `value`.\n    pub fn from_{c}(value: &str) -> ::std::option::Option<{n}> {{\n        Self::ALL.into_iter().find(|r| r.{c}() == value)\n    }}"
            );
        }
    }
    code.push_str("}\n");
    code
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(text: &str) -> Result<Table, Vec<Diagnostic>> {
        let doc = yog_data::text::parse(text).unwrap();
        let def = match yog_data::schema::from_doc::<yog_formats::def::Def>(text, &doc)
            .unwrap()
            .value
        {
            yog_formats::def::Def::Table(t) => t,
            _ => panic!("not a table"),
        };
        let mut refs = yres::Refs::default();
        refs.texts.insert(
            "Strings.tier_wood".into(),
            ("mymod.tier_wood".into(), "Wood".into()),
        );
        let mut known = refs::Known::new("mymod", &[], None);
        known.add(RefKind::Item, String::from("mymod:chip_wood"));
        check("content/tier.ydef", def, &refs, &known)
    }

    #[test]
    fn a_table_is_an_enum_with_a_method_per_column() {
        let t = table(
            r#"{ kind: "table", id: "tier",
                columns: { size: "u32", chip: "item", name: "text", inputs: "{item: u64}" },
                rows: {
                    wood: { size: 16, chip: "@item:mymod:chip_wood", name: "@Strings.tier_wood", inputs: { "minecraft:oak_log": 1 } },
                    stone: { size: 32, chip: "@item:mymod:chip_wood", name: "@Strings.tier_wood", inputs: {} },
                } }"#,
        )
        .unwrap();
        let code = rust(&t);
        assert!(
            code.contains("pub enum Tier {\n    Wood,\n    Stone,\n}"),
            "{code}"
        );
        assert!(code.contains("::yog_api::Schema"));
        assert!(code.contains("pub const ALL: [Tier; 2] = [Tier::Wood, Tier::Stone];"));
        assert!(code.contains("pub fn size(self) -> u32"));
        assert!(code.contains("Tier::Stone => 32,"));
        assert!(code.contains("pub fn from_chip(value: &str)"));
        assert!(code.contains("::yog_api::tr(\"mymod.tier_wood\")"));
        assert!(code.contains("&[(\"minecraft:oak_log\", 1)]"), "{code}");
    }

    #[test]
    fn rows_are_checked_against_the_columns() {
        let e = table(
            r#"{ kind: "table", id: "tier", columns: { size: "u8", chip: "item", odd: "matrix" },
                rows: { wood: { size: 300, chip: "@item:mymod:nope", extra: 1 } } }"#,
        )
        .unwrap_err();
        let all = e
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(all.contains("columns.odd"), "{all}");
        assert!(
            all.contains("rows.wood.size: 300 doesn't fit a u8"),
            "{all}"
        );
        assert!(all.contains("rows.wood.chip"), "{all}");
        assert!(all.contains("rows.wood.extra: no such column"), "{all}");
    }
}
