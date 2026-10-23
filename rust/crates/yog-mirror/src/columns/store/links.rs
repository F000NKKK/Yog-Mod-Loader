//! The store's directories kept straight: mirrors made and merged, fields
//! placed in their facts' columns, columns made and freed, and after each
//! change every link, writer and merged demand worked out again.

use yog_data::Diagnostic;
use yog_ids::{mirror_problem, sep};

use super::{
    Column, ColumnId, MAX_FIELDS, Mirror, Place, Store, Write, fact_space, problem, unknown_space,
};
use crate::columns::buffer::Buffer;
use crate::columns::keys::{BIT_WORD, bit_words};
use crate::spec::{FieldSpec, MirrorSpec, Rate};

impl Store {
    /// Mirror `spec` known: made, or merged with what's known of it (a
    /// spec whose fields extend the known one's replaces it).
    pub(super) fn ensure_mirror(&mut self, spec: &MirrorSpec) -> Result<(), Diagnostic> {
        if spec.fields.len() > MAX_FIELDS {
            let n = spec.fields.len();
            return Err(problem(
                mirror_problem::TOO_MANY_FIELDS,
                format_args!("{} has {n} fields, {MAX_FIELDS} at most", spec.name),
            ));
        }
        let space = self.ensure_space(&spec.space)?;
        let places = spec
            .fields
            .iter()
            .map(|f| self.place(spec, f))
            .collect::<Result<Vec<_>, _>>()?;
        if let Some(known) = self.mirrors.get_mut(&spec.name) {
            return merge(known, spec, places);
        }
        let rows = Buffer::new(BIT_WORD, bit_words(self.space_of(space).keys.capacity()));
        let mirror = Mirror {
            name: spec.name.as_str().into(),
            spec: spec.clone(),
            space,
            columns: vec![None; places.len()],
            places,
            rows,
            provider: None,
            writes: Vec::new(),
            declarations: 0,
            touched: false,
            scopes: Vec::new(),
        };
        self.mirrors.insert(spec.name.clone(), mirror);
        Ok(())
    }

    /// Where `field` of `mirror` is kept.
    fn place(&self, mirror: &MirrorSpec, field: &FieldSpec) -> Result<Place, Diagnostic> {
        let fact = field
            .fact
            .clone()
            .unwrap_or_else(|| format!("{}{}{}", mirror.name, sep::PATH, field.name));
        let space_name = fact_space(&fact).unwrap_or_else(|| mirror.space.clone());
        // A family's fact is kept in the member the mirror is of.
        if self.is_family(&space_name) && Self::member_of(&space_name, &mirror.space) {
            let space = self
                .space_id(&mirror.space)
                .ok_or_else(|| unknown_space(&mirror.space))?;
            return Ok(Place {
                space,
                fact,
                depth: 0,
            });
        }
        let space = self
            .space_id(&space_name)
            .ok_or_else(|| unknown_space(&space_name))?;
        let mut at = Some(
            self.space_id(&mirror.space)
                .ok_or_else(|| unknown_space(&mirror.space))?,
        );
        let mut depth = 0u8;
        while let Some(s) = at {
            if s == space {
                return Ok(Place { space, fact, depth });
            }
            at = self.space_of(s).parent;
            depth += 1;
        }
        Err(problem(
            mirror_problem::FOREIGN_FACT,
            format_args!(
                "{fact} is of {space_name}, which {} isn't part of",
                mirror.space
            ),
        ))
    }

    /// The column of `place`, made if no one asked for it yet.
    pub(super) fn ensure_column(
        &mut self,
        place: &Place,
        field: &FieldSpec,
    ) -> Result<ColumnId, Diagnostic> {
        let key = (place.space, place.fact.clone());
        if let Some(&id) = self.by_fact.get(&key) {
            let c = self.columns[id as usize].as_ref();
            if c.is_some_and(|c| c.kind != field.kind || c.count != field.count) {
                return Err(problem(
                    mirror_problem::WRONG_SHAPE,
                    format_args!(
                        "{} is kept in another shape than field {}'s",
                        place.fact, field.name
                    ),
                ));
            }
            return Ok(id);
        }
        let width = field.width().ok_or_else(|| {
            problem(
                mirror_problem::FAILED,
                format_args!("field {} has no kind {}", field.name, field.kind),
            )
        })?;
        let column = Column {
            buf: Buffer::new(width, self.space_of(place.space).keys.capacity()),
            space: place.space,
            fact: place.fact.clone(),
            kind: field.kind,
            count: field.count,
            demand: Vec::new(),
            writer: None,
        };
        let id = match self.columns.iter().position(Option::is_none) {
            Some(free) => {
                self.columns[free] = Some(column);
                free
            }
            None => {
                self.columns.push(Some(column));
                self.columns.len() - 1
            }
        } as ColumnId;
        self.by_fact.insert(key, id);
        Ok(id)
    }

