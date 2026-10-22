//! Nanites (`docs/design/nanites.md`).
//!
//! A host's mapping table ("Names"): the game's
//! classes and members by Mojang's names, and what they're called at run
//! time on that host (intermediary on Fabric before 26.2, SRG members on
//! Forge). Only names that differ are kept. `yog mappings` makes it; the
//! host reads it once to look up every name a nanite uses.

use yog_data::{Schema, Value};
use yog_ids as ids;

actions! {
    /// A member by Mojang's names (`class`: `net.minecraft.world.entity.LivingEntity`;
    /// `desc`: a method's or constructor's (`<init>`) descriptor, or a
    /// field's type) → its handle, for every call after; `None`: no such member.
    Resolve { class: String, name: String, desc: String } -> Option<u32> = ids::service_nanite::RESOLVE, "nanite.YogReflect.resolve";
    /// Method or constructor `member` called on `target` (null: a static one,
    /// a constructor) with `args`; its result (`None`: it failed). An object
    /// that isn't a number, text, a list or an entity comes back as a
    /// reference (`yog_ids::nanite_ref::OBJECT`) the host holds until released.
    Call { member: u32, target: Value, args: Vec<Value> } -> Option<Value> = ids::service_nanite::CALL, "nanite.YogReflect.call";
    /// Field `member` of `target` (null: a static one).
    Get { member: u32, target: Value } -> Option<Value> = ids::service_nanite::GET, "nanite.YogReflect.get";
    /// Field `member` of `target` set to `value`.
    Set { member: u32, target: Value, value: Value } -> bool = ids::service_nanite::SET, "nanite.YogReflect.set";
    /// The objects referenced let go.
    Release { objects: Vec<u64> } -> bool = ids::service_nanite::RELEASE, "nanite.YogReflect.release";
}

/// Every class a member or the class itself of is renamed.
#[derive(Schema, Debug, Clone, Default, PartialEq, Eq)]
pub struct Mappings {
    /// The run-time namespace (`yog_ids::mapping_ns::*`).
    pub runtime: String,
    pub classes: Vec<ClassMap>,
}

/// A class: its names (internal form, `net/minecraft/world/entity/Entity`)
/// and its renamed members.
#[derive(Schema, Debug, Clone, Default, PartialEq, Eq)]
pub struct ClassMap {
    pub named: String,
    pub runtime: String,
    pub fields: Vec<MemberMap>,
    pub methods: Vec<MemberMap>,
}

/// A member: its name, its descriptor in Mojang's names (a method's;
/// empty for a field) and its run-time name.
#[derive(Schema, Debug, Clone, Default, PartialEq, Eq)]
pub struct MemberMap {
    pub named: String,
    pub desc: String,
    pub runtime: String,
}

impl Mappings {
    /// Drops what isn't renamed: members named alike, classes left with
    /// nothing renamed.
    pub fn prune(&mut self) {
        for c in &mut self.classes {
            c.fields.retain(|m| m.named != m.runtime);
            c.methods.retain(|m| m.named != m.runtime);
        }
        self.classes
            .retain(|c| c.named != c.runtime || !c.fields.is_empty() || !c.methods.is_empty());
        self.classes.sort_by(|a, b| a.named.cmp(&b.named));
    }
}
