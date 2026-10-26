//! A delegate as Java writes it: a functional interface whose one method
//! is `Invoke` (the CLR's), so a lambda is one and calling one is calling
//! `Invoke`. The source's are nested in its class; the library's are the
//! runtime's (`Action2`, by the CLR's arity rule: `Index::place`).

use yog_script_analyze::ast::{Decl, Method};
use yog_script_ids::clr::INVOKE;

use super::Gen;
use crate::decls::generic_list;

impl Gen<'_> {
    /// `delegate R Name<…>(params);` as a functional interface.
    pub(crate) fn delegate_decl(&mut self, d: &Decl, m: &Method) {
        let place = self.index.place(&m.name.text, generics_of(m));
        let name = place.rsplit('.').next().unwrap_or(&place);
        let ret = m
            .ret
            .as_ref()
            .map_or_else(|| "void".to_string(), |t| self.written(t, false));
        let params = self.params(&m.params);
        let stat = if self.nested { "static " } else { "" };
        self.code.line("@FunctionalInterface");
        self.code.open(format!(
            "public {stat}interface {name}{}",
            generic_list(&m.generics)
        ));
        self.describe(d);
        self.code.line(format!("{ret} {INVOKE}({params});"));
        self.code.close("");
    }
}

/// How many type parameters `m` declares (lifetimes aside).
fn generics_of(m: &Method) -> usize {
    m.generics.iter().filter(|g| !g.lifetime).count()
}
