//! `saved` members on the JVM (docs/design/storage.md): an object keeping
//! some implements `State.Saved` — each member a record of its own
//! (`$Visits(int value)`, Yog Data by the bus's codec) keyed
//! `Owner.Member` in its owner's store; `$load` reads them in as the object
//! is made live, `$save` writes them out as the world saves. What the codec
//! doesn't keep as it is goes as what it does: a `T?` an `Optional<T>`, a
//! `List<T>` Java's list (read back into one), an enum its number; a
//! record of what it keeps as it is, as it is.

use yog_script_analyze::Type;
use yog_script_analyze::ast::{Decl, DeclKind, Modifier, Type as Written, TypeKind};
use yog_script_ids::java::{IF, LONG, NEW, NULL, OVERRIDE, PUBLIC, RECORD, VOID};
use yog_script_ids::system::LIST;
use yog_script_ids::jvm::state::{
    ACCEPT, ADD_ALL, APPLY, BYTES, ENUM_OF, ENUM_VALUE, FORGET, FROM_BYTES, JAVA_LIST, LOAD, OF_NULLABLE, OPTIONAL, OR_ELSE, READ,
    READ_TYPE, RECORD_PREFIX, SAVE, VALUE, WRITE, WRITE_TYPE,
};

use super::Gen;
use crate::index::Kind;

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
        let mut load = Vec::new();
        let mut save = Vec::new();
        for (ty, name) in saved_members(body) {
            let Some(kept) = self.codec_form(ty) else {
                return self.not_yet("saving a value of this type", ty.span);
            };
            let (field, record) = (Self::ident(name), format!("{RECORD_PREFIX}{name}"));
            self.code.line(format!("{PUBLIC} {RECORD} {record}({} {VALUE}) {{}}", kept.java));
            let key = self.code.str(&format!("{owner}.{name}"));
            let bytes = self.temp();
            let read = format!("{FROM_BYTES}({bytes}, {record}.class).{VALUE}()");
            load.push(format!(
                "byte[] {bytes} = {READ}.{APPLY}({key}); {IF} ({bytes} != {NULL}) {{ {} }}",
                (kept.load)(&field, &read)
            ));
            let value = (kept.save)(&field);
            save.push(format!("{WRITE}.{ACCEPT}({key}, {BYTES}({NEW} {record}({value})));"));
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

    /// How a member of `ty` goes through the bus's codec, if it does yet.
    fn codec_form(&self, ty: &Written) -> Option<Kept> {
        match &ty.kind {
            TypeKind::Nullable(inner) => {
                let java = self.written(inner, true);
                self.plain(inner).then(|| Kept {
                    java: format!("{OPTIONAL}<{java}>"),
                    save: Box::new(|f| format!("{OPTIONAL}.{OF_NULLABLE}({f})")),
                    load: Box::new(|f, v| format!("{f} = {v}.{OR_ELSE}({NULL});")),
                })
            }
            TypeKind::Path { path, args } if args.len() == 1 && path.last().is_some_and(|n| n.text == LIST) => {
                let (item, made) = (self.written(&args[0], true), self.written(ty, false));
                self.plain(&args[0]).then(|| Kept {
                    java: format!("{JAVA_LIST}<{item}>"),
                    save: Box::new(|f| f.to_string()),
                    load: Box::new(move |f, v| format!("{f} = {NEW} {made}(); {f}.{ADD_ALL}({v});")),
                })
            }
            _ if self.enum_of(ty).is_some() => {
                let class = self.written(ty, false);
                Some(Kept {
                    java: LONG.to_string(),
                    save: Box::new(|f| format!("{f}.{ENUM_VALUE}()")),
                    load: Box::new(move |f, v| format!("{f} = {class}.{ENUM_OF}({v});")),
                })
            }
            _ => self.plain(ty).then(|| Kept {
                java: self.written(ty, false),
                save: Box::new(|f| f.to_string()),
                load: Box::new(|f, v| format!("{f} = {v};")),
            }),
        }
    }

    /// Own enum `ty` names, when it's one whose cases carry no data.
    fn enum_of(&self, ty: &Written) -> Option<String> {
        let TypeKind::Path { path, args } = &ty.kind else {
            return None;
        };
        let name = path.last()?.text.clone();
        let data = self.index.cases.iter().any(|((e, _), fields)| *e == name && !fields.is_empty());
        (args.is_empty() && self.index.kind(&(name.clone(), 0)) == Some(Kind::Enum) && !data).then_some(name)
    }

    /// Whether the bus's codec keeps a value of `ty` as it is: a number,
    /// `bool`, `string`, a quantity, an id, a handle, a record of those.
    fn plain(&self, ty: &Written) -> bool {
        let TypeKind::Path { path, args } = &ty.kind else {
            return false;
        };
        let Some(name) = path.last().map(|n| n.text.as_str()).filter(|_| args.is_empty()) else {
            return false;
        };
        if self.index.kind(&(name.to_string(), 0)) == Some(Kind::Record) {
            return self.index.record_types(name).iter().all(|t| self.plain(t));
        }
        let t = yog_script_analyze::builtin_type(name)
            .or_else(|| yog_script_analyze::system_type(name))
            .or_else(|| self.index.is_unit(name).then(|| Type::Unit(name.to_string())))
            .or_else(|| yog_script_analyze::yog::is_handle(name).then(|| Type::Yog(name.to_string())));
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
                    | Type::Yog(_)
            )
        )
    }
}

/// How a member's value goes through the codec: its record's component's
/// Java type, what's written of the field, what reads it back.
struct Kept {
    java: String,
    save: Box<SaveOf>,
    load: Box<LoadInto>,
}

/// What's written of a field (`f` → the value packed).
type SaveOf = dyn Fn(&str) -> String;
/// What reads a field back (`f`, the value read → the statement).
type LoadInto = dyn Fn(&str, &str) -> String;
