//! What a mirror is, as `#[yog_mirror]` describes it and the bus carries it:
//! its fields, the facts they are, how often they're wanted.

use yog_data::Schema;

use crate::cell::Kind;

/// A mirror field.
#[derive(Schema, Clone, Debug, PartialEq, Eq)]
pub struct FieldSpec {
    pub name: String,
    /// The fact it is (`yog_ids::fact::*`, or a mod's own): fields naming
    /// the same fact share its column. `None`: the mirror's own value.
    pub fact: Option<String>,
    /// [`Kind::code`] of its numbers.
    pub kind: u8,
    /// Numbers a row.
    pub count: u32,
}

impl FieldSpec {
    pub fn kind(&self) -> Option<Kind> {
        Kind::from_code(self.kind)
    }

    /// Bytes a row.
    pub fn width(&self) -> Option<u32> {
        Some(self.kind()?.size() as u32 * self.count)
    }
}

/// A mirror: `yog:mirror/<mod>/<name>`, rows of one key space.
#[derive(Schema, Clone, Debug, PartialEq, Eq)]
pub struct MirrorSpec {
    /// Its address.
    pub name: String,
    /// Its key space (`yog_ids::key_space::*`).
    pub space: String,
    pub fields: Vec<FieldSpec>,
    /// Whose ticks write it.
    #[data(default)]
    pub clock: Clock,
}

/// Whose ticks a mirror is written after: the server's (the world as it
/// is), or a client's (the world as that player sees it — what a client
/// mod reads on any server).
#[derive(Schema, Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Clock {
    #[default]
    Server,
    Client,
}

/// How often a declaration wants its fields written.
#[derive(Schema, Copy, Clone, Debug, PartialEq, Eq)]
pub enum Rate {
    /// Every `n` ticks (1: every tick).
    Every(u32),
    /// When its provider says the mirror changed.
    Changes,
}

/// A field's place for its readers.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Linked {
    /// Its column.
    pub column: *const yog_abi::YogColumn,
    /// How many parents up its column's space is (0: the mirror's own): a
    /// row's key is looked up there through the parent keys.
    pub depth: u8,
}

/// What a declaration gets.
#[derive(Clone, Debug)]
pub struct Declared {
    pub id: u32,
    /// The mirror's key space.
    pub space: *const yog_abi::YogKeySpace,
    /// The mirror's rows: a bit a space row its provider fills.
    pub rows: *const yog_abi::YogColumn,
    /// Each field's place, `None` when it wasn't asked for.
    pub fields: Vec<Option<Linked>>,
}

/// A provider's turn to write: its mirror, the fields due (a bit each, in
/// the mirror's field order) and where they go.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Due {
    pub writer: u32,
    pub mirror: yog_sync::Arc<str>,
    pub fields: u64,
    /// The mirror's key space (`*const YogKeySpace`).
    pub space: u64,
    /// Its name.
    pub space_name: yog_sync::Arc<str>,
    /// The mirror's rows (`*const YogColumn`).
    pub rows: u64,
    /// Its due fields' columns in [`DueList::columns`], in field order.
    pub columns: std::ops::Range<usize>,
    /// What its declarations asked it to cover, in [`DueList::scopes`].
    pub scopes: std::ops::Range<usize>,
}

/// What's due a tick: every provider's turn, their columns in one list —
/// kept from tick to tick, so nothing is allocated once it's grown.
#[derive(Clone, Debug, Default)]
pub struct DueList {
    pub turns: Vec<Due>,
    columns: Vec<u64>,
    scopes: Vec<yog_sync::Arc<[u8]>>,
}

impl DueList {
    /// `due`'s columns (`*const YogColumn` each).
    pub fn columns(&self, due: &Due) -> &[u64] {
        &self.columns[due.columns.clone()]
    }

    /// `due`'s scopes (each its mirror's scope type, as Yog Data).
    pub fn scopes(&self, due: &Due) -> &[yog_sync::Arc<[u8]>] {
        &self.scopes[due.scopes.clone()]
    }

    pub(crate) fn clear(&mut self) {
        self.turns.clear();
        self.columns.clear();
        self.scopes.clear();
    }

    pub(crate) fn push_scope(&mut self, scope: yog_sync::Arc<[u8]>) {
        self.scopes.push(scope);
    }

    pub(crate) fn next_scope(&self) -> usize {
        self.scopes.len()
    }

    pub(crate) fn push_column(&mut self, column: u64) {
        self.columns.push(column);
    }

    pub(crate) fn next_column(&self) -> usize {
        self.columns.len()
    }
}
