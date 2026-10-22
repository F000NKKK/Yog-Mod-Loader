//! Mirrors (`docs/design/bus.md`, "Mirrors"): data read every tick kept in
//! shared memory. A mod declares the fields it reads ([`Declared`]: read in
//! place, released on drop) or provides a mirror ([`Registry::provide_mirror`]:
//! its writer is called when fields are due, with only those). Usually
//! through `#[yog_mirror]`, which generates both sides typed.

use std::os::raw::c_void;

use yog_abi::{YogBytes, YogReply, YogStatus, YogStr, status};
use yog_data::Diagnostic;
use yog_event::Message;
use yog_event::mirror::{Declare, Provide, Release, Write};
use yog_mirror::{Bits, ColumnRef, ColumnWriter, KeySpaceRef};
use yog_sync::Arc;

pub use yog_mirror::{Cell, FieldSpec, Kind, MirrorSpec, Rate};

use crate::registry::{Registry, guard};

mod host;
mod typed;
pub use host::{Blocks, Entities, Players};
pub use typed::{Mirror, MirrorWriter, Mirrored, Row, RowWriter};
pub use yog_event::mirror::BlockScope;
pub use yog_mirror::host as host_tables;

/// Bits a word of a row bitmap holds.
const WORD_BITS: u32 = u64::BITS;

/// A declaration's field: its column, how many parents up its space is.
#[derive(Copy, Clone)]
struct Field {
    column: ColumnRef,
    depth: u8,
}

/// Fields of a mirror asked for: written at the rate asked while this
/// lives, read in place; dropped, what only it asked for stops.
pub struct Declared {
    id: u32,
    space: KeySpaceRef,
    rows: ColumnRef,
    fields: Vec<Option<Field>>,
}

fn no_runtime() -> Diagnostic {
    Diagnostic::error(
        yog_ids::mirror_problem::FAILED,
        "no one answers mirrors here",
    )
}

impl Declared {
    /// Asks for `wanted` fields of `spec` (a bit each, in field order) at `rate`.
    pub fn new(spec: &MirrorSpec, wanted: u64, rate: Rate) -> Result<Declared, Diagnostic> {
        Self::scoped(spec, wanted, rate, None)
    }

    /// [`Declared::new`] covering `scope` only (the mirror's scope type as
    /// Yog Data: a `BlockScope`…).
    pub fn scoped(
        spec: &MirrorSpec,
        wanted: u64,
        rate: Rate,
        scope: Option<Vec<u8>>,
    ) -> Result<Declared, Diagnostic> {
        let request = Declare {
            spec: spec.clone(),
            wanted,
            rate,
            scope: scope.map(|bytes| yog_event::mirror::Scope { bytes }),
        };
        let reply = crate::call(&request).ok_or_else(no_runtime)?;
        let d = match (reply.declaration, reply.problem) {
            (Some(d), _) => d,
            (None, problem) => return Err(problem.unwrap_or_else(no_runtime)),
        };
        // SAFETY: the runtime keeps what a declaration points at until it's released.
        let (space, rows) = unsafe {
            (
                KeySpaceRef::new(d.space as *const _),
                ColumnRef::new(d.rows as *const _),
            )
        };
        let fields = d
            .fields
            .iter()
            .map(|f| {
                let f = (*f)?;
                // SAFETY: as above.
                let column = unsafe { ColumnRef::new(f.column as *const _) }?;
                Some(Field {
                    column,
                    depth: f.depth,
                })
            })
            .collect();
        match (space, rows) {
            (Some(space), Some(rows)) => Ok(Declared {
                id: d.id,
                space,
                rows,
                fields,
            }),
            _ => Err(no_runtime()),
        }
    }

    /// The rows its provider fills that hold a key now.
    pub fn rows(&self) -> impl Iterator<Item = u32> + '_ {
        let words = self.space.rows().div_ceil(WORD_BITS);
        (0..words).flat_map(move |w| {
            let both = self.rows.word(w) & self.space.live_word(w);
            Bits(both).map(move |b| w * WORD_BITS + b)
        })
    }

    /// `key`'s row.
    pub fn row_of(&self, key: u64) -> Option<u32> {
        self.space.row_of(key)
    }

    /// Row `row`'s key.
    pub fn key_of(&self, row: u32) -> Option<u64> {
        self.space.key_of(row)
    }

    /// Field `field`'s value at row `row` (`None`: not asked for, no row).
    #[inline]
    pub fn get<T: Cell>(&self, field: usize, row: u32) -> Option<T> {
        let f = (*self.fields.get(field)?)?;
        let (mut space, mut row) = (self.space, row);
        // A parent space's fact: the row's key there, its row there.
        for _ in 0..f.depth {
            let key = space.parent_key(row)?;
            space = space.parent()?;
            row = space.row_of(key)?;
        }
        f.column.get(row)
    }
}

