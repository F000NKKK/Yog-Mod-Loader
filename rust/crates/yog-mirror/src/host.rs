//! The host's mirrors: what the game itself writes (every entity, every
//! player), described once here — the runtime makes the Java host their
//! provider, Java's constants are generated from these tables, and
//! `yog_api`'s typed structs are checked against them when they compile.

use yog_ids::{fact, host_mirror, key_space, mirror_word, word};

use crate::cell::Kind;
use crate::spec::{FieldSpec, MirrorSpec};

/// A host mirror field: its name, fact and shape.
pub struct HostField {
    pub name: &'static str,
    pub fact: &'static str,
    pub kind: Kind,
    pub count: u32,
}

/// A host mirror.
pub struct HostMirror {
    /// Its address.
    pub address: &'static str,
    pub space: &'static str,
    pub fields: &'static [HostField],
}

/// Numbers in a position, a velocity.
const XYZ: u32 = 3;
/// Numbers in a rotation: yaw, pitch.
const YAW_PITCH: u32 = 2;
/// Numbers in a uuid: its high and low halves.
const UUID_HALVES: u32 = 2;

const fn field(name: &'static str, fact: &'static str, kind: Kind, count: u32) -> HostField {
    HostField {
        name,
        fact,
        kind,
        count,
    }
}

/// Every live entity, by network id. Its fields, in order.
pub const ENTITY: HostMirror = HostMirror {
    address: host_mirror::ENTITY,
    space: key_space::ENTITY,
    fields: &[
        field(mirror_word::POS, fact::ENTITY_POS, Kind::F64, XYZ),
        field(mirror_word::VELOCITY, fact::ENTITY_VELOCITY, Kind::F64, XYZ),
        field(
            yog_ids::model::ROTATION,
            fact::ENTITY_ROTATION,
            Kind::F32,
            YAW_PITCH,
        ),
        field(word::KIND, fact::ENTITY_KIND, Kind::U32, 1),
        field(word::DIMENSION, fact::ENTITY_DIMENSION, Kind::U32, 1),
        field(mirror_word::HEALTH, fact::ENTITY_HEALTH, Kind::F32, 1),
        field(mirror_word::UUID, fact::ENTITY_UUID, Kind::U64, UUID_HALVES),
    ],
};

/// Every player, by its entity's network id (part of [`ENTITY`]).
pub const PLAYER: HostMirror = HostMirror {
    address: host_mirror::PLAYER,
    space: key_space::PLAYER,
    fields: &[field(word::NAME, fact::PLAYER_NAME, Kind::U32, 1)],
};

/// Every host mirror.
pub const ALL: &[&HostMirror] = &[&ENTITY, &PLAYER];

impl HostMirror {
    pub fn spec(&self) -> MirrorSpec {
        MirrorSpec {
            name: self.address.to_owned(),
            space: self.space.to_owned(),
            fields: self.fields().collect(),
        }
    }

    /// Its fields as specs.
    pub fn fields(&self) -> impl Iterator<Item = FieldSpec> + '_ {
        self.fields.iter().map(|f| FieldSpec {
            name: f.name.to_owned(),
            fact: Some(f.fact.to_owned()),
            kind: f.kind.code(),
            count: f.count,
        })
    }

    /// Whether field `i` is `kind` × `count` (what a typed struct checks
    /// its fields against when it compiles).
    pub const fn shaped(&self, i: usize, kind: Kind, count: u32) -> bool {
        i < self.fields.len()
            && self.fields[i].kind.code() == kind.code()
            && self.fields[i].count == count
    }
}
