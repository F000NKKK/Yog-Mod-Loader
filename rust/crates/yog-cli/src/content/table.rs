//! `.ydef` tables (`kind: "table"`): a mod's own rows of declared columns,
//! checked here and made an enum in the mod's code (`yog_formats::def::TableDef`).

use std::fmt::Write as _;

use super::refs::{self, RefKind};
use yog_data::Value;
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
        "item" => Scalar::Item,
        "block" => Scalar::Block,
        _ => return None,
    })
}

fn column(s: &str) -> Result<Column, String> {
    let s = s.trim();
    let bad = || format!("`{s}`: a column is bool, u8…u64, i32, i64, usize, f32, f64, string, text, color, item, block, [<one>] or {{<one>: <one>}}");
    let col = if let Some(inner) = s.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
        Column::List(scalar(inner).ok_or_else(bad)?)
    } else if let Some(inner) = s.strip_prefix('{').and_then(|r| r.strip_suffix('}')) {
        let (k, v) = inner.split_once(':').ok_or_else(bad)?;
        Column::Map(scalar(k).ok_or_else(bad)?, scalar(v).ok_or_else(bad)?)
    } else {
        Column::One(scalar(s).ok_or_else(bad)?)
    };
    if matches!(col, Column::List(Scalar::Text) | Column::Map(Scalar::Text, _) | Column::Map(_, Scalar::Text)) {
        return Err(format!("`{s}`: a text is translated when asked for — one per column, not in a list"));
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
            Scalar::Bool => v.as_bool().map(|b| b.to_string()).map_or_else(|| want("true or false"), Ok),
            Scalar::Int(t) => {
                let Some(n) = v.as_i64() else { return want("an integer") };
                let fits = match t {
                    "u8" => u8::try_from(n).is_ok(),
                    "u16" => u16::try_from(n).is_ok(),
                    "u32" => u32::try_from(n).is_ok(),
                    "u64" | "usize" => n >= 0,
                    "i32" => i32::try_from(n).is_ok(),
                    _ => true,
                };
                if fits { Ok(format!("{n}")) } else { Err(format!("{n} doesn't fit a {t}")) }
            }
            Scalar::Float(_) => v.as_f64().map(|f| format!("{f:?}")).map_or_else(|| want("a number"), Ok),
            Scalar::Str => v.as_str().map(|s| format!("{s:?}")).map_or_else(|| want("a string"), Ok),
            Scalar::Text => match v {
                Value::Res(r) => match refs.texts.get(&format!("{}.{}", r.set, r.key)) {
                    Some((key, _)) => Ok(format!("::yog_api::tr({key:?})")),
                    None => Err(format!("@{}.{}: no such text", r.set, r.key)),
                },
                _ => want("a text, `@Strings.<key>`"),
            },
            Scalar::Color => match v {
                Value::Res(r) => match refs.colors.get(&format!("{}.{}", r.set, r.key)) {
                    Some(c) => Ok(format!("0x{c:08X}")),
                    None => Err(format!("@{}.{}: no such color", r.set, r.key)),
                },
                _ => want("a color, `@Colors.<key>`"),
            },
            Scalar::Item | Scalar::Block => {
                let (kind, word) = if self == Scalar::Item { (RefKind::Item, yog_ids::kind::ITEM) } else { (RefKind::Block, yog_ids::kind::BLOCK) };
                match v {
                    Value::Ref(r) if r.kind == word => known.check(kind, &r.id).map(|()| format!("{:?}", r.id)),
                    _ => want(&format!("`@{word}:<id>`")),
                }
            }
        }
    }
}

/// A value, short, for an error.
fn short(v: &Value) -> String {
    let s = format!("{v:?}");
    if s.chars().count() > 40 { format!("{}…", s.chars().take(40).collect::<String>()) } else { s }
}

/// `snake_case` (or `kebab-case`) → `PascalCase`.
fn pascal(s: &str) -> String {
    s.split(['_', '-', '/', '.'])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            c.next().map(|f| f.to_ascii_uppercase().to_string() + c.as_str()).unwrap_or_default()
        })
        .collect()
}

