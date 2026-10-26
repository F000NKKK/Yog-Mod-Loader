//! What a source does when the mods load, in its `$Load` — made then as a
//! mod is (`YsMod`), so a source without a `mod` does it too: its content
//! registered (definitions `yog build` made, `Content.Register`), its
//! `[Provide("…")]` methods answering the bus (their request a record of
//! their parameters, as a `[Service]`'s).

use yog_script_analyze::ast::{Decl, Method};
use yog_script_ids::java::{
    BYTE, CLASS, CLASS_LITERAL, FINAL, IMPLEMENTS, NEW, NULL, PUBLIC, RANK, RECORD, RETURN, STATIC,
};
use yog_script_ids::jvm::SERVICE_CALL;
use yog_script_ids::jvm::{BUS_PROVIDE, LOAD, REQUEST};
use yog_script_ids::system::PROVIDE;

use crate::{ContentDef, Gen};

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

    /// Content `defs` registered when the mods load.
    pub(crate) fn registered(&mut self, defs: &[ContentDef]) {
        let Some(record) = yog_script_analyze::yog::content_register() else {
            return;
        };
        for d in defs {
            let bytes: Vec<String> = d.data.iter().map(|b| (*b as i8).to_string()).collect();
            self.on_load.push(format!(
                "{NEW} {record}({}, {}, {NEW} {BYTE}{RANK} {{ {} }}).{SERVICE_CALL}();",
                self.code.str(&d.kind),
                self.code.str(&d.id),
                bytes.join(", ")
            ));
        }
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
