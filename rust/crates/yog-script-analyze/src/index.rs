//! The class index (`docs/design/ys.md` §8): what the compiler knows of the
//! game for one Minecraft version — each class by Mojang's name, the side
//! it's on, what it extends and implements, its fields and methods with
//! their descriptors and access. Built by `yog index` from the game's own
//! jars; the resolver and the type checker look the game's names up in it.

use std::collections::HashMap;

use yog_data::Schema;
/// What separates a nested class from the class it's in (`Outer$Inner`).
pub use yog_script_ids::jvm::NESTED;

/// Where a class exists: on both sides, or only in the client's or the
/// dedicated server's jar. Code of one side can't reach the other's.
#[derive(Schema, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Side {
    #[default]
    Shared,
    Client,
    Server,
}

/// A field or a method: its name, JVM descriptor (`(F)Z`, `I`) and access
/// flags (`ACC_PUBLIC` 0x0001, `ACC_STATIC` 0x0008, …).
#[derive(Schema, Debug, Clone, Default, PartialEq, Eq)]
pub struct Member {
    pub name: String,
    pub desc: String,
    pub access: u16,
}

/// A class by its internal name (`net/minecraft/world/entity/LivingEntity`).
#[derive(Schema, Debug, Clone, Default, PartialEq, Eq)]
pub struct Class {
    pub name: String,
    pub side: Side,
    pub access: u16,
    pub extends: Option<String>,
    pub implements: Vec<String>,
    pub fields: Vec<Member>,
    pub methods: Vec<Member>,
}

/// The game's classes for Minecraft `mc`.
#[derive(Schema, Debug, Clone, Default, PartialEq, Eq)]
pub struct ClassIndex {
    pub mc: String,
    pub classes: Vec<Class>,
}

pub mod access;
pub mod namespace;

/// A [`ClassIndex`] ready to look things up in: classes by name, and by
/// simple name (`LivingEntity`) for what a source writes unqualified.
pub struct Lookup<'i> {
    by_name: HashMap<&'i str, &'i Class>,
    by_simple: HashMap<&'i str, Vec<&'i Class>>,
}

impl ClassIndex {
    pub fn lookup(&self) -> Lookup<'_> {
        let mut by_name = HashMap::new();
        let mut by_simple: HashMap<&str, Vec<&Class>> = HashMap::new();
        for c in &self.classes {
            by_name.insert(c.name.as_str(), c);
            by_simple.entry(simple_name(&c.name)).or_default().push(c);
        }
        Lookup { by_name, by_simple }
    }
}

/// `net/minecraft/world/entity/LivingEntity` → `LivingEntity`; a nested
/// class (`Outer$Inner`) by its own name (`Inner`).
pub fn simple_name(internal: &str) -> &str {
    let tail = internal.rsplit('/').next().unwrap_or(internal);
    tail.rsplit(NESTED).next().unwrap_or(tail)
}

impl<'i> Lookup<'i> {
    /// The class of internal name `name`.
    pub fn class(&self, name: &str) -> Option<&'i Class> {
        self.by_name.get(name).copied()
    }

    /// The classes a source's unqualified `name` may mean.
    pub fn simple(&self, name: &str) -> &[&'i Class] {
        self.by_simple
            .get(name)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    /// `class` and every class it extends, nearest first (the index's).
    pub fn ancestry(&self, class: &'i Class) -> Vec<&'i Class> {
        let mut out = vec![class];
        while let Some(parent) = out
            .last()
            .and_then(|c| c.extends.as_deref())
            .and_then(|p| self.class(p))
        {
            if out.iter().any(|c| c.name == parent.name) {
                break;
            }
            out.push(parent);
        }
        out
    }

    /// The methods named `name` that `class` has, its own or inherited
    /// (through what it extends and implements).
    pub fn methods(&self, class: &'i Class, name: &str) -> Vec<&'i Member> {
        let mut out = Vec::new();
        let mut seen = Vec::new();
        self.collect_methods(class, name, &mut out, &mut seen);
        out
    }

    fn collect_methods(
        &self,
        class: &'i Class,
        name: &str,
        out: &mut Vec<&'i Member>,
        seen: &mut Vec<&'i str>,
    ) {
        if seen.contains(&class.name.as_str()) {
            return;
        }
        seen.push(&class.name);
        for m in class.methods.iter().filter(|m| m.name == name) {
            if !out.iter().any(|o| o.desc == m.desc) {
                out.push(m);
            }
        }
        let parents = class.extends.iter().chain(&class.implements);
        for p in parents.filter_map(|p| self.class(p)) {
            self.collect_methods(p, name, out, seen);
        }
    }

    /// The field `name` of `class`, its own or inherited.
    pub fn field(&self, class: &'i Class, name: &str) -> Option<&'i Member> {
        self.ancestry(class)
            .into_iter()
            .find_map(|c| c.fields.iter().find(|f| f.name == name))
    }
}