impl Drop for Declared {
    fn drop(&mut self) {
        crate::call(&Release { id: self.id });
    }
}

/// A provider's turn: the fields due, written for the rows it fills.
pub struct Writer {
    /// The mirror's address (a family member's ends in its parameter).
    mirror: String,
    /// What its declarations asked it to cover (its scope type's frames).
    scopes: Vec<Vec<u8>>,
    fields: u64,
    space: Option<KeySpaceRef>,
    rows: Option<ColumnWriter>,
    /// The due fields' columns open for writing, by field.
    columns: Vec<Option<ColumnWriter>>,
}

impl Writer {
    fn open(w: Write) -> Writer {
        // SAFETY: the runtime's addresses, alive through this turn.
        let open = |c: u64| unsafe { ColumnRef::new(c as *const _) }.map(|c| c.write());
        let due = (0..u64::BITS as usize).filter(|i| w.fields >> i & 1 == 1);
        let mut columns: Vec<Option<ColumnWriter>> = Vec::new();
        for (i, c) in due.zip(&w.columns) {
            columns.resize_with(columns.len().max(i + 1), || None);
            columns[i] = open(*c);
        }
        Writer {
            mirror: w.mirror,
            scopes: w.scopes.into_iter().map(|s| s.bytes).collect(),
            fields: w.fields,
            // SAFETY: as above.
            space: unsafe { KeySpaceRef::new(w.space as *const _) },
            rows: open(w.rows),
            columns,
        }
    }

    /// The mirror's address.
    pub fn mirror(&self) -> &str {
        &self.mirror
    }

    /// What the declarations asked it to cover, each its scope type's frame.
    pub fn scopes(&self) -> &[Vec<u8>] {
        &self.scopes
    }

    /// Whether field `field` is due (someone asked for it now).
    #[inline]
    pub fn wants(&self, field: usize) -> bool {
        self.fields >> field & 1 == 1
    }

    /// `key`'s row (a key its space hasn't got has none: nothing to write).
    pub fn row(&mut self, key: u64) -> Option<u32> {
        let row = self.space?.row_of(key)?;
        if let Some(rows) = &mut self.rows {
            rows.set_bit(row, true);
        }
        Some(row)
    }

    /// Row `row` no longer filled by this mirror.
    pub fn leave(&mut self, row: u32) {
        if let Some(rows) = &mut self.rows {
            rows.set_bit(row, false);
        }
    }

    /// Sets field `field` at `row` to what `value` gives — called only when
    /// the field is due.
    #[inline]
    pub fn set<T: Cell>(&mut self, field: usize, row: u32, value: impl FnOnce() -> T) {
        if let Some(Some(w)) = self.columns.get_mut(field) {
            w.set(row, value());
        }
    }
}

/// The bus's handler for a mirror's writer `ud`.
unsafe extern "C" fn on_write<F: Fn(&mut Writer) + Send + Sync>(
    ud: *mut c_void,
    payload: YogBytes,
    _reply: *const YogReply,
) -> YogStatus {
    let f = unsafe { &*(ud as *const F) };
    let Some(w) = Write::from_bytes(unsafe { payload.as_slice() }) else {
        return status::BAD_PAYLOAD;
    };
    let mut writer = Writer::open(w);
    guard((), || f(&mut writer));
    status::OK
}

impl Registry {
    /// Provides mirror `spec`: `write` fills what's due each time it is.
    pub fn provide_mirror(
        &mut self,
        spec: &MirrorSpec,
        write: impl Fn(&mut Writer) + Send + Sync + 'static,
    ) -> bool {
        self.providing_mirror(spec, false, Arc::new(write))
    }

    pub(crate) fn providing_mirror<F: Fn(&mut Writer) + Send + Sync + 'static>(
        &mut self,
        spec: &MirrorSpec,
        host: bool,
        write: Arc<F>,
    ) -> bool {
        let bus = unsafe { &*self.api().bus };
        let ud = Arc::into_raw(write).cast_mut().cast::<c_void>();
        let st = unsafe {
            let at = (bus.resolve)(bus.ctx, YogStr::from_str(&spec.name));
            (bus.provide)(bus.ctx, at, ud, on_write::<F>)
        };
        if st != status::OK {
            crate::error!("{}: someone provides it already", spec.name);
            return false;
        }
        let problem = self.call(&Provide {
            spec: spec.clone(),
            host,
        });
        match problem.flatten() {
            Some(p) => {
                crate::error!("{}: {p}", spec.name);
                false
            }
            None => true,
        }
    }
}

#[cfg(test)]
mod tests;
