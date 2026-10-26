//! The library's collections in Java: a collection expression made as
//! C#'s collection initializer does (`new List<T>()`, then `Add` each item,
//! `Add(k, v)` each entry, every item of a spread), and Yog's lists and
//! maps read off its values as the library's, not copied (`List.view`).

use yog_script_analyze::Type;
use yog_script_analyze::ast::{Element, Expr};
use yog_script_ids::system::{DICTIONARY, LIST};

use super::Gen;
use crate::index::Kind;

use yog_script_ids::java::{FOR, NEW, VAR};
use yog_script_ids::jvm::{ARRAY_LIST, VIEW};

impl Gen<'_> {
    /// `[a, ..rest]`, `[k => v]`: made where it's met, its temporary its value.
    pub(crate) fn collection(&mut self, e: &Expr, items: &[Element]) -> String {
        if let Some(Type::Array(..)) = self.types.of(e).map(Type::non_null) {
            return self.array_collection(e, items);
        }
        let Some(Type::Own(name, args)) = self.types.of(e).map(Type::non_null).cloned() else {
            self.not_yet(
                "a collection of a type not known (`List<T> xs = [];`)",
                e.span,
            );
            return String::new();
        };
        // An interface wanted (`IEnumerable<T>`): a list, as C#'s; a
        // dictionary for one of a key and a value.
        let made = if self.index.kind(&(name.clone(), args.len())) == Some(Kind::Interface) {
            let concrete = if args.len() == 2 { DICTIONARY } else { LIST };
            Type::Own(concrete.to_string(), args)
        } else {
            Type::Own(name, args)
        };
        let java = self.java(&made, false);
        let new = self.new_of(&made, &java, "");
        let t = self.temp();
        self.code.line(format!("{VAR} {t} = {new};"));
        for i in items {
            match i {
                Element::Item(x) => {
                    let v = self.handed_expr(x);
                    self.code.line(format!("{t}.Add({v});"));
                }
                Element::Entry(k, v) => {
                    let (k, v) = (self.handed_expr(k), self.handed_expr(v));
                    self.code.line(format!("{t}.Add({k}, {v});"));
                }
                Element::Spread(s) => {
                    let (all, item) = (self.expr(s), self.temp());
                    self.code
                        .line(format!("{FOR} ({VAR} {item} : {all}) {t}.Add({item});"));
                }
            }
        }
        t
    }

    /// `[a, ..rest]`'s items gathered in Java's list of their boxed type
    /// (`e` an array's): its temporary.
    pub(crate) fn collection_of_list(&mut self, e: &Expr, items: &[Element]) -> String {
        let item = match self.types.of(e) {
            Some(Type::Array(item, _)) => self.java(item, true),
            _ => yog_script_ids::jvm::OBJECT.to_string(),
        };
        let t = self.temp();
        self.code
            .line(format!("{VAR} {t} = {NEW} {ARRAY_LIST}<{item}>();"));
        for i in items {
            match i {
                Element::Item(x) | Element::Entry(x, _) => {
                    let v = self.handed_expr(x);
                    self.code.line(format!("{t}.add({v});"));
                }
                Element::Spread(s) => {
                    let (all, one) = (self.expr(s), self.temp());
                    self.code
                        .line(format!("{FOR} ({VAR} {one} : {all}) {t}.add({one});"));
                }
            }
        }
        t
    }

    /// `read`, a Yog value's field (`e`): a list or a map seen as the
    /// library's.
    pub(crate) fn viewed(&self, e: &Expr, read: String) -> String {
        match self.types.of(e).map(Type::non_null) {
            Some(Type::Own(name, args)) if name == LIST || name == DICTIONARY => {
                let place = self.index.place(name, args.len());
                format!("{place}.{VIEW}({read})")
            }
            _ => read,
        }
    }
}
