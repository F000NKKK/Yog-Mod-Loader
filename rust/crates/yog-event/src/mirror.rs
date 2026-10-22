//! Mirrors (`docs/design/bus.md`, "Mirrors"): declaring one, providing one,
//! its keys — services the runtime answers. A declaration's reply is where
//! its data is: addresses in this process, read in place (`yog_mirror::view`).

use yog_core::BlockPos;
use yog_data::with::Raw;
use yog_data::{Diagnostic, Schema};
use yog_mirror::{MirrorSpec, Rate};

/// Asks for `wanted` fields of mirror `spec` (a bit each, in its field
/// order) at `rate`; they're written while the declaration lasts.
#[derive(Debug, Clone, Schema)]
pub struct Declare {
    pub spec: MirrorSpec,
    pub wanted: u64,
    pub rate: Rate,
    /// What its provider should cover (the mirror's scope type: a
    /// [`BlockScope`]…); none when it covers everything.
    pub scope: Option<Scope>,
}

/// A declaration's scope, as its mirror's scope type's Yog Data.
#[derive(Debug, Clone, PartialEq, Eq, Schema)]
pub struct Scope {
    #[data(with = "Raw")]
    pub bytes: Vec<u8>,
}

/// The blocks of a box (both corners in): the host's block mirror's scope.
/// Its dimension is its key space's (`yog:keys/block/<dimension>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Schema)]
pub struct BlockScope {
    pub min: BlockPos,
    pub max: BlockPos,
}

/// Where a declared field's values are.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Schema)]
pub struct Place {
    /// Its column (`*const YogColumn`).
    pub column: u64,
    /// How many parents up its column's key space is.
    pub depth: u8,
}

/// A declaration made: its id (to release it) and where its data is.
#[derive(Debug, Clone, PartialEq, Eq, Schema)]
pub struct Declaration {
    pub id: u32,
    /// The mirror's key space (`*const YogKeySpace`).
    pub space: u64,
    /// The mirror's rows (`*const YogColumn`, a bit a space row).
    pub rows: u64,
    /// Each field's place, `None` when it wasn't asked for.
    pub fields: Vec<Option<Place>>,
}

/// [`Declare`]'s answer: the declaration, or why there's none.
#[derive(Debug, Clone, Schema)]
pub struct Declared {
    pub declaration: Option<Declaration>,
    pub problem: Option<Diagnostic>,
}

/// Lets declaration `id` go.
#[derive(Debug, Clone, Schema)]
pub struct Release {
    pub id: u32,
}

/// Makes the caller mirror `spec`'s provider. It answers the mirror's own
/// address (`spec.name`) with a [`Write`] whenever fields are due; `host`:
/// the game's own mirrors, first for every fact they write. The answer is
/// what's wrong, if anything.
#[derive(Debug, Clone, Schema)]
pub struct Provide {
    pub spec: MirrorSpec,
    pub host: bool,
    /// Its space is a family: a member each parameter
    /// (`<space>/<parameter>`), made when it's first named, with this
    /// mirror in it as `<name>/<parameter>`.
    pub family: bool,
}

/// Mirror `name`'s provider saying it changed (its on-change fields are due).
#[derive(Debug, Clone, Schema)]
pub struct Touch {
    pub name: String,
}

/// A key coming into a key space (`parent`: its key in the parent space).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Schema)]
pub struct Key {
    pub key: u64,
    pub parent: Option<u64>,
}

/// Keys coming into key space `space` and leaving it; the answer is the
/// rows the new ones got.
#[derive(Debug, Clone, Schema)]
pub struct Keys {
    pub space: String,
    pub put: Vec<Key>,
    pub take: Vec<u64>,
}

/// A provider's turn (sent to its mirror's address): the fields due, a
/// bit each in its field order, and where they go.
#[derive(Debug, Clone, Default, PartialEq, Eq, Schema)]
pub struct Write {
    /// The mirror's address (a family member's ends in its parameter).
    pub mirror: String,
    pub fields: u64,
    /// The mirror's key space (`*const YogKeySpace`).
    pub space: u64,
    /// Its name (what [`Keys`] takes).
    pub space_name: String,
    /// The mirror's rows (`*const YogColumn`): a bit set a row it fills.
    pub rows: u64,
    /// The due fields' columns (`*const YogColumn`), in field order.
    pub columns: Vec<u64>,
    /// What the declarations asked it to cover.
    pub scopes: Vec<Scope>,
}
