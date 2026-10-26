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
    /// them and they're known here (generic code not knowing its `T` can't).
    pub(crate) fn made_of_type(&mut self, t: &Type, made: String) -> String {
        let Type::Own(name, args) = t.non_null() else {
            return made;
        };
        if !self.index.keeps_types(&(name.clone(), args.len())) {
            return made;
        }
        let Some(values) = args
            .iter()
            .map(|a| self.value_of(a))
            .collect::<Option<Vec<_>>>()
        else {
            return made;
        };
        let list = format!("{NEW} {TYPE}[] {{ {} }}", values.join(", "));
        format!("{TYPE}.{MADE}({made}, {})", self.kept(list))
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
