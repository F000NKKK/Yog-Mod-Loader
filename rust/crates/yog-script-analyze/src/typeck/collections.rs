//! Collection expressions, as C# 12's: `[a, b, ..rest]` a `List<T>` of
//! its items, `[k: v]` a `Dictionary<K, V>` — or, where a collection type
//! is wanted (`HashSet<i32> s = [1, 2];`), one of that, each item fitting it.

use super::Checker;
use super::model::Type;
use crate::ast::{Element, Expr};
use yog_script_ids::system::{DICTIONARY, KEY_VALUE_PAIR, LIST};

impl Checker<'_, '_, '_> {
    /// `[…]` alone: its natural type (unknown when it's empty).
    pub(super) fn collection(&mut self, items: &[Element]) -> Type {
        let mut item = Type::Unknown;
        let mut entry = None;
        for i in items {
            let t = match i {
                Element::Item(e) => self.expr(e),
                Element::Spread(e) => {
                    let all = self.expr(e);
                    self.item_of(&all)
                }
                Element::Entry(k, v) => {
                    let kv = (self.expr(k), self.expr(v));
                    entry.get_or_insert(kv);
                    continue;
                }
            };
            if !item.known() {
                item = t;
            }
        }
        match entry {
            Some((k, v)) => Type::Own(DICTIONARY.into(), vec![k, v]),
            None if item.known() => Type::Own(LIST.into(), vec![item]),
            None => Type::Unknown,
        }
    }

    /// `[…]` where `wanted` is: each item fitting what going through a
    /// `wanted` gives (a dictionary's entries its key and value). Whether
    /// `wanted` is a collection it makes.
    pub(super) fn collection_as(&mut self, e: &Expr, items: &[Element], wanted: &Type) -> bool {
        let item = self.item_of(wanted);
        // C#'s collection expressions make one-dimensional arrays only.
        if !item.known() || matches!(wanted, Type::Array(_, rank) if *rank > 1) {
            return false;
        }
        let pair = match &item {
            Type::Own(name, args) if name == KEY_VALUE_PAIR && args.len() == 2 => {
                Some((args[0].clone(), args[1].clone()))
            }
            _ => None,
        };
        for i in items {
            match (i, &pair) {
                (Element::Entry(k, v), Some((kt, vt))) => {
                    self.fits(k, kt);
                    self.fits(v, vt);
                }
                (Element::Spread(s), _) => {
                    let all = self.expr(s);
                    let got = self.item_of(&all);
                    self.want(&got, &item, s.span);
                }
                (Element::Item(x) | Element::Entry(x, _), _) => self.fits(x, &item),
            }
        }
        self.types.set(e, wanted.clone());
        true
    }
}
