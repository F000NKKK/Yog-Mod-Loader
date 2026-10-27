//! `saved` members on the JVM (docs/design/storage.md): an object keeping
//! some implements `State.Saved` — each member a record of its own
//! (`$Visits(int value)`, Yog Data by the bus's codec) keyed
//! `Owner.Member` in its owner's store; `$load` reads them in as the object
//! is made live, `$save` writes them out as the world saves.

use yog_script_analyze::Type;
use yog_script_analyze::ast::{Decl, DeclKind, Modifier, Type as Written};
use yog_script_ids::java::{IF, NULL, OVERRIDE, PUBLIC, RECORD, VOID};
use yog_script_ids::jvm::state::{
    ACCEPT, APPLY, BYTES, FORGET, FROM_BYTES, LOAD, READ, READ_TYPE, RECORD_PREFIX, SAVE, VALUE, WRITE, WRITE_TYPE,
};

use super::Gen;

/// The `saved` members of `body`: each its type as written and its name.
pub(crate) fn saved_members(body: &[Decl]) -> Vec<(&Written, &str)> {
    body.iter()
        .filter(|d| d.has(Modifier::Saved))
        .filter_map(|d| match &d.kind {
            DeclKind::Field(f) => Some((&f.ty, f.name.text.as_str())),
            DeclKind::Property { ty, name, .. } if yog_script_analyze::ast::stores_value(d) => Some((ty, name.text.as_str())),
            _ => None,
        })
        .collect()
}

impl Gen<'_> {
    /// `State.Saved`'s members for `body`'s `saved` ones, keyed `owner.Member`;
    /// `forget` what `$forget` does.
    pub(crate) fn saved_state(&mut self, owner: &str, body: &[Decl], forget: &str) {
        let members = saved_members(body);
        let mut load = Vec::new();
        let mut save = Vec::new();
        for (ty, name) in &members {
            if !self.keeps(ty) {
                return self.not_yet("saving a value of this type", ty.span);
            }
            let (field, record) = (Self::ident(name), format!("{RECORD_PREFIX}{name}"));
            let t = self.written(ty, false);
            self.code.line(format!("{PUBLIC} {RECORD} {record}({t} {VALUE}) {{}}"));
            let key = self.code.str(&format!("{owner}.{name}"));
            let bytes = self.temp();
            load.push(format!(
                "byte[] {bytes} = {READ}.{APPLY}({key}); {IF} ({bytes} != {NULL}) {{ {field} = {FROM_BYTES}({bytes}, {record}.class).{VALUE}(); }}"
            ));
            save.push(format!("{WRITE}.{ACCEPT}({key}, {BYTES}(new {record}({field})));"));
        }
        self.state_method(LOAD, &format!("{READ_TYPE} {READ}"), &load);
        self.state_method(SAVE, &format!("{WRITE_TYPE} {WRITE}"), &save);
        self.state_method(FORGET, "", &[forget.to_string()]);
    }

    /// `@Override public void name(params) { lines }`.
    fn state_method(&mut self, name: &str, params: &str, lines: &[String]) {
        self.code.line(OVERRIDE);
        self.code.open(format!("{PUBLIC} {VOID} {name}({params})"));
        for l in lines.iter().filter(|l| !l.is_empty()) {
            self.code.line(l);
        }
        self.code.close("");
    }

    /// Whether the bus's codec keeps a value of `ty` as it is (what the
    /// analyzer allows beyond is said not compiled yet).
    fn keeps(&self, ty: &Written) -> bool {
        let yog_script_analyze::ast::TypeKind::Path { path, args } = &ty.kind else {
            return false;
        };
        let Some(name) = path.last().map(|n| n.text.as_str()).filter(|_| args.is_empty()) else {
            return false;
        };
        let t = yog_script_analyze::builtin_type(name)
            .or_else(|| yog_script_analyze::system_type(name))
            .or_else(|| self.index.is_unit(name).then(|| Type::Unit(name.to_string())));
        matches!(
            t,
            Some(
                Type::Bool
                    | Type::I8
                    | Type::I16
                    | Type::I32
                    | Type::I64
                    | Type::U8
                    | Type::U16
                    | Type::U32
                    | Type::U64
                    | Type::F32
                    | Type::F64
                    | Type::String
                    | Type::Unit(_)
                    | Type::Content(_)
                    | Type::Tag(_)
            )
        )
    }
}
