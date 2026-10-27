//! What a source does when the mods load, in its `$Load` — made then as a
//! mod is (`YsMod`), so a source without a `mod` does it too: its content's
//! objects bound to the content its files register (the generated Rust
//! does), its `[Provide("…")]` methods answering the bus (their request a
//! record of their parameters, as a `[Service]`'s).

use yog_script_analyze::ast::{Decl, Method};
use yog_script_ids::java::METHOD_REF;
use yog_script_ids::java::{
    CLASS, CLASS_LITERAL, FINAL, IMPLEMENTS, NEW, NULL, PUBLIC, RECORD, RETURN, STATIC,
};
use yog_script_ids::jvm::{BUS_PROVIDE, LOAD, OBJECTS_REGISTER, REQUEST};
use yog_script_ids::system::PROVIDE;

use crate::Gen;

impl Gen<'_> {
    /// Method `m` (Java's `name`), when it's `[Provide("…")]`: its request
    /// record written, its answering kept for `$Provided`.
    pub(crate) fn provided(&mut self, d: &Decl, m: &Method, name: &str) {
        let Some(address) = crate::index::attribute_text(d, PROVIDE) else {
            return;
        };
        if !self.statik {
            return self.not_yet("a `[Provide]` method that isn't `static`", m.name.span);
        }
        let request = format!("{}{REQUEST}", m.name.text);
        let fields = self.params(&m.params);
        self.code
            .line(format!("{PUBLIC} {RECORD} {request}({fields}) {{}}"));
        let owner = match &self.owner {
            Some((o, arity)) => self.index.place(o, *arity),
            None => self.class_name.clone(),
        };
        let q = self.temp();
        let args: Vec<String> = m
            .params
            .iter()
            .map(|p| format!("{q}.{}()", Self::ident(&p.name.text)))
            .collect();
        let call = format!("{owner}.{name}({})", args.join(", "));
        let answer = if m.ret.is_some() {
            format!("{q} -> {call}")
        } else {
            format!("{q} -> {{ {call}; {RETURN} {NULL}; }}")
        };
        self.on_load.push(format!(
            "{BUS_PROVIDE}({}, {owner}.{request}{CLASS_LITERAL}, {answer});",
            self.code.str(&address)
        ));
    }

    /// The file's `$Load`, when it has something to do when the mods load.
    pub(crate) fn load_class(&mut self) -> bool {
        if self.on_load.is_empty() {
            return false;
        }
        self.code.open(format!(
            "{PUBLIC} {STATIC} {FINAL} {CLASS} {LOAD} {IMPLEMENTS} {}",
            crate::MOD_INTERFACE
        ));
        self.code.open(format!("{PUBLIC} {LOAD}()"));
        for line in std::mem::take(&mut self.on_load) {
            self.code.line(line);
        }
        self.code.close("");
        self.code.close("");
        true
    }
}

impl Gen<'_> {
    /// Content `name` of kind `kind` with members: a class of its kind's
    /// object (`BlockObject`), its objects made by it for the content of
    /// its id (`Lamp`: `mymod:lamp`, its `lamp.yblock`'s).
    pub(crate) fn content_class(
        &mut self,
        d: &Decl,
        (kind, name): (&str, &str),
        header: &yog_script_analyze::ast::TypeHeader,
        body: &[Decl],
    ) {
        let Some(base) = yog_script_analyze::content_base(kind) else {
            return self.not_yet(
                "members of content of this kind",
                crate::decls::decl_span(d),
            );
        };
        let mut header = header.clone();
        header.bases.insert(0, written_named(base));
        // Its `saved` members: a block's are its chunk's, an entity's its
        // own store's, an item's its stack's (their objects' makers read
        // them in).
        if !crate::saved::saved_members(body).is_empty() {
            self.saved_owner = Some(name.to_string());
        }
        self.type_decl(d, (yog_script_ids::java::CLASS, name), &header, body);
        let Some(mod_id) = &self.mod_id else {
            return;
        };
        let id = yog_ids::qualify_in(mod_id, &yog_script_analyze::content_path(name));
        let line = format!(
            "{}.{OBJECTS_REGISTER}({}, {name}{METHOD_REF}{NEW});",
            self.index.place(base, 0),
            self.code.str(&id)
        );
        self.on_load.push(line);
    }
}

/// Type `name` as written (no arguments), where no source wrote it.
fn written_named(name: &str) -> yog_script_analyze::ast::Type {
    use yog_script_analyze::ast::{Name, Type, TypeKind};
    Type {
        kind: TypeKind::Path {
            path: vec![Name {
                text: name.to_string(),
                span: Default::default(),
            }],
            args: Vec::new(),
        },
        span: Default::default(),
    }
}
