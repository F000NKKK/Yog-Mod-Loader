//! Of the overloads a call fits, the best (C#'s better function member):
//! one whose parameters take the arguments as they are, or are more
//! specific (`Task<T>[]` over `Task[]`), at least once and never worse;
//! else a non-generic one over a generic one; else the first declared.

use std::cmp::Ordering;

use super::Checker;
use super::generics::Bound;
use super::members::Signature;
use super::model::Type;

/// An overload a call fits: its index, its type parameters bound, its
/// `params` expanded form (when that's what fits).
pub(super) type Fit = (usize, Bound, Option<Signature>);

impl Checker<'_, '_, '_> {
    /// The best of `fits` (of `sigs`) for arguments `given`.
    pub(super) fn best(&self, sigs: &[Signature], fits: Vec<Fit>, given: &[Type]) -> Option<Fit> {
        let mut fits = fits.into_iter();
        let mut best = fits.next()?;
        for f in fits {
            if self.better(sigs, (&f, &best), given) == Ordering::Greater {
                best = f;
            }
        }
        Some(best)
    }

    /// How fit `a` compares with fit `b` for `given`.
    fn better(&self, sigs: &[Signature], (a, b): (&Fit, &Fit), given: &[Type]) -> Ordering {
        let (pa, pb) = (taken(sigs, a), taken(sigs, b));
        let mut out = Ordering::Equal;
        for (g, (x, y)) in given.iter().zip(pa.iter().zip(&pb)) {
            match (out, self.better_conversion(g, x, y)) {
                (_, Ordering::Equal) => {}
                (Ordering::Equal, o) => out = o,
                (o, p) if o != p => return Ordering::Equal,
                _ => {}
            }
        }
        if out != Ordering::Equal {
            return out;
        }
        let generic = |f: &Fit| !sigs[f.0].generics.is_empty();
        match (generic(a), generic(b)) {
            (false, true) => Ordering::Greater,
            (true, false) => Ordering::Less,
            _ => Ordering::Equal,
        }
    }

    /// Which of parameters `x` and `y` takes a `g` better: as it is, else
    /// the more specific (one converting to the other).
    fn better_conversion(&self, g: &Type, x: &Type, y: &Type) -> Ordering {
        if x == y || !g.known() {
            return Ordering::Equal;
        }
        if g == x {
            return Ordering::Greater;
        }
        if g == y {
            return Ordering::Less;
        }
        match (self.converts_plainly(x, y), self.converts_plainly(y, x)) {
            (true, false) => Ordering::Greater,
            (false, true) => Ordering::Less,
            _ => Ordering::Equal,
        }
    }
}

/// The parameters fit `f` takes its arguments as, its type parameters bound.
fn taken(sigs: &[Signature], f: &Fit) -> Vec<Type> {
    let s = f.2.as_ref().unwrap_or(&sigs[f.0]);
    s.params.iter().map(|p| p.substitute(&f.1)).collect()
}
