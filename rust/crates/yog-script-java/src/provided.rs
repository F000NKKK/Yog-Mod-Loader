//! `[Provide("…")]` methods: the bus asks them at an address (Rust, another
//! mod). The request is a record of the method's parameters, as a
//! `[Service]`'s; the answering registered by the file's `$Provided`, made
//! when the mods load as a mod is (`YsMod`), so a source without a `mod`
//! answers too.

use yog_script_analyze::ast::{Decl, Method};
use yog_script_ids::java::{
    CLASS, CLASS_LITERAL, FINAL, IMPLEMENTS, NULL, PUBLIC, RECORD, RETURN, STATIC,
};
use yog_script_ids::jvm::{BUS_PROVIDE, PROVIDED, REQUEST};
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
        self.provided.push(format!(
            "{BUS_PROVIDE}({}, {owner}.{request}{CLASS_LITERAL}, {answer});",
            self.code.str(&address)
        ));
    }

    /// The file's `$Provided`, when it has `[Provide]` methods: made when
    /// the mods load, it registers each.
    pub(crate) fn provided_class(&mut self) -> bool {
        if self.provided.is_empty() {
            return false;
        }
        self.code.open(format!(
            "{PUBLIC} {STATIC} {FINAL} {CLASS} {PROVIDED} {IMPLEMENTS} {}",
            crate::MOD_INTERFACE
        ));
        self.code.open(format!("{PUBLIC} {PROVIDED}()"));
        for line in std::mem::take(&mut self.provided) {
            self.code.line(line);
        }
        self.code.close("");
        self.code.close("");
        true
    }
}