/// A table, checked: what its enum is made of.
#[derive(Debug, Clone)]
pub struct Table {
    file: String,
    name: String,
    serde: bool,
    columns: Vec<(String, Column)>,
    /// (row id, variant, each column's expression).
    rows: Vec<(String, String, Vec<String>)>,
}

/// Checks a table; its errors as `path: message`.
pub fn check(file: &str, t: TableDef, refs: &yres::Refs, known: &refs::Known) -> Result<Table, Vec<String>> {
    let mut errors = Vec::new();
    let name = pascal(&t.id);
    if name.is_empty() || !name.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) || !name.chars().all(|c| c.is_ascii_alphanumeric()) {
        errors.push(format!("id: `{}`: a name like `tier` or `power_level`", t.id));
    }
    let mut columns = Vec::new();
    for (c, ty) in &t.columns.0 {
        if !super::valid_path(c) || c.contains('/') || c.starts_with(|ch: char| ch.is_ascii_digit()) {
            errors.push(format!("columns.{c}: a name like `world_size`"));
        } else if ["id", "index", "from_id", "all"].contains(&c.as_str()) {
            errors.push(format!("columns.{c}: taken (every table has it)"));
        }
        match column(ty) {
            Ok(col) => columns.push((c.clone(), col)),
            Err(e) => errors.push(format!("columns.{c}: {e}")),
        }
    }
    let mut rows = Vec::new();
    for (id, v) in &t.rows.0 {
        let variant = pascal(id);
        if variant.is_empty() || !variant.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) {
            errors.push(format!("rows.{id}: a name like `wood`"));
            continue;
        }
        let Some(map) = v.as_map() else {
            errors.push(format!("rows.{id}: a row is a map of its columns"));
            continue;
        };
        for (k, _) in map.iter() {
            if !columns.iter().any(|(c, _)| c == k) {
                errors.push(format!("rows.{id}.{k}: no such column"));
            }
        }
        let mut exprs = Vec::new();
        for (c, col) in &columns {
            let Some(cell) = map.get(c) else {
                errors.push(format!("rows.{id}.{c}: missing"));
                continue;
            };
            let e = match col {
                Column::One(s) => s.expr(cell, refs, known),
                Column::List(s) => match cell.as_list() {
                    Some(items) => items.iter().map(|i| s.expr(i, refs, known)).collect::<Result<Vec<_>, _>>().map(|v| format!("&[{}]", v.join(", "))),
                    None => Err("want a list".into()),
                },
                Column::Map(k, val) => match cell.as_map() {
                    Some(m) => m
                        .iter()
                        .map(|(key, x)| {
                            // A key is written bare: `minecraft:redstone`, not `@item:…`.
                            let key = match k {
                                Scalar::Item => Value::Ref(yog_data::Ref::new(yog_ids::kind::ITEM, key)),
                                Scalar::Block => Value::Ref(yog_data::Ref::new(yog_ids::kind::BLOCK, key)),
                                _ => yog_data::text::parse_value(key).unwrap_or_else(|_| Value::Str(key.to_string())),
                            };
                            Ok(format!("({}, {})", k.expr(&key, refs, known)?, val.expr(x, refs, known)?))
                        })
                        .collect::<Result<Vec<_>, String>>()
                        .map(|v| format!("&[{}]", v.join(", "))),
                    None => Err("want a map".into()),
                },
            };
            match e {
                Ok(e) => exprs.push(e),
                Err(e) => errors.push(format!("rows.{id}.{c}: {e}")),
            }
        }
        if rows.iter().any(|(_, v, _): &(String, String, Vec<String>)| *v == variant) {
            errors.push(format!("rows.{id}: twice"));
        }
        rows.push((id.clone(), variant, exprs));
    }
    if rows.is_empty() {
        errors.push("rows: none".into());
    }
    if errors.is_empty() { Ok(Table { file: file.to_string(), name, serde: t.serde, columns, rows }) } else { Err(errors) }
}

