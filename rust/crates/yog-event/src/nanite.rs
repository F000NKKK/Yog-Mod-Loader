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

/// A nanite a mod declares (`#[inject]`, `#[redirect]`, `#[modify_arg]`…):
/// where, by Mojang's names, and what it does there — what the host turns
/// into the Mixin injector of its `kind` before the class loads. Mods at one
/// point share it: its [`Inject::point`] is the event's key, their handlers
/// ordered by the bus's priorities.
#[derive(Schema, Debug, Clone, Default, PartialEq, Eq)]
pub struct Inject {
    /// The point's key ([`point`]).
    pub point: String,
    /// `yog_ids::nanite_kind`.
    pub kind: String,
    /// `net.minecraft.world.entity.LivingEntity`.
    pub class: String,
    pub method: String,
    pub desc: String,
    /// Mixin's `@At` value (`yog_ids::mixin_at`).
    pub at: String,
    /// What an `INVOKE` / `FIELD` / `NEW` at names (`Lowner;name(desc)`, Mojang's), else empty.
    pub target: String,
    /// Which occurrence (-1: every one).
    pub ordinal: i32,
    /// `yog_ids::mixin_shift`.
    pub shift: String,
    /// Whether handlers may cancel the method (or set its return value).
    pub cancellable: bool,
    /// A `modify_arg`'s argument, a `modify_variable`'s local (-1: by type).
    pub index: i32,
    /// A `modify_constant`'s constant: `<yog_ids::nanite_constant>:<value>`.
    pub constant: String,
    /// A `modify_variable`'s local's type (a descriptor, Mojang's).
    pub value_desc: String,
}

/// An injection point's key:
/// `<kind>:<class>#<method><desc>@<at>[:<target>][:<qualifier>]` — the
/// qualifier a `modify_arg`'s index, a `modify_variable`'s local, a
/// `modify_constant`'s constant.
pub fn point(i: &Inject) -> String {
    use ids::nanite_point::{AT, MEMBER, TARGET};
    let mut key = format!(
        "{}{TARGET}{}{MEMBER}{}{}{AT}{}",
        i.kind, i.class, i.method, i.desc, i.at
    );
    for part in [i.target.clone(), qualifier(i)] {
        if !part.is_empty() {
            key.push_str(TARGET);
            key.push_str(&part);
        }
    }
    key
}

/// What tells apart nanites of one kind at one place.
fn qualifier(i: &Inject) -> String {
    match i.kind.as_str() {
        ids::nanite_kind::MODIFY_ARG => i.index.to_string(),
        ids::nanite_kind::MODIFY_VARIABLE => format!("{}{}", i.index, i.value_desc),
        ids::nanite_kind::MODIFY_CONSTANT => i.constant.clone(),
        _ => String::new(),
    }
}

/// Every injection a mod declares (`yog_mod_nanites`).
#[derive(Schema, Debug, Clone, Default, PartialEq, Eq)]
pub struct Nanites {
    pub injects: Vec<Inject>,
}

/// A nanite's point reached (keyed by its point): the object (null in a
/// static method), the arguments (a redirected call's, the receiver first),
/// the value at stake. Handlers change it: `cancelled` stops the method,
/// `returned` makes `value` the answer.
#[derive(Schema, Debug, Clone, Default, PartialEq)]
pub struct Injected {
    pub point: String,
    pub this: Value,
    pub args: Vec<Value>,
    /// The value at stake: the return value, a `modify_*`'s value.
    pub value: Value,
    pub cancelled: bool,
    /// `value` is the answer (a return value, a call's result, a modified value).
    pub returned: bool,
    /// A redirect's original call, an object (`java.util.function.Function`
    /// of its arguments) a handler may call instead of answering.
    pub original: Value,
}
