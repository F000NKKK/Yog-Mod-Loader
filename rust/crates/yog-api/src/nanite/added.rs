//! Members a mod adds to a game class (`docs/design/nanites.md`): fields
//! (`added_field!`, the mod's own state on the object — Mixin's `@Unique`),
//! methods (`#[add_method]`, their bodies the mod's handlers) and interfaces
//! (`implement!`). Declared before the game's classes load, as injections are.

use std::marker::PhantomData;

use super::{Java, Member, Nanite, Reply};

/// A field a mod added to class `O`, of type `T`: named
/// `yog$<crate>$<name>` on it (`yog_ids::nanite_added`).
pub struct Added<O, T> {
    member: Member,
    _types: PhantomData<fn() -> (O, T)>,
}

impl<O: Nanite, T: Java + Reply> Added<O, T> {
    /// The field `name` (the full one) of type `T` on `O`.
    pub const fn new(name: &'static str) -> Self {
        Added {
            member: Member::new(O::CLASS, name, <T as Java>::desc),
            _types: PhantomData,
        }
    }

    /// Its member (what the host adds).
    pub fn member(&'static self) -> &'static Member {
        &self.member
    }

    /// Its value on `object` (its type's default before it's set).
    pub fn get(&self, object: &O) -> T {
        <T as Reply>::from_reply(self.member.get(object.handle().to_value()))
    }

    /// Sets it on `object`; whether it was.
    pub fn set(&self, object: &O, value: T) -> bool {
        self.member
            .set(object.handle().to_value(), value.to_value())
    }
}

/// `added_field!(LivingEntity, pub CHARGE: i32);` — a field on every
/// `LivingEntity`, the mod's own (`CHARGE.get(&e)`, `CHARGE.set(&e, 3)`).
#[macro_export]
macro_rules! added_field {
    ($owner:ty, $vis:vis $name:ident : $ty:ty) => {
        $vis static $name: $crate::nanite::Added<$owner, $ty> = $crate::nanite::Added::new(
            $crate::yog_ids::concatcp!(
                $crate::yog_ids::nanite_added::PREFIX,
                ::core::env!("CARGO_PKG_NAME"),
                $crate::yog_ids::nanite_added::SEP,
                ::core::stringify!($name),
            ),
        );

        const _: () = {
            // Runs when this mod's library loads: declared before the game loads.
            #[$crate::ctor::ctor(unsafe, crate_path = $crate::ctor)]
            fn declare() {
                $crate::nanite::__declare(
                    $crate::yog_ids::nanite_kind::ADD_FIELD,
                    $name.member(),
                    "",
                );
            }
        };
    };
}

/// `implement!(LivingEntity, "net.minecraft.world.entity.OwnableEntity");` —
/// the class made to implement the interface (its methods `#[add_method]`s).
#[macro_export]
macro_rules! implement {
    ($owner:ty, $interface:expr) => {
        const _: () = {
            static CLASS: $crate::nanite::Member = $crate::nanite::Member::new(
                <$owner as $crate::nanite::Nanite>::CLASS,
                "",
                ::std::string::String::new,
            );

            #[$crate::ctor::ctor(unsafe, crate_path = $crate::ctor)]
            fn declare() {
                $crate::nanite::__declare(
                    $crate::yog_ids::nanite_kind::IMPLEMENT,
                    &CLASS,
                    $interface,
                );
            }
        };
    };
}
