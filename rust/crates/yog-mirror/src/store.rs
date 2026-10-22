//! The column store (`docs/design/bus.md`, "Mirrors"): key spaces, columns
//! (one fact in one space: one buffer, one writer, a reference count),
//! mirrors (directories: field → column, and their rows), declarations
//! (demand) and providers (writers).
//!
//! Nothing is written that no one declared: a column exists while some
//! declaration asks for it, and its writer is told only of what's due.
//! Keys come and go by a space's [`SpaceId`], no name looked up.

use std::collections::HashMap;

use yog_data::Diagnostic;
use yog_ids::{fact, key_space, mirror_problem, sep};
use yog_sync::Arc;

use crate::buffer::Buffer;
use crate::keys::{KeySpace, bit_words, set_bit};
use crate::spec::{Declared, Due, DueList, Linked, MirrorSpec, Rate};

mod links;

/// Fields a mirror may have: a bit each in a mask.
pub const MAX_FIELDS: usize = u64::BITS as usize;

type ColumnId = u32;

/// A key space's handle in its store.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct SpaceId(u32);

struct Space {
    keys: KeySpace,
    parent: Option<SpaceId>,
    /// Rows its columns and mirrors' rows were last grown to.
    grown: u32,
}

struct Column {
    buf: Buffer,
    space: SpaceId,
    fact: String,
    kind: u8,
    count: u32,
    /// The declarations asking for it, at their rates.
    demand: Vec<(u32, Rate)>,
    writer: Option<u32>,
}

#[derive(Copy, Clone)]
struct Provider {
    writer: u32,
    /// The host's writers come first.
    host: bool,
    /// Then whoever provided first.
    order: u64,
}

/// Where a field's values are: its column's space and fact, how many
/// parents up that space is.
#[derive(Clone)]
struct Place {
    space: SpaceId,
    fact: String,
    depth: u8,
}

/// A field its mirror's provider writes, with the merged demand for it.
#[derive(Copy, Clone)]
struct Write {
    bit: u64,
    /// Its column (`*const YogColumn`).
    column: u64,
    /// The most frequent period asked (0: none).
    period: u32,
    /// The tick it's due next at.
    next: u64,
    on_change: bool,
}

struct Mirror {
    /// Its name, as [`Due`]s carry it.
    name: Arc<str>,
    spec: MirrorSpec,
    space: SpaceId,
    places: Vec<Place>,
    /// Each field's column while someone asks for it.
    columns: Vec<Option<ColumnId>>,
    rows: Buffer,
    provider: Option<Provider>,
    /// What its provider writes now (worked out on every change).
    writes: Vec<Write>,
    declarations: u32,
    touched: bool,
}

struct Declaration {
    mirror: String,
    columns: Vec<ColumnId>,
}

/// Every mirror of a process (the runtime holds one).
#[derive(Default)]
pub struct Store {
    spaces: Vec<Space>,
    space_ids: HashMap<String, SpaceId>,
    columns: Vec<Option<Column>>,
    by_fact: HashMap<(SpaceId, String), ColumnId>,
    mirrors: HashMap<String, Mirror>,
    declarations: HashMap<u32, Declaration>,
    next_declaration: u32,
    next_order: u64,
}

fn problem(code: &str, message: impl std::fmt::Display) -> Diagnostic {
    Diagnostic::error(code, message)
}

/// The key space a `yog:fact/<space>/…` fact is of; `None` for a mod's own.
fn fact_space(fact: &str) -> Option<String> {
    let (space, _) = fact.strip_prefix(fact::PREFIX)?.split_once(sep::PATH)?;
    Some(format!("{}{space}", key_space::PREFIX))
}

