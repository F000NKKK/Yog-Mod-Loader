//! `saved` members on the JVM (docs/design/storage.md): an object keeping
//! some implements `State.Saved` — each member a record of its own
//! (`$Visits(int value)`, Yog Data by the bus's codec) keyed
//! `Owner.Member` in its owner's store; `$load` reads them in as the object
//! is made live, `$save` writes them out as the world saves. What the codec
//! doesn't keep as it is goes as what it does: a `T?` an `Optional<T>`, a
//! `List<T>` Java's list (read back into one), a `Dictionary<K, V>` a list
//! of its entries, an enum its number, a `Text` its own Yog Data; a
//! record of what it keeps as it is, as it is.

use yog_script_analyze::Type;
use yog_script_analyze::ast::{Decl, DeclKind, Modifier, Type as Written, TypeKind};
use yog_script_ids::java::{IF, LONG, NEW, NULL, OVERRIDE, PUBLIC, RECORD, RETURN, VOID};
use yog_script_ids::jvm::STRING;
use yog_script_ids::jvm::state::{
    ACCEPT, ADD_ALL, BYTES, DROP_RECORD, ENTRIES, ENTRY_KEY, ENTRY_SUFFIX, ENTRY_VALUE, ENTRY_VAR,
    ENUM_OF, ENUM_VALUE, FORGET, FROM_BYTES, GET_KEY, GET_VALUE, JAVA_LIST, LOAD, MAP, OF_NULLABLE,
    OPTIONAL, OR_ELSE, READ, READ_RECORD, READ_TYPE, RECORD_PREFIX, SAVE, SET_ITEM,
    TEXT_BYTES_TYPE, TEXT_FROM_BYTES, TEXT_TO_BYTES, TO_LIST, VALUE, WAS_SUFFIX, WRITE, WRITE_TYPE,
};
use yog_script_ids::jvm::synced::{KEY, OUT, SYNC, SYNCED, SYNCED_BYTES};
use yog_script_ids::system::{DICTIONARY, LIST, TEXT_TYPE};

use super::Gen;
use crate::index::Kind;

/// A stored member: its type as written, its name, and what an old save
/// called it (`was Power: i32`: the old name, its type when it changed).
pub(crate) struct Stored<'a> {
    pub(crate) ty: &'a Written,
    pub(crate) name: &'a str,
    pub(crate) was: Option<(&'a str, Option<&'a Written>)>,
}

/// The `saved` members of `body`.
pub(crate) fn saved_members(body: &[Decl]) -> Vec<Stored<'_>> {
    members_with(body, Modifier::Saved)
}

/// The `synced` members of `body`.
pub(crate) fn synced_members(body: &[Decl]) -> Vec<Stored<'_>> {
    members_with(body, Modifier::Synced)
}

/// The stored members of `body` marked `modifier`.
fn members_with(body: &[Decl], modifier: Modifier) -> Vec<Stored<'_>> {
    body.iter()
        .filter(|d| d.has(modifier))
        .filter_map(|d| match &d.kind {
            DeclKind::Field(f) => Some(Stored {
                ty: &f.ty,
                name: &f.name.text,
                was: f
                    .was
                    .as_ref()
                    .map(|(old, ty)| (old.text.as_str(), ty.as_ref())),
            }),
            DeclKind::Property { ty, name, .. } if yog_script_analyze::ast::stores_value(d) => {
                Some(Stored {
                    ty,
                    name: &name.text,
                    was: None,
                })
            }
            _ => None,
        })
        .collect()
}

