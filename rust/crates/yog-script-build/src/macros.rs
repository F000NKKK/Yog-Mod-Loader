//! The procedural macros a build has: Yog Script sources whose methods are
//! marked `[ProcMacro]` (`Name!(…)`) or `[DeriveMacro]` (`[Derive(Name)]`),
//! and attribute classes marked `[Macro]` (`[Name(…)]`, their `Expand`),
//! checked first, then run by the interpreter when a source calls them.

use std::collections::HashSet;

use yog_data::Diagnostic;
use yog_script_analyze::ast::{Decl, DeclKind, File, TypeKind};
use yog_script_analyze::macros::ProcMacros;
use yog_script_analyze::system;
use yog_script_ids::system::{DERIVE_MACRO, FIELD, ITEM, MACRO, PROC_MACRO};
use yog_script_run::{Interp, Panic, Program, Value};

/// A build's macros, loaded from their sources.
pub struct MacroLibrary {
    files: Vec<File>,
    calls: HashSet<String>,
    derives: HashSet<String>,
    /// Attribute classes marked `[Macro]`: their `Expand` runs.
    attributes: HashSet<String>,
}

impl MacroLibrary {
    /// The macros of `sources` (name, text), or what's wrong with them.
    pub fn load(sources: &[(String, String)]) -> Result<Self, Vec<Diagnostic>> {
        let mut lib = MacroLibrary {
            files: Vec::new(),
            calls: HashSet::new(),
            derives: HashSet::new(),
            attributes: HashSet::new(),
        };
        let mut problems = Vec::new();
        for (name, text) in sources {
            let checked = yog_script_analyze::check(name, text);
            problems.extend(checked.problems.into_iter().filter(Diagnostic::is_error));
            lib.files.push(checked.file);
        }
        if !problems.is_empty() {
            return Err(problems);
        }
        for d in lib.files.iter().flat_map(|f| &f.decls) {
            match &d.kind {
                DeclKind::Method(m) if marked(d, PROC_MACRO) => {
                    lib.calls.insert(m.name.text.clone());
                }
                DeclKind::Method(m) if marked(d, DERIVE_MACRO) => {
                    lib.derives.insert(m.name.text.clone());
                }
                DeclKind::Type { name, .. } if marked(d, MACRO) => {
                    lib.attributes.insert(name.text.clone());
                }
                _ => {}
            }
        }
        Ok(lib)
    }

    /// `f` run with an interpreter over the library and the System's.
    fn run<R>(&self, f: impl FnOnce(&mut Interp<'_>) -> R) -> R {
        let mut sources: Vec<&[Decl]> = vec![&system::DECLS];
        sources.extend(self.files.iter().map(|f| f.decls.as_slice()));
        let program = Program::of(&sources);
        f(&mut Interp::new(&program))
    }
}

impl ProcMacros for MacroLibrary {
    fn call(&self, name: &str, input: &str) -> Option<Result<String, String>> {
        self.calls
            .contains(name)
            .then(|| self.run(|i| code(i.call(name, vec![Value::str(input)]))))
    }

    fn derive(&self, name: &str, item: &str) -> Option<Result<String, String>> {
        if !self.derives.contains(name) {
            return None;
        }
        Some(self.run(|i| {
            let item = item_value(i, item)?;
            code(i.call(name, vec![item]))
        }))
    }

    fn attribute(&self, name: &str, args: &str, item: &str) -> Option<Result<String, String>> {
        if !self.attributes.contains(name) {
            return None;
        }
        Some(self.run(|i| {
            let item = item_value(i, item)?;
            code(i.call_static(name, EXPAND, vec![Value::str(args), item]))
        }))
    }
}

/// What an attribute macro's class runs: `static string Expand(string
/// args, Item item)`.
const EXPAND: &str = "Expand";

/// Whether `d` carries attribute `name`.
fn marked(d: &Decl, name: &str) -> bool {
    d.attributes
        .iter()
        .any(|a| a.path.last().is_some_and(|n| n.text == name))
}

/// A macro's result as code.
fn code(out: Result<Value, Panic>) -> Result<String, String> {
    match out {
        Ok(Value::Str(s)) => Ok(s.to_string()),
        Ok(other) => Err(format!("a macro gives its code as a string, not `{other}`")),
        Err(p) => Err(p.message),
    }
}

/// The `System.Meta.Item` a derive macro is given for the declaration
/// written `text`.
fn item_value(i: &mut Interp<'_>, text: &str) -> Result<Value, String> {
    let parsed = yog_script_analyze::parse("item.ys", text);
    let Some((name, kind, fields)) = parsed.file.decls.iter().find_map(|d| shape(d, text)) else {
        return Err("a derive macro is for a type".to_string());
    };
    let mut made = Vec::new();
    for (n, t) in fields {
        made.push(
            i.make_object(FIELD, vec![Value::str(&n), Value::str(&t)])
                .map_err(|p| p.message)?,
        );
    }
    let args = vec![
        Value::str(&name),
        Value::str(&kind),
        Value::list(made),
        Value::str(text),
    ];
    i.make_object(ITEM, args).map_err(|p| p.message)
}

/// A type's name, its word, its fields (name, type as written).
type Shape = (String, String, Vec<(String, String)>);

/// A type's name, word and fields (name, type as written).
fn shape(d: &Decl, text: &str) -> Option<Shape> {
    let written = |t: &yog_script_analyze::ast::Type| {
        text.get(t.span.start as usize..t.span.end as usize)
            .unwrap_or_default()
            .to_string()
    };
    match &d.kind {
        DeclKind::Type {
            word,
            name,
            header,
            body,
            ..
        } => {
            let mut fields: Vec<(String, String)> = header
                .ctor
                .iter()
                .flatten()
                .map(|p| (p.name.text.clone(), written(&p.ty)))
                .collect();
            for m in body {
                match &m.kind {
                    DeclKind::Field(f) => fields.push((f.name.text.clone(), written(&f.ty))),
                    DeclKind::Property { ty, name, .. }
                        if !matches!(ty.kind, TypeKind::Lifetime(_)) =>
                    {
                        fields.push((name.text.clone(), written(ty)));
                    }
                    _ => {}
                }
            }
            Some((name.text.clone(), word.text.clone(), fields))
        }
        DeclKind::Enum { name, .. } => Some((
            name.text.clone(),
            yog_script_analyze::words::r#enum::WORD.text.to_string(),
            Vec::new(),
        )),
        _ => None,
    }
}