impl Store {
    /// Makes key space `name` (part of `parent`); a space made already stays.
    pub fn space(&mut self, name: &str, parent: Option<&str>) -> Result<SpaceId, Diagnostic> {
        if let Some(id) = self.space_id(name) {
            return Ok(id);
        }
        let parent = parent
            .map(|p| self.space_id(p).ok_or_else(|| unknown_space(p)))
            .transpose()?;
        let keys = KeySpace::new(parent.map(|p| &self.space_of(p).keys));
        let id = SpaceId(self.spaces.len() as u32);
        let grown = keys.capacity();
        self.spaces.push(Space {
            keys,
            parent,
            grown,
        });
        self.space_ids.insert(name.to_string(), id);
        Ok(id)
    }

    /// Key space `name`'s handle.
    pub fn space_id(&self, name: &str) -> Option<SpaceId> {
        self.space_ids.get(name).copied()
    }

    fn space_of(&self, id: SpaceId) -> &Space {
        &self.spaces[id.0 as usize]
    }

    /// A space's header (what readers are handed).
    pub fn space_raw(&self, id: SpaceId) -> &yog_abi::YogKeySpace {
        self.space_of(id).keys.raw()
    }

    /// `key`'s row in `space`, given one if it has none.
    pub fn insert(&mut self, space: SpaceId, key: u64, parent_key: Option<u64>) -> u32 {
        let s = &mut self.spaces[space.0 as usize];
        let row = s.keys.insert(key, parent_key);
        let capacity = s.keys.capacity();
        if capacity != s.grown {
            s.grown = capacity;
            self.grow(space, capacity);
        }
        row
    }

    /// Takes `key`'s row in `space` back; it leaves every mirror's rows.
    pub fn remove(&mut self, space: SpaceId, key: u64) -> Option<u32> {
        let row = self.spaces[space.0 as usize].keys.remove(key)?;
        for m in self.mirrors.values_mut().filter(|m| m.space == space) {
            m.rows.locked(|rows| set_bit(rows, row, false));
        }
        Some(row)
    }

    /// `key`'s row in `space`.
    pub fn row_of(&self, space: SpaceId, key: u64) -> Option<u32> {
        self.space_of(space).keys.row_of(key)
    }

    /// Every column and mirror row set of `space` holds `capacity` rows.
    fn grow(&mut self, space: SpaceId, capacity: u32) {
        for c in self
            .columns
            .iter_mut()
            .flatten()
            .filter(|c| c.space == space)
        {
            c.buf.locked(|b| b.reserve(capacity));
        }
        for m in self.mirrors.values_mut().filter(|m| m.space == space) {
            m.rows.locked(|rows| rows.reserve(bit_words(capacity)));
        }
    }

    /// Asks for `wanted` fields of mirror `spec` (a bit each) at `rate`:
    /// their columns are made (or linked) and written from now on.
    pub fn declare(
        &mut self,
        spec: &MirrorSpec,
        wanted: u64,
        rate: Rate,
    ) -> Result<Declared, Diagnostic> {
        self.ensure_mirror(spec)?;
        let id = self.next_declaration;
        self.next_declaration += 1;
        let places = self.mirrors[&spec.name].places.clone();
        let mut fields = vec![None; spec.fields.len()];
        let mut columns = Vec::new();
        for (i, (field, place)) in spec.fields.iter().zip(&places).enumerate() {
            if wanted >> i & 1 == 0 {
                continue;
            }
            let col = self.ensure_column(place, field)?;
            let Some(column) = self.column_mut(col) else {
                continue;
            };
            column.demand.push((id, rate));
            fields[i] = Some(Linked {
                column: column.buf.raw(),
                depth: place.depth,
            });
            columns.push(col);
        }
        let mirror = self
            .mirrors
            .get_mut(&spec.name)
            .ok_or_else(|| unknown_mirror(&spec.name))?;
        mirror.declarations += 1;
        let (space, rows) = (mirror.space, std::ptr::from_ref(mirror.rows.raw()));
        self.declarations.insert(
            id,
            Declaration {
                mirror: spec.name.clone(),
                columns,
            },
        );
        self.relink();
        Ok(Declared {
            id,
            space: self.space_raw(space),
            rows,
            fields,
        })
    }