impl Gen<'_> {
    /// The members of `body`'s declared state, keyed `owner.Member`: each
    /// `saved` or `synced` one a record; `State.Saved`'s `$load`/`$save`
    /// when some are saved, `SyncedState.Synced`'s `$sync`/`$synced` when
    /// some are synced; `$forget` doing `forget`. An item's (`on_stack`)
    /// `synced` members are its stack's like its `saved` ones: the game
    /// sends the stack to its clients.
    pub(crate) fn saved_state(
        &mut self,
        owner: &str,
        body: &[Decl],
        (forget, on_stack): (&str, bool),
    ) {
        let (mut saved, mut synced) = (saved_members(body), synced_members(body));
        if on_stack {
            saved.append(&mut synced);
        }
        let (mut load, mut save, mut sync, mut take) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        let mut done: Vec<&str> = Vec::new();
        for member in saved.iter().chain(&synced) {
            let (ty, name) = (member.ty, member.name);
            if done.contains(&name) {
                continue;
            }
            done.push(name);
            let Some(kept) = self.codec_form(ty, name) else {
                return self.not_yet("keeping a value of this type", ty.span);
            };
            let (field, record) = (Self::ident(name), format!("{RECORD_PREFIX}{name}"));
            for line in &kept.declared {
                self.code.line(line);
            }
            self.code.line(format!(
                "{PUBLIC} {RECORD} {record}({} {VALUE}) {{}}",
                kept.java
            ));
            let key = self.code.str(&format!("{owner}.{name}"));
            let read = |bytes: &str| format!("{FROM_BYTES}({bytes}, {record}.class).{VALUE}()");
            let written = format!("{BYTES}({NEW} {record}({}))", (kept.save)(&field));
            if saved.iter().any(|m| m.name == name) {
                let bytes = self.temp();
                let old = member
                    .was
                    .map_or_else(String::new, |was| self.was_read(owner, member, &kept, was));
                load.push(format!(
                    "byte[] {bytes} = {READ}.{READ_RECORD}({key}); {IF} ({bytes} != {NULL}) {{ {} }}{old}",
                    (kept.load)(&field, &read(&bytes))
                ));
                save.push(format!("{WRITE}.{ACCEPT}({key}, {written});"));
            }
            if synced.iter().any(|m| m.name == name) {
                sync.push(format!("{OUT}.{ACCEPT}({key}, {written});"));
                take.push(format!(
                    "{IF} ({key}.equals({KEY})) {{ {} {RETURN}; }}",
                    (kept.load)(&field, &read(SYNCED_BYTES))
                ));
            }
        }
        if !saved.is_empty() {
            self.state_method(LOAD, &format!("{READ_TYPE} {READ}"), &load);
            self.state_method(SAVE, &format!("{WRITE_TYPE} {WRITE}"), &save);
        }
        if !synced.is_empty() {
            self.state_method(SYNC, &format!("{WRITE_TYPE} {OUT}"), &sync);
            self.state_method(
                SYNCED,
                &format!("{STRING} {KEY}, byte[] {SYNCED_BYTES}"),
                &take,
            );
        }
        self.state_method(FORGET, "", &[forget.to_string()]);
    }

    /// What reads `member` from what an old save called it (`was`), when
    /// its own record is missing: the old key's bytes as the old type's
    /// record (`$Energy$was`, the member's own when the type kept), cast
    /// to the type now, the old record dropped. `else { … }` after the
    /// member's own read.
    fn was_read(
        &mut self,
        owner: &str,
        member: &Stored<'_>,
        kept: &Kept,
        (old, old_ty): (&str, Option<&Written>),
    ) -> String {
        let (field, bytes) = (Self::ident(member.name), self.temp());
        let record = match old_ty {
            Some(ty) => {
                let record = format!("{RECORD_PREFIX}{}{WAS_SUFFIX}", member.name);
                self.code.line(format!(
                    "{PUBLIC} {RECORD} {record}({} {VALUE}) {{}}",
                    self.written(ty, false)
                ));
                record
            }
            None => format!("{RECORD_PREFIX}{}", member.name),
        };
        let value = format!("{FROM_BYTES}({bytes}, {record}.class).{VALUE}()");
        let value = match old_ty {
            Some(_) => format!("(({}) {value})", kept.java),
            None => value,
        };
        let key = self.code.str(&format!("{owner}.{old}"));
        format!(
            " else {{ byte[] {bytes} = {READ}.{READ_RECORD}({key}); {IF} ({bytes} != {NULL}) {{ {} {READ}.{DROP_RECORD}({key}); }} }}",
            (kept.load)(&field, &value)
        )
    }

    /// `@Override public void name(params) { lines }`.
    pub(crate) fn state_method(&mut self, name: &str, params: &str, lines: &[String]) {
        self.code.line(OVERRIDE);
        self.code.open(format!("{PUBLIC} {VOID} {name}({params})"));
        for l in lines.iter().filter(|l| !l.is_empty()) {
            self.code.line(l);
        }
        self.code.close("");
    }

    /// How a member of `ty` goes through the bus's codec, if it does yet.
    pub(crate) fn codec_form(&self, ty: &Written, name: &str) -> Option<Kept> {
        match &ty.kind {
            TypeKind::Nullable(inner) => {
                let java = self.written(inner, true);
                self.plain(inner).then(|| Kept {
                    declared: Vec::new(),
                    java: format!("{OPTIONAL}<{java}>"),
                    save: Box::new(|f| format!("{OPTIONAL}.{OF_NULLABLE}({f})")),
                    load: Box::new(|f, v| format!("{f} = {v}.{OR_ELSE}({NULL});")),
                })
            }
            TypeKind::Path { path, args }
                if args.len() == 1 && path.last().is_some_and(|n| n.text == LIST) =>
            {
                let (item, made) = (self.written(&args[0], true), self.written(ty, false));
                self.plain(&args[0]).then(|| Kept {
                    declared: Vec::new(),
                    java: format!("{JAVA_LIST}<{item}>"),
                    save: Box::new(|f| f.to_string()),
                    load: Box::new(move |f, v| {
                        format!("{f} = {NEW} {made}(); {f}.{ADD_ALL}({v});")
                    }),
                })
            }
            TypeKind::Path { path, args }
                if args.len() == 2 && path.last().is_some_and(|n| n.text == DICTIONARY) =>
            {
                self.dictionary_form(ty, (&args[0], &args[1]), name)
            }
            TypeKind::Path { path, args }
                if args.is_empty() && path.last().is_some_and(|n| n.text == TEXT_TYPE) =>
            {
                let text = self.written(ty, false);
                Some(Kept {
                    declared: Vec::new(),
                    java: TEXT_BYTES_TYPE.to_string(),
                    save: Box::new(|f| format!("{f}.{TEXT_TO_BYTES}()")),
                    load: Box::new(move |f, v| format!("{f} = {text}.{TEXT_FROM_BYTES}({v});")),
                })
            }
            _ if let Some(e) = self.data_enum_of(ty) => self.data_enum_form(ty, &e, name),
            _ if self.enum_of(ty).is_some() => {
                let class = self.written(ty, false);
                Some(Kept {
                    declared: Vec::new(),
                    java: LONG.to_string(),
                    save: Box::new(|f| format!("{f}.{ENUM_VALUE}()")),
                    load: Box::new(move |f, v| format!("{f} = {class}.{ENUM_OF}({v});")),
                })
            }
            _ => self.plain(ty).then(|| Kept {
                declared: Vec::new(),
                java: self.written(ty, false),
                save: Box::new(|f| f.to_string()),
                load: Box::new(|f, v| format!("{f} = {v};")),
            }),
        }
    }

    /// A `Dictionary<K, V>` (`ty`, member `name`'s) of what the codec keeps
    /// as it is: a list of its entries, each a record of its own.
    fn dictionary_form(
        &self,
        ty: &Written,
        (k, v): (&Written, &Written),
        name: &str,
    ) -> Option<Kept> {
        if !(self.plain(k) && self.plain(v)) {
            return None;
        }
        let entry = format!("{RECORD_PREFIX}{name}{ENTRY_SUFFIX}");
        let (kj, vj, made) = (
            self.written(k, true),
            self.written(v, true),
            self.written(ty, false),
        );
        let declared =
            format!("{PUBLIC} {RECORD} {entry}({kj} {ENTRY_KEY}, {vj} {ENTRY_VALUE}) {{}}");
        let saved_as = entry.clone();
        Some(Kept {
            declared: vec![declared],
            java: format!("{JAVA_LIST}<{entry}>"),
            save: Box::new(move |f| {
                format!(
                    "{f}.{ENTRIES}.{MAP}({ENTRY_VAR} -> {NEW} {saved_as}({ENTRY_VAR}.{GET_KEY}(), {ENTRY_VAR}.{GET_VALUE}())).{TO_LIST}()"
                )
            }),
            load: Box::new(move |f, v| {
                format!(
                    "{f} = {NEW} {made}(); for (var {ENTRY_VAR} : {v}) {{ {f}.{SET_ITEM}({ENTRY_VAR}.{ENTRY_KEY}(), {ENTRY_VAR}.{ENTRY_VALUE}()); }}"
                )
            }),
        })
    }

    /// Own enum `ty` names, when it's one whose cases carry no data.
    fn enum_of(&self, ty: &Written) -> Option<String> {
        let TypeKind::Path { path, args } = &ty.kind else {
            return None;
        };
        let name = path.last()?.text.clone();
        let data = self
            .index
            .cases
            .iter()
            .any(|((e, _), fields)| *e == name && !fields.is_empty());
        (args.is_empty() && self.index.kind(&(name.clone(), 0)) == Some(Kind::Enum) && !data)
            .then_some(name)
    }

    /// Whether the bus's codec keeps a value of `ty` as it is: a number,
    /// `bool`, `string`, a quantity, an id, a handle, a record of those.
    pub(crate) fn plain(&self, ty: &Written) -> bool {
        let TypeKind::Path { path, args } = &ty.kind else {
            return false;
        };
        let Some(name) = path
            .last()
            .map(|n| n.text.as_str())
            .filter(|_| args.is_empty())
        else {
            return false;
        };
        if self.index.kind(&(name.to_string(), 0)) == Some(Kind::Record) {
            return self.index.record_types(name).iter().all(|t| self.plain(t));
        }
        let t = yog_script_analyze::builtin_type(name)
            .or_else(|| yog_script_analyze::system_type(name))
            .or_else(|| {
                self.index
                    .is_unit(name)
                    .then(|| Type::Unit(name.to_string()))
            })
            .or_else(|| {
                yog_script_analyze::yog::is_handle(name).then(|| Type::Yog(name.to_string()))
            });
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
pub(crate) struct Kept {
    /// What its value's parts need declared first (a dictionary's entry, an
    /// enum's cases' records and what writes and reads them).
    pub(crate) declared: Vec<String>,
    pub(crate) java: String,
    pub(crate) save: Box<SaveOf>,
    pub(crate) load: Box<LoadInto>,
}

/// What's written of a field (`f` → the value packed).
pub(crate) type SaveOf = dyn Fn(&str) -> String;
/// What reads a field back (`f`, the value read → the statement).
pub(crate) type LoadInto = dyn Fn(&str, &str) -> String;