    /// Column `id` freed (no one asks for it any more).
    pub(super) fn drop_column(&mut self, id: ColumnId) {
        if let Some(c) = self.columns.get_mut(id as usize).and_then(Option::take) {
            self.by_fact.remove(&(c.space, c.fact));
        }
    }

    /// Mirror `name` forgotten when no one declares or provides it.
    pub(super) fn forget_unused(&mut self, name: &str) {
        if self
            .mirrors
            .get(name)
            .is_some_and(|m| m.declarations == 0 && m.provider.is_none())
        {
            self.mirrors.remove(name);
        }
    }

    /// Every mirror's links to the columns there are, every column's
    /// writer, and what each provider writes at which rate, worked out again.
    pub(super) fn relink(&mut self) {
        for m in self.mirrors.values_mut() {
            for (col, place) in m.columns.iter_mut().zip(&m.places) {
                *col = self
                    .by_fact
                    .get(&(place.space, place.fact.clone()))
                    .copied();
            }
        }
        for (id, c) in self.columns.iter_mut().enumerate() {
            let Some(c) = c else { continue };
            c.writer = self
                .mirrors
                .values()
                .filter(|m| m.columns.contains(&Some(id as ColumnId)))
                .filter_map(|m| m.provider)
                .min_by_key(|p| (!p.host, p.order))
                .map(|p| p.writer);
        }
        self.in_use = vec![false; self.spaces.len()];
        let read: Vec<_> = self
            .mirrors
            .values()
            .filter(|m| m.declarations > 0)
            .map(|m| m.space)
            .collect();
        for space in read {
            let mut at = Some(space);
            while let Some(s) = at {
                self.in_use[s.0 as usize] = true;
                at = self.space_of(s).parent;
            }
        }
        for m in self.mirrors.values_mut() {
            let writer = m.provider.map(|p| p.writer);
            let writes = m
                .columns
                .iter()
                .enumerate()
                .filter_map(|(i, col)| {
                    let c = self.columns[(*col)? as usize].as_ref()?;
                    (writer.is_some() && c.writer == writer).then(|| write_of(i, c))
                })
                .map(|mut w| {
                    // A field written before keeps its turn.
                    if let Some(old) = m.writes.iter().find(|o| o.bit == w.bit) {
                        w.next = old.next;
                    }
                    w
                })
                .collect();
            m.writes = writes;
        }
    }
}

/// Field `i`'s demand, merged: the most frequent period, any on change.
fn write_of(i: usize, c: &Column) -> Write {
    let period = c
        .demand
        .iter()
        .filter_map(|(_, r)| match r {
            Rate::Every(n) => Some((*n).max(1)),
            Rate::Changes => None,
        })
        .min()
        .unwrap_or(0);
    Write {
        bit: 1 << i,
        column: std::ptr::from_ref(c.buf.raw()) as u64,
        period,
        next: 0,
        on_change: c.demand.iter().any(|(_, r)| *r == Rate::Changes),
    }
}

/// `spec` merged into the mirror known by its name.
fn merge(known: &mut Mirror, spec: &MirrorSpec, places: Vec<Place>) -> Result<(), Diagnostic> {
    let (old, new) = (&known.spec.fields, &spec.fields);
    let extends = |a: &[FieldSpec], b: &[FieldSpec]| a.len() <= b.len() && b[..a.len()] == *a;
    if known.spec.space != spec.space || !(extends(old, new) || extends(new, old)) {
        return Err(problem(
            mirror_problem::TAKEN,
            format_args!("{} is declared otherwise already", spec.name),
        ));
    }
    if new.len() > old.len() {
        known.spec = spec.clone();
        known.columns.resize(places.len(), None);
        known.places = places;
    }
    Ok(())
}
