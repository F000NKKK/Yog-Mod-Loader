//! A property's own stored value (C#'s `field`): `i32 Level { get; set =>
//! field = Math.Max(value, 0); }`. A property whose accessors have bodies is
//! Java's methods; one reaching `field`, or with an accessor without a body
//! beside one with, keeps its value in a field of its own (`Level$field`),
//! what `field` is in its accessors and what its accessor without a body
//! reads or writes.

use yog_script_analyze::ast::{Accessor, Decl, DeclKind};
use yog_script_analyze::words::field;
use yog_script_ids::java::{PRIVATE, THIS};
use yog_script_ids::jvm::BACKING;

use crate::Gen;

impl Gen<'_> {
    /// The fields of `body`'s properties keeping their own value.
    pub(crate) fn backing_fields(&mut self, body: &[Decl]) {
        for (ty, name) in backed(body) {
            let t = self.written(ty, false);
            self.code
                .line(format!("{PRIVATE} {t} {};", backing_name(name)));
        }
    }

    /// `f` with `field` naming property `name`'s own value, when it keeps one.
    pub(crate) fn with_backing<R>(
        &mut self,
        name: &str,
        accessors: &[Accessor],
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        if !yog_script_analyze::ast::keeps_value(accessors) {
            return f(self);
        }
        self.renamed.push((
            field::WORD.text.to_string(),
            format!("{THIS}.{}", backing_name(name)),
        ));
        let out = f(self);
        self.renamed.pop();
        out
    }
}

/// Property `name`'s own value's field.
pub(crate) fn backing_name(name: &str) -> String {
    format!("{name}{BACKING}")
}

/// The properties of `body` keeping their own value: their types and names.
pub(crate) fn backed(body: &[Decl]) -> Vec<(&yog_script_analyze::ast::Type, &str)> {
    body.iter()
        .filter_map(|d| match &d.kind {
            DeclKind::Property {
                ty,
                name,
                accessors,
                ..
            } if yog_script_analyze::ast::keeps_value(accessors) => Some((ty, name.text.as_str())),
            _ => None,
        })
        .collect()
}
