//! An item's properties as the builder calls the mod's generated code makes.

use std::fmt::Write;

use crate::content::ItemProps;
use crate::content::source::float;

/// The builder calls (`.max_stack(16)`…) for an item's properties; empty
/// when it has none of its own.
pub(in crate::content) fn item_props(p: &ItemProps) -> String {
    let mut b = String::new();
    if let Some(s) = p.stack_size {
        let _ = write!(b, "\n            .max_stack({s})");
    }
    if p.durability > 0 {
        let _ = write!(b, "\n            .max_damage({})", p.durability);
    }
    if p.fire_resistant {
        b.push_str("\n            .fire_resistant()");
    }
    if p.fuel_ticks > 0 {
        let _ = write!(b, "\n            .fuel({})", p.fuel_ticks);
    }
    if let Some((kind, material, d, s)) = &p.tool {
        let _ = write!(b, "\n            .tool({kind:?}, {material:?})");
        if d.is_some() || s.is_some() {
            let _ = write!(b, "\n            .tool_attack({d:?}, {s:?})");
        }
    }
    if let Some(slot) = &p.wearable {
        let _ = write!(b, "\n            .wearable({slot:?})");
    }
    if let Some((entity, size)) = &p.hangs {
        let _ = write!(b, "\n            .hangs({entity:?}, {size:?})");
    }
    if let Some((slot, material)) = &p.armor {
        let _ = write!(b, "\n            .armor({slot:?}, {material:?})");
    }
    if let Some((n, s, always)) = p.food {
        let _ = write!(
            b,
            "\n            .food(::yog_api::FoodDef::new({n}, {}){})",
            float(s),
            if always { ".can_always_eat()" } else { "" }
        );
    }
    b
}
