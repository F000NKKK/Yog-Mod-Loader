//! The generated `Blocks` and `Items`: each one's id, and the builder
//! calls making its definition.

use std::fmt::Write;
use yog_ids::Format;

use super::{float, unique_ident};
use crate::content::entry::{BlockProps, Entry, ItemProps};

/// What starts a builder call, on a line of its own.
pub(super) const CALL: &str = "\n            .";

/// A block's strength when it gives only one of its numbers: stone's.
const STONE_HARDNESS: f64 = 1.5;
const STONE_RESISTANCE: f64 = 6.0;

/// A kind of entry's code: its struct, the builder it makes, what its
/// entries are called.
struct Kind {
    format: Format,
    set: &'static str,
    def: &'static str,
    noun: &'static str,
}

const KINDS: [Kind; 2] = [
    Kind {
        format: Format::Block,
        set: yog_ids::code_set::BLOCKS,
        def: "BlockDef",
        noun: yog_ids::word::BLOCKS,
    },
    Kind {
        format: Format::Item,
        set: yog_ids::code_set::ITEMS,
        def: "ItemDef",
        noun: yog_ids::word::ITEMS,
    },
];

/// `.name()` when `on`.
fn flag(b: &mut String, on: bool, name: &str) {
    if on {
        let _ = write!(b, "{CALL}{name}()");
    }
}

/// `.name("value")` when there's a value.
fn quoted(b: &mut String, value: &Option<String>, name: &str) {
    if let Some(v) = value {
        let _ = write!(b, "{CALL}{name}({v:?})");
    }
}