    /// Lets declaration `id` go: what only it asked for stops being written
    /// and is freed.
    pub fn release(&mut self, id: u32) {
        let Some(d) = self.declarations.remove(&id) else {
            return;
        };
        for col in d.columns {
            let Some(column) = self.column_mut(col) else {
                continue;
            };
            column.demand.retain(|(by, _)| *by != id);
            if column.demand.is_empty() {
                self.drop_column(col);
            }
        }
        if let Some(m) = self.mirrors.get_mut(&d.mirror) {
            m.declarations -= 1;
        }
        self.forget_unused(&d.mirror);
        self.relink();
    }

    /// Makes `writer` mirror `spec`'s provider (`host`: the game's own
    /// writers, first for every fact they write).
    pub fn provide(
        &mut self,
        spec: &MirrorSpec,
        writer: u32,
        host: bool,
    ) -> Result<(), Diagnostic> {
        self.ensure_mirror(spec)?;
        let order = self.next_order;
        let m = self
            .mirrors
            .get_mut(&spec.name)
            .ok_or_else(|| unknown_mirror(&spec.name))?;
        if m.provider.is_some_and(|p| p.writer != writer) {
            return Err(problem(
                mirror_problem::TAKEN,
                format_args!("{} has a provider already", spec.name),
            ));
        }
        m.provider = Some(Provider {
            writer,
            host,
            order,
        });
        self.next_order += 1;
        self.relink();
        Ok(())
    }

    /// `writer` is gone (its mod unloaded): its columns go to the next
    /// provider of their fact, or wait.
    pub fn withdraw(&mut self, writer: u32) {
        let gone: Vec<String> = self
            .mirrors
            .iter_mut()
            .filter(|(_, m)| m.provider.is_some_and(|p| p.writer == writer))
            .map(|(name, m)| {
                m.provider = None;
                name.clone()
            })
            .collect();
        for name in gone {
            self.forget_unused(&name);
        }
        self.relink();
    }

    /// Mirror `name`'s provider says it changed: its on-change columns are due.
    pub fn touch(&mut self, name: &str) {
        if let Some(m) = self.mirrors.get_mut(name) {
            m.touched = true;
        }
    }

    /// Mirror `name`'s rows (the provider sets a bit a row it fills).
    pub fn rows_raw(&self, name: &str) -> Option<&yog_abi::YogColumn> {
        Some(self.mirrors.get(name)?.rows.raw())
    }

    /// What's due at `tick` into `out` (emptied first): each provider with
    /// the fields it writes and someone wants now.
    pub fn due(&mut self, tick: u64, out: &mut DueList) {
        out.clear();
        for m in self.mirrors.values_mut() {
            let Some(p) = m.provider else { continue };
            let touched = std::mem::take(&mut m.touched);
            let (first, mut fields) = (out.next_column(), 0);
            for w in &mut m.writes {
                let every = w.period != 0 && tick >= w.next;
                if every {
                    w.next = tick + u64::from(w.period);
                }
                if every || (w.on_change && touched) {
                    fields |= w.bit;
                    out.push_column(w.column);
                }
            }
            if fields != 0 {
                out.turns.push(Due {
                    writer: p.writer,
                    mirror: m.name.clone(),
                    fields,
                    space: std::ptr::from_ref(self.spaces[m.space.0 as usize].keys.raw()) as u64,
                    rows: std::ptr::from_ref(m.rows.raw()) as u64,
                    columns: first..out.next_column(),
                });
            }
        }
    }

    fn column_mut(&mut self, id: ColumnId) -> Option<&mut Column> {
        self.columns.get_mut(id as usize)?.as_mut()
    }
}

fn unknown_space(name: &str) -> Diagnostic {
    problem(
        mirror_problem::UNKNOWN_SPACE,
        format_args!("no key space {name}"),
    )
}

fn unknown_mirror(name: &str) -> Diagnostic {
    problem(mirror_problem::FAILED, format_args!("no mirror {name}"))
}
