//! A `command "charge" { … }`: a class of its methods (static: a command
//! has no object), and for each a way of running it per how many arguments
//! it's given (those left out their defaults) — a wrapper reading the
//! arguments the game parsed, refusing what it `requires` doesn't let,
//! calling it, its `string` result what's replied — declared when the mods
//! load (`Commands.declare`).

use yog_script_analyze::Type;
use yog_script_analyze::ast::{Decl, DeclKind, Method};
use yog_script_analyze::{builtin_type, command_arg};
use yog_script_ids::java::{
    BOOLEAN, CLASS, FALSE, FINAL, IF, NEW, PRIVATE, PUBLIC, RETURN, STATIC, TRUE,
};
use yog_script_ids::jvm::{
    COMMAND_SUFFIX, COMMANDS_DECLARE, LIST, PARSE_DOUBLE, PARSE_FLOAT, PARSE_INT, RUN_WAY, STRING,
};
use yog_script_ids::system::{SENDER, SENDER_REPLY};

use crate::Gen;
use crate::decls::Place;

impl Gen<'_> {
    /// Command `name`'s class and its ways of running, declared when the mods load.
    pub(crate) fn command_class(&mut self, name: &str, body: &[Decl]) {
        let class = format!("{}{COMMAND_SUFFIX}", yog_codegen::pascal(name));
        self.code
            .open(format!("{PUBLIC} {STATIC} {FINAL} {CLASS} {class}"));
        self.code.line(format!("{PRIVATE} {class}() {{}}"));
        let outer = std::mem::replace(&mut self.in_command, true);
        self.members(body, Place::File);
        self.in_command = outer;
        for d in body {
            if let DeclKind::Method(m) = &d.kind {
                self.command_ways(name, &class, m);
            }
        }
        self.code.close("");
    }

    /// Method `m`'s ways of running: one per how many arguments it's given.
    fn command_ways(&mut self, command: &str, class: &str, m: &Method) {
        let args = &m.params[1..];
        let required = args.iter().take_while(|p| p.default.is_none()).count();
        let types: Vec<Type> = args.iter().map(|p| param_type(&p.ty)).collect();
        for given in required..=args.len() {
            let way = format!("{RUN_WAY}{}{given}", Self::ident(&m.name.text));
            self.command_wrapper(m, (&way, given), &types);
            let schema: Vec<String> = types[..given]
                .iter()
                .map(|t| self.code.str(command_arg(t).unwrap_or_default()))
                .collect();
            let declared = format!(
                "{COMMANDS_DECLARE}({}, {}, {NEW} {STRING}[] {{ {} }}, {class}::{way});",
                self.code.str(command),
                self.code.str(&m.name.text.to_lowercase()),
                schema.join(", ")
            );
            self.on_load.push(declared);
        }
    }

    /// The wrapper running `m` with `given` arguments (`way`, of `types`):
    /// each read, the rest their defaults; what it requires checked.
    fn command_wrapper(&mut self, m: &Method, (way, given): (&str, usize), types: &[Type]) {
        let sender = &m.params[0];
        let list = self.temp();
        let sender_t = self.index.place(SENDER, 0);
        self.code.open(format!(
            "{STATIC} {BOOLEAN} {way}({sender_t} {}, {LIST}<{STRING}> {list})",
            Self::ident(&sender.name.text)
        ));
        for (i, (p, t)) in m.params[1..].iter().zip(types).enumerate() {
            let value = if i < given {
                parsed(t, &format!("{list}.get({i})"))
            } else {
                p.default
                    .as_ref()
                    .map_or_else(String::new, |d| self.expr(d))
            };
            self.code.line(format!(
                "{} {} = {value};",
                self.java(t, false),
                Self::ident(&p.name.text)
            ));
        }
        if let Some(r) = &m.requires {
            let r = self.expr(r);
            self.code.line(format!("{IF} (!({r})) {RETURN} {FALSE};"));
        }
        let call = format!(
            "{}({})",
            Self::ident(&m.name.text),
            m.params
                .iter()
                .map(|p| Self::ident(&p.name.text))
                .collect::<Vec<_>>()
                .join(", ")
        );
        match m.ret {
            Some(_) => self.code.line(format!(
                "{}.{SENDER_REPLY}({call});",
                Self::ident(&sender.name.text)
            )),
            None => self.code.line(format!("{call};")),
        };
        self.code.line(format!("{RETURN} {TRUE};"));
        self.code.close("");
    }
}

/// Argument `arg` (text, as the game parsed it) as a value of `t`.
fn parsed(t: &Type, arg: &str) -> String {
    match t {
        Type::I32 => format!("{PARSE_INT}({arg})"),
        Type::F32 => format!("{PARSE_FLOAT}({arg})"),
        Type::F64 => format!("{PARSE_DOUBLE}({arg})"),
        _ => arg.to_string(),
    }
}

/// A command's parameter's type as written, checked: a built-in one, else
/// Yog's handle (`Player`) — what a command takes.
fn param_type(written: &yog_script_analyze::ast::Type) -> Type {
    match &written.kind {
        yog_script_analyze::ast::TypeKind::Path { path, args } if args.is_empty() => {
            let name = path.last().map_or("", |n| n.text.as_str());
            builtin_type(name).unwrap_or_else(|| Type::Yog(name.to_string()))
        }
        _ => Type::Unknown,
    }
}