/// `["a", "b"]`'s items, quoted.
fn quoted_list(values: &[String]) -> String {
    values
        .iter()
        .map(|v| format!("{v:?}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// How the block is shaped and stands: its strength, sound, light, shape.
fn block_body(b: &mut String, p: &BlockProps) {
    if p.hardness.is_some() || p.resistance.is_some() {
        let hardness = float(p.hardness.unwrap_or(STONE_HARDNESS));
        let resistance = float(p.resistance.unwrap_or(STONE_RESISTANCE));
        let _ = write!(b, "{CALL}strength({hardness}, {resistance})");
    }
    quoted(b, &p.sound, yog_ids::word::SOUND);
    if p.light > 0 {
        let _ = write!(b, "{CALL}light_level({})", p.light);
    }
    flag(b, p.requires_tool, "requires_tool");
    flag(b, p.no_collision, "no_collision");
    if let Some(s) = p.slipperiness {
        let _ = write!(b, "{CALL}slipperiness({})", float(s));
    }
    if let Some(s) = p.shape {
        let corner = |c: &[f64]| c.iter().map(|n| float(*n)).collect::<Vec<_>>().join(", ");
        let (from, to) = s.split_at(3);
        let _ = write!(b, "{CALL}shape([{}], [{}])", corner(from), corner(to));
    }
}

/// The builder calls for a block's properties.
pub(super) fn block_props(p: &BlockProps) -> String {
    let mut b = String::new();
    block_body(&mut b, p);
    flag(&mut b, p.connects, "connects_to_neighbors");
    if !p.connect_groups.is_empty() {
        let _ = write!(
            b,
            "{CALL}connect_groups(&[{}])",
            quoted_list(&p.connect_groups)
        );
    }
    quoted(&mut b, &p.inventory, "inventory");
    flag(&mut b, p.redstone, "redstone_source");
    if let Some(facing) = p.facing {
        let _ = write!(
            b,
            "{CALL}facing(::yog_api::Facing::{facing:?}, ::yog_api::Placement::{:?})",
            p.placement
        );
    }
    flag(&mut b, p.attach, "attach");
    flag(&mut b, p.waterlogged, "waterlogged");
    for (name, values) in &p.properties {
        let _ = write!(b, "{CALL}property({name:?}, &[{}])", quoted_list(values));
    }
    quoted(&mut b, &p.powered, "powered");
    quoted(&mut b, &p.comparator, "comparator");
    flag(&mut b, p.accepts_items, "accepts_items");
    flag(&mut b, p.no_occlusion, "no_occlusion");
    if !p.kind.is_empty() {
        let _ = write!(b, "{CALL}kind({:?})", p.kind);
    }
    quoted(&mut b, &p.base, "base");
    quoted(&mut b, &p.set, "set");
    if let Some(t) = p.press_ticks {
        let _ = write!(b, "{CALL}press_ticks({t})");
    }
    flag(&mut b, p.mobs_only, "mobs_only");
    b
}

/// The builder calls (`.max_stack(16)`…) for an item's properties; empty
/// when it has none of its own.
pub(super) fn item_props(p: &ItemProps) -> String {
    let mut b = String::new();
    if let Some(s) = p.stack_size {
        let _ = write!(b, "{CALL}max_stack({s})");
    }
    if p.durability > 0 {
        let _ = write!(b, "{CALL}max_damage({})", p.durability);
    }
    flag(&mut b, p.fire_resistant, "fire_resistant");
    if p.fuel_ticks > 0 {
        let _ = write!(b, "{CALL}fuel({})", p.fuel_ticks);
    }
    if let Some((kind, material, d, s)) = &p.tool {
        let _ = write!(b, "{CALL}tool({kind:?}, {material:?})");
        if d.is_some() || s.is_some() {
            let _ = write!(b, "{CALL}tool_attack({d:?}, {s:?})");
        }
    }
    if let Some(slot) = p.wearable {
        let _ = write!(b, "{CALL}wearable(::yog_api::ArmorSlot::{slot:?})");
    }
    if let Some((entity, size)) = &p.hangs {
        let _ = write!(b, "{CALL}hangs({entity:?}, {size:?})");
    }
    if let Some((slot, material)) = &p.armor {
        let _ = write!(
            b,
            "{CALL}armor(::yog_api::ArmorSlot::{slot:?}, {material:?})"
        );
    }
    if let Some((n, s, always)) = p.food {
        let always = if always { ".can_always_eat()" } else { "" };
        let _ = write!(
            b,
            "{CALL}food(::yog_api::FoodDef::new({n}, {}){always})",
            float(s)
        );
    }
    b
}

/// Each entry's name in the generated code: its path, unique among its
/// kind's (`ruby`, `ruby_2`) — a keyword not yet escaped (see [`escaped`]).
pub(super) fn entry_idents(entries: &[Entry]) -> Vec<String> {
    let mut used = std::collections::HashMap::new();
    entries
        .iter()
        .map(|e| {
            let used = used.entry(e.kind).or_default();
            unique_ident(used, super::rust_ident(yog_ids::split(&e.id).1))
        })
        .collect()
}

/// `ident` as Rust takes it: a keyword as `r#ident`.
pub(super) fn escaped(ident: &str) -> String {
    if crate::project::rust_code::is_keyword(ident) {
        format!("r#{ident}")
    } else {
        ident.to_string()
    }
}

/// Each kind's struct — `Blocks`, `Items` — with an id const and a
/// definition fn per entry: (part name, code) each.
pub(super) fn entries_code(entries: &[Entry], idents: &[String]) -> Vec<(String, String)> {
    KINDS
        .iter()
        .map(|k| (k.noun.to_string(), kind_code(k, entries, idents)))
        .collect()
}

fn kind_code(k: &Kind, entries: &[Entry], idents: &[String]) -> String {
    let Kind { set, def, noun, .. } = k;
    let mut code = format!(
        "\n/// The {noun} of `content/`: the id and a [`::yog_api::{def}`] of each.\n#[allow(dead_code)]\npub struct {set};\n\n#[allow(dead_code, clippy::all)]\nimpl {set} {{\n"
    );
    for (e, ident) in entries
        .iter()
        .zip(idents)
        .filter(|(e, _)| e.kind == k.format)
    {
        let upper = ident.to_ascii_uppercase();
        let _ = writeln!(
            code,
            "    /// `{}` — `{}`.\n    pub const {upper}: &'static str = {:?};",
            e.id, e.file, e.id
        );
        let props = if k.format == Format::Block {
            block_props(&e.block)
        } else {
            item_props(&e.item)
        };
        let _ = writeln!(
            code,
            "    /// The definition from `{}` (name and tooltip come from the lang files).\n    pub fn {}() -> ::yog_api::{def} {{\n        ::yog_api::{def}::new(Self::{upper}){props}\n    }}",
            e.file,
            escaped(ident)
        );
    }
    code.push_str("}\n");
    code
}