/// The enum a table makes.
pub fn rust(t: &Table) -> String {
    let mut code = String::new();
    let n = &t.name;
    let derive = if t.serde { ", ::serde::Serialize, ::serde::Deserialize" } else { "" };
    let _ = writeln!(code, "\n/// The `{}` table: a variant per row.\n#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord{derive})]\n#[allow(dead_code)]\npub enum {n} {{", t.file);
    for (_, v, _) in &t.rows {
        let _ = writeln!(code, "    {v},");
    }
    code.push_str("}\n\n#[allow(dead_code, clippy::all)]\nimpl ");
    let _ = writeln!(code, "{n} {{\n    /// Every row, in order.\n    pub const ALL: [{n}; {}] = [{}];\n", t.rows.len(), t.rows.iter().map(|(_, v, _)| format!("{n}::{v}")).collect::<Vec<_>>().join(", "));
    let arms = |f: &dyn Fn(usize, &(String, String, Vec<String>)) -> String| -> String {
        t.rows.iter().enumerate().map(|(i, r)| format!("            {n}::{} => {},\n", r.1, f(i, r))).collect()
    };
    let _ = writeln!(code, "    /// Its row's id.\n    pub fn id(self) -> &'static str {{\n        match self {{\n{}        }}\n    }}\n", arms(&|_, r| format!("{:?}", r.0)));
    let _ = writeln!(code, "    pub fn from_id(id: &str) -> ::std::option::Option<{n}> {{\n        Self::ALL.into_iter().find(|r| r.id() == id)\n    }}\n");
    let _ = writeln!(code, "    /// Its place in [`Self::ALL`].\n    pub fn index(self) -> usize {{\n        self as usize\n    }}");
    for (ci, (c, col)) in t.columns.iter().enumerate() {
        let ty = match col {
            Column::One(s) => s.rust().to_string(),
            Column::List(s) => format!("&'static [{}]", s.rust()),
            Column::Map(k, v) => format!("&'static [({}, {})]", k.rust(), v.rust()),
        };
        let _ = writeln!(code, "\n    pub fn {c}(self) -> {ty} {{\n        match self {{\n{}        }}\n    }}", arms(&|_, r| r.2[ci].clone()));
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

    fn table(text: &str) -> Result<Table, Vec<String>> {
        let doc = yog_data::text::parse(text).unwrap();
        let def = match yog_data::schema::from_doc::<yog_formats::def::Def>(text, &doc).unwrap().value {
            yog_formats::def::Def::Table(t) => t,
            _ => panic!("not a table"),
        };
        let mut refs = yres::Refs::default();
        refs.texts.insert("Strings.tier_wood".into(), ("mymod.tier_wood".into(), "Wood".into()));
        let mut known = refs::Known::new("mymod", &[], None);
        known.add(RefKind::Item, String::from("mymod:chip_wood"));
        check("content/tier.ydef", def, &refs, &known)
    }

    #[test]
    fn a_table_is_an_enum_with_a_method_per_column() {
        let t = table(
            r#"{ kind: "table", id: "tier", serde: true,
                columns: { size: "u32", chip: "item", name: "text", inputs: "{item: u64}" },
                rows: {
                    wood: { size: 16, chip: "@item:mymod:chip_wood", name: "@Strings.tier_wood", inputs: { "minecraft:oak_log": 1 } },
                    stone: { size: 32, chip: "@item:mymod:chip_wood", name: "@Strings.tier_wood", inputs: {} },
                } }"#,
        )
        .unwrap();
        let code = rust(&t);
        assert!(code.contains("pub enum Tier {\n    Wood,\n    Stone,\n}"), "{code}");
        assert!(code.contains("::serde::Serialize"));
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
        let all = e.join("\n");
        assert!(all.contains("columns.odd"), "{all}");
        assert!(all.contains("rows.wood.size: 300 doesn't fit a u8"), "{all}");
        assert!(all.contains("rows.wood.chip"), "{all}");
        assert!(all.contains("rows.wood.extra: no such column"), "{all}");
    }
}
