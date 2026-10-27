//! Nanites (`docs/design/nanites.md`): a Yog mod reaching into the game's
//! (or a Java mod's) classes by Mojang's names — any member, private ones
//! too. Usually through `#[nanite]`:
//!
//! ```ignore
//! #[nanite("net.minecraft.world.entity.LivingEntity")]
//! pub struct LivingEntity;
//!
//! #[nanite]
//! impl LivingEntity {
//!     fn hurt(&self, source: &DamageSource, amount: f32) -> bool;
//!     #[field] fn health(&self) -> f32;
//! }
//!
//! LivingEntity::of(&entity).hurt(&source, 4.0);
//! ```
//!
//! A member is resolved once (its run-time name looked up by the host), then
//! called by its handle; objects that cross are held by the host for the
//! mod until their [`Handle`]'s last copy drops.

use yog_data::{Ref, Value};
use yog_event::nanite::{Call, Get, Release, Resolve, Set};
use yog_ids::{kind, nanite_ref};
use yog_sync::{Arc, OnceLock};

mod added;
mod inject;
mod java;
pub use added::Added;
pub use inject::{__declare, __yog_nanite_registry, __yog_nanites, Hook, InjectEntry};
pub use java::{Java, Reply};

#[doc(hidden)]
pub use inject::subscribe as __subscribe;

/// An object the host holds for this mod, or an entity (by uuid).
#[derive(Clone, Debug)]
pub enum Handle {
    Object(Arc<Held>),
    Entity(String),
}

/// An object held: let go when the last handle to it drops.
#[derive(Debug)]
pub struct Held(u64);

impl Drop for Held {
    fn drop(&mut self) {
        crate::call(&Release {
            objects: vec![self.0],
        });
    }
}

yog_data::kind!(HeldObject = nanite_ref::OBJECT: u64);

impl Handle {
    /// The entity `uuid`.
    pub fn entity(uuid: impl Into<String>) -> Handle {
        Handle::Entity(uuid.into())
    }

    /// As the bus carries it.
    pub fn to_value(&self) -> Value {
        match self {
            Handle::Object(held) => Value::Ref(Ref::of::<HeldObject>(&held.0)),
            Handle::Entity(uuid) => Value::Ref(Ref::of::<yog_data::kinds::Entity>(uuid)),
        }
    }

    /// Read off the bus: a held object or an entity.
    pub fn from_value(v: Value) -> Option<Handle> {
        let Value::Ref(r) = v else { return None };
        match r.kind.as_str() {
            nanite_ref::OBJECT => r.id.as_str().parse().ok().map(|n| Handle::Object(Arc::new(Held(n)))),
            kind::ENTITY => Some(Handle::Entity(r.id.as_str().to_owned())),
            _ => None,
        }
    }
}

/// A Java class a mod reaches (`#[nanite("…")]` implements it on a struct
/// holding its [`Handle`]).
pub trait Nanite: Sized {
    /// Its name, Mojang's (`net.minecraft.world.entity.LivingEntity`).
    const CLASS: &'static str;
    fn from_handle(handle: Handle) -> Self;
    fn handle(&self) -> &Handle;

    /// The entity `entity` as this class (it should be one).
    fn of(entity: &yog_event::Entity) -> Self {
        Self::from_handle(Handle::entity(entity.uuid.clone()))
    }
}

/// A member of a class, resolved the first time it's used.
pub struct Member {
    class: &'static str,
    name: &'static str,
    /// Its descriptor, made from its Rust types the first time.
    desc: fn() -> String,
    id: OnceLock<Option<u32>>,
}

/// A constructor's name.
pub const CONSTRUCTOR: &str = "<init>";

impl Member {
    /// Member `name` of `class`, `desc` its descriptor (a method's
    /// `(…)R`, a constructor's `(…)V` named [`CONSTRUCTOR`], a field's type).
    pub const fn new(class: &'static str, name: &'static str, desc: fn() -> String) -> Member {
        Member {
            class,
            name,
            desc,
            id: OnceLock::new(),
        }
    }

    /// Its class, Mojang's name.
    pub fn class(&self) -> &'static str {
        self.class
    }

    /// Its name and descriptor, as resolved.
    pub fn signature(&self) -> (&'static str, String) {
        (self.name, (self.desc)())
    }

    /// Its handle; `None` when the host has no such member (logged there).
    pub fn id(&self) -> Option<u32> {
        *self.id.get_or_init(|| {
            crate::call(&Resolve {
                class: self.class.to_owned(),
                name: self.name.to_owned(),
                desc: (self.desc)(),
            })
            .flatten()
        })
    }

    /// Called on `target` (null: static, a constructor) with `args`.
    pub fn call(&self, target: Value, args: Vec<Value>) -> Option<Value> {
        let member = self.id()?;
        crate::call(&Call {
            member,
            target,
            args,
        })
        .flatten()
    }

    /// The field of `target` (null: static).
    pub fn get(&self, target: Value) -> Option<Value> {
        let member = self.id()?;
        crate::call(&Get { member, target }).flatten()
    }

    /// The field of `target` set to `value`.
    pub fn set(&self, target: Value, value: Value) -> bool {
        let Some(member) = self.id() else {
            return false;
        };
        crate::call(&Set {
            member,
            target,
            value,
        })
        .unwrap_or(false)
    }
}

/// `(params)ret`, a method's descriptor.
pub fn method_desc(params: &[String], ret: &str) -> String {
    format!("({}){ret}", params.concat())
}

#[cfg(test)]
mod tests;
