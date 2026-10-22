//! Typed mirrors: what `#[yog_mirror]` implements on a struct — its spec,
//! a view a row (`ThreatView`: a getter a field) and a writer a row
//! (`ThreatWriter`: a setter a field, given a closure called only when due).

use std::marker::PhantomData;

use yog_data::{Diagnostic, Schema};
use yog_ids::{sep, service_mirror};
use yog_sync::Arc;

use super::{Cell, Declared, FieldSpec, MirrorSpec, Rate, Writer};
use crate::registry::Registry;

/// A mirror's row type (`#[yog_mirror]` implements it).
pub trait Mirror: Sized + 'static {
    /// Its name's last part: `yog:mirror/<mod>/<NAME>`.
    const NAME: &'static str;
    /// The mod providing it; `None`: this one.
    const OWNER: Option<&'static str>;
    /// Its key space (`yog_ids::key_space::*`).
    const SPACE: &'static str;
    /// Every field's bit.
    const ALL: u64;
    /// A row read.
    type View<'a>: From<Row<'a, Self>>;
    /// A row written.
    type Writer<'w>: From<RowWriter<'w, Self>>;

    /// Its fields, in order.
    fn fields() -> Vec<FieldSpec>;

    /// The whole row (a field not asked for reads as its default).
    fn read(row: &Row<'_, Self>) -> Self;

    /// Every due field of `row` set from `self`.
    fn write(&self, row: &mut RowWriter<'_, Self>);

    /// Its address.
    fn address() -> String {
        let owner = Self::OWNER.unwrap_or_else(crate::mod_id);
        format!(
            "{}{owner}{}{}",
            service_mirror::DOMAIN,
            sep::PATH,
            Self::NAME
        )
    }

    fn spec() -> MirrorSpec {
        MirrorSpec {
            name: Self::address(),
            space: Self::SPACE.to_owned(),
            fields: Self::fields(),
        }
    }

    /// Asks for `fields` (its bit consts or-ed) at `rate`.
    fn declare(fields: u64, rate: Rate) -> Result<Mirrored<Self>, Diagnostic> {
        Ok(Mirrored {
            declared: Declared::new(&Self::spec(), fields, rate)?,
            _mirror: PhantomData,
        })
    }

    /// [`Mirror::declare`] in member `member` of its family (a dimension:
    /// `minecraft:overworld`), covering `scope` (its scope type: a
    /// `BlockScope`…).
    fn declare_in<S: Schema>(
        member: &str,
        scope: Option<&S>,
        fields: u64,
        rate: Rate,
    ) -> Result<Mirrored<Self>, Diagnostic> {
        let mut spec = Self::spec();
        spec.name = format!("{}{}{member}", spec.name, sep::PATH);
        spec.space = format!("{}{}{member}", spec.space, sep::PATH);
        let scope = scope.map(yog_data::frame::to_frame);
        Ok(Mirrored {
            declared: Declared::scoped(&spec, fields, rate, scope)?,
            _mirror: PhantomData,
        })
    }
}

/// A typed declaration: its rows read in place; dropped, released.
pub struct Mirrored<M: Mirror> {
    declared: Declared,
    _mirror: PhantomData<M>,
}

impl<M: Mirror> Mirrored<M> {
    /// Every row its provider fills.
    pub fn iter(&self) -> impl Iterator<Item = M::View<'_>> {
        self.declared
            .rows()
            .map(|row| Row::new(&self.declared, row).into())
    }

    /// `key`'s row.
    pub fn get(&self, key: u64) -> Option<M::View<'_>> {
        Some(Row::new(&self.declared, self.declared.row_of(key)?).into())
    }

    pub fn declared(&self) -> &Declared {
        &self.declared
    }
}

/// A row being read (what a generated view wraps).
pub struct Row<'a, M> {
    declared: &'a Declared,
    row: u32,
    _mirror: PhantomData<M>,
}

impl<'a, M> Row<'a, M> {
    fn new(declared: &'a Declared, row: u32) -> Self {
        Row {
            declared,
            row,
            _mirror: PhantomData,
        }
    }

    pub fn row(&self) -> u32 {
        self.row
    }

    /// Its key.
    pub fn key(&self) -> Option<u64> {
        self.declared.key_of(self.row)
    }

    /// Field `field`'s value; its default when it wasn't asked for.
    #[inline]
    pub fn field<T: Cell>(&self, field: usize) -> T {
        self.declared.get(field, self.row).unwrap_or_default()
    }
}

/// A row being written (what a generated writer wraps).
pub struct RowWriter<'w, M> {
    writer: &'w mut Writer,
    row: u32,
    _mirror: PhantomData<M>,
}

impl<M> RowWriter<'_, M> {
    pub fn row(&self) -> u32 {
        self.row
    }

    /// Sets field `field` to what `value` gives, if it's due.
    #[inline]
    pub fn set<T: Cell>(&mut self, field: usize, value: impl FnOnce() -> T) {
        if self.writer.wants(field) {
            self.writer.set(field, self.row, value);
        }
    }
}

/// A typed provider's turn.
pub struct MirrorWriter<'w, M> {
    writer: &'w mut Writer,
    _mirror: PhantomData<M>,
}

impl<M: Mirror> MirrorWriter<'_, M> {
    /// `key`'s row to write, marked as filled; `None` when its space hasn't the key.
    pub fn row(&mut self, key: u64) -> Option<M::Writer<'_>> {
        let row = self.writer.row(key)?;
        Some(
            RowWriter {
                writer: &mut *self.writer,
                row,
                _mirror: PhantomData,
            }
            .into(),
        )
    }

    /// Row `row` no longer filled.
    pub fn leave(&mut self, row: u32) {
        self.writer.leave(row);
    }

    /// The family member written now (a dimension), when its space is a family's.
    pub fn member(&self) -> Option<&str> {
        let base = M::address();
        self.writer
            .mirror()
            .strip_prefix(&base)?
            .strip_prefix(sep::PATH)
    }

    /// What the declarations asked it to cover.
    pub fn scopes<S: Schema>(&self) -> impl Iterator<Item = S> + '_ {
        self.writer
            .scopes()
            .iter()
            .filter_map(|b| yog_data::frame::read_frame(b))
    }

    /// Whether any of `fields` is due.
    pub fn wants(&self, fields: u64) -> bool {
        (0..u64::BITS as usize).any(|i| fields >> i & 1 == 1 && self.writer.wants(i))
    }
}

impl Registry {
    /// Provides mirror `M`: `write` fills what's due each time it is.
    pub fn provide_mirror_of<M: Mirror>(
        &mut self,
        write: impl Fn(&mut MirrorWriter<'_, M>) + Send + Sync + 'static,
    ) -> bool {
        let write = Arc::new(move |w: &mut Writer| {
            write(&mut MirrorWriter {
                writer: w,
                _mirror: PhantomData,
            })
        });
        self.providing_mirror(&M::spec(), false, write)
    }
}
