//! A generic object keeping the type arguments it's made with
//! (`GetType()` gives them, as .NET's): `Type.made(new List<Integer>(),
//! $kept1)`, each list of arguments a constant of the source's class —
//! worked out once, the object only handed it.

use yog_script_analyze::Type;
use yog_script_ids::jvm::reflection::{KEPT, MADE, TYPE};

use crate::Gen;
use yog_script_ids::java::{FINAL, NEW, PRIVATE, STATIC};

impl Gen<'_> {
    /// `made` (an object of type `t`), keeping `t`'s arguments when it keeps
    /// them and they're known here (a type parameter's as the code reaches it).
    pub(crate) fn made_of_type(&mut self, t: &Type, made: String) -> String {
        match self.types_list(t) {
            Some(list) => format!("{TYPE}.{MADE}({made}, {list})"),
            None => made,
        }
    }

    /// A new object of `t` (Java's `java`) made with `args`: an own generic
    /// one's type arguments handed to its constructor first (`null` where
    /// they aren't known here); another keeping them once it's made.
    pub(crate) fn new_of(&mut self, t: &Type, java: &str, args: &str) -> String {
        let Type::Own(name, targs) = t.non_null() else {
            return format!("{NEW} {java}({args})");
        };
        if !self.index.takes_types(&(name.clone(), targs.len())) {
            return self.made_of_type(t, format!("{NEW} {java}({args})"));
        }
        let list = self.types_list(t).unwrap_or_else(|| yog_script_ids::java::NULL.into());
        let sep = if args.is_empty() { "" } else { ", " };
        format!("{NEW} {java}({list}{sep}{args})")
    }

    /// The type arguments `t` is made with, as a `Type[]` (`$kept1`, or made
    /// in place where one is a type parameter's); none when `t` keeps none
    /// or one isn't known here.
    pub(crate) fn types_list(&mut self, t: &Type) -> Option<String> {
        let Type::Own(name, args) = t.non_null() else {
            return None;
        };
        if !self.index.keeps_types(&(name.clone(), args.len())) {
            return None;
        }
        let values = args
            .iter()
            .map(|a| self.value_of(a))
            .collect::<Option<Vec<_>>>()?;
        let list = format!("{NEW} {TYPE}[] {{ {} }}", values.join(", "));
        // One of a type parameter's is what the code reaches: made each time.
        Some(if args.iter().any(crate::reified::names_parameter) {
            list
        } else {
            self.kept(list)
        })
    }

    /// Arguments `list`'s constant (the library's files have no class of a
    /// source to hold it: there, the list itself).
    fn kept(&mut self, list: String) -> String {
        if !self.nested {
            return list;
        }
        let at = match self.kept_types.iter().position(|l| *l == list) {
            Some(at) => at,
            None => {
                self.kept_types.push(list);
                self.kept_types.len() - 1
            }
        };
        format!("{KEPT}{}", at + 1)
    }

    /// The constants the source's objects' arguments are, in its class.
    pub(crate) fn kept_types_constants(&mut self) {
        for (at, list) in std::mem::take(&mut self.kept_types).iter().enumerate() {
            self.code.line(format!(
                "{PRIVATE} {STATIC} {FINAL} {TYPE}[] {KEPT}{} = {list};",
                at + 1
            ));
        }
    }
}
