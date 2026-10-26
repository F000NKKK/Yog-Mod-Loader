//! A generic type's static fields, one set per list of its arguments as
//! C#'s (`Box<int>.Count` isn't `Box<string>.Count`): kept in its nested
//! `$Statics`, each set made the first time its arguments are reached
//! (`Box.<Integer>$statics(args).Count`), its fields' first values set then.

use yog_script_analyze::Type;
use yog_script_analyze::ast::{Decl, DeclKind, Modifier, TypeHeader};
use yog_script_ids::java::{CLASS, FINAL, NEW, NULL, PRIVATE, PUBLIC, RETURN, STATIC, UNCHECKED};
use yog_script_ids::jvm::reflection::{
    STATICS, STATICS_CLASS, STATICS_FIELD, STATICS_METHOD, STATICS_OF, TYPE, TYPES_FIELD,
};

use crate::decls::generic_list;
use crate::{Gen, modifier};

impl Gen<'_> {
    /// The `$Statics` of an own generic type `key` declaring static fields in
    /// `body`, and its sets; nothing for another type.
    pub(crate) fn statics_holder(
        &mut self,
        key: &(String, usize),
        header: &TypeHeader,
        body: &[Decl],
    ) {
        let fields = per_type_fields(body);
        if fields.is_empty() || !self.index.takes_types(key) {
            return;
        }
        let generics = generic_list(&header.generics);
        self.code.open(format!(
            "{PUBLIC} {STATIC} {FINAL} {CLASS} {STATICS_CLASS}{generics}"
        ));
        for (d, f) in &fields {
            let fin = if d.has(Modifier::Readonly) {
                modifier(FINAL)
            } else {
                String::new()
            };
            let t = self.written(&f.ty, false);
            self.code
                .line(format!("{PUBLIC} {fin}{t} {};", Self::ident(&f.name.text)));
        }
        self.code
            .open(format!("{STATICS_CLASS}({TYPE}[] {TYPES_FIELD})"));
        let params = self
            .owner_generics
            .iter()
            .enumerate()
            .map(|(i, g)| (g.clone(), format!("{TYPES_FIELD}[{i}]")))
            .collect();
        let (statik, in_statics) = (self.statik, self.in_statics);
        (self.statik, self.in_statics) = (true, true);
        self.with_type_params(params, |g| {
            for (_, f) in &fields {
                if let Some(v) = &f.value {
                    let v = g.expr(v);
                    g.code.line(format!("{} = {v};", Self::ident(&f.name.text)));
                }
            }
        });
        (self.statik, self.in_statics) = (statik, in_statics);
        self.code.close("");
        self.code.close("");
        let wild = generic_wildcards(header);
        self.code.line(format!(
            "{PRIVATE} {STATIC} {FINAL} {STATICS}<{STATICS_CLASS}{wild}> {STATICS_FIELD} = {NEW} {STATICS}<>({STATICS_CLASS}::{NEW});"
        ));
        self.code.line(UNCHECKED);
        self.code.open(format!(
            "{PUBLIC} {STATIC} {generics} {STATICS_CLASS}{generics} {STATICS_METHOD}({TYPE}[] {TYPES_FIELD})"
        ));
        self.code.line(format!(
            "{RETURN} ({STATICS_CLASS}{generics}) {STATICS_FIELD}.{STATICS_OF}({TYPES_FIELD});"
        ));
        self.code.close("");
    }

    /// Static field `name` of own generic type `of` with arguments `args`:
    /// its set's (`Box.<Integer>$statics(args).Count`).
    pub(crate) fn per_type_static(&mut self, of: &str, args: &[Type], name: &str) -> String {
        let witness: Vec<String> = args.iter().map(|a| self.java(a, true)).collect();
        let values: Vec<String> = args
            .iter()
            .map(|a| self.value_of(a).unwrap_or_else(|| NULL.into()))
            .collect();
        self.statics_of(of, &witness, &values, name)
    }

    /// Static field `name` of the type being written, by its name alone:
    /// the set of its own arguments as the code reaches them (in its
    /// `$Statics`' constructor, the field itself).
    pub(crate) fn own_static(&mut self, name: &str) -> String {
        if self.in_statics {
            return Self::ident(name);
        }
        let Some((of, _)) = self.owner.clone() else {
            return Self::ident(name);
        };
        let witness = self.owner_generics.clone();
        let values: Vec<String> = witness
            .iter()
            .map(|g| self.type_param(g).unwrap_or_else(|| NULL.into()))
            .collect();
        self.statics_of(&of, &witness, &values, name)
    }

    fn statics_of(
        &mut self,
        of: &str,
        witness: &[String],
        values: &[String],
        name: &str,
    ) -> String {
        format!(
            "{}.<{}>{STATICS_METHOD}({NEW} {TYPE}[] {{ {} }}).{}",
            self.index.place(of, witness.len()),
            witness.join(", "),
            values.join(", "),
            Self::ident(name)
        )
    }
}

/// The static fields of `body` (not its constants), each with its declaration.
pub(crate) fn per_type_fields(body: &[Decl]) -> Vec<(&Decl, &yog_script_analyze::ast::Field)> {
    body.iter()
        .filter_map(|d| match &d.kind {
            DeclKind::Field(f) if d.has(Modifier::Static) && !d.has(Modifier::Const) => {
                Some((d, f))
            }
            _ => None,
        })
        .collect()
}

/// `<?, ?>` for a type of `header`'s arity.
fn generic_wildcards(header: &TypeHeader) -> String {
    let n = header.generics.iter().filter(|g| !g.lifetime).count();
    format!("<{}>", vec![yog_script_ids::java::WILDCARD; n].join(", "))
}
