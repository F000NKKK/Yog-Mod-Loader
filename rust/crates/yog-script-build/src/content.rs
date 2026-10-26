//! A source's content declarations made definitions: each its kind's
//! defaults (`yog-registry`'s), its properties — run by the interpreter —
//! over them, read back by its kind's schema. What's left is the bytes the
//! mod registers when it loads (`Content.Register`).

use yog_data::{Diagnostic, Value as Data};
use yog_ids::script_problem;
use yog_script_analyze::Checked;
use yog_script_analyze::ast::{Decl, DeclKind};
use yog_script_java::ContentDef;
use yog_script_run::{Interp, Program, Value};

/// The content source `name` (`src`, `checked`) declares, its ids in the
/// mod's namespace `mod_id`: each definition, or what's wrong with them.
pub fn content_defs(
    (name, src): (&str, &str),
    checked: &Checked,
    mod_id: &str,
) -> Result<Vec<ContentDef>, Vec<Diagnostic>> {
    let program = Program::of(&[&yog_script_analyze::system::DECLS, &checked.file.decls]);
    let mut interp = Interp::new(&program);
    let mut report = yog_analysis::Report::new(name, src);
    let mut out = Vec::new();
    for d in all(&checked.file.decls) {
        if let Some(def) = definition(d, mod_id, &mut interp, &mut report) {
            out.push(def);
        }
    }
    if report.out.iter().any(Diagnostic::is_error) {
        Err(report.out)
    } else {
        Ok(out)
    }
}

/// Every declaration, those inside namespaces too.
fn all(decls: &[Decl]) -> Vec<&Decl> {
    decls
        .iter()
        .flat_map(|d| match &d.kind {
            DeclKind::Namespace {
                body: Some(body), ..
            } => all(body),
            _ => vec![d],
        })
        .collect()
}

/// Content declaration `d`'s definition, when it's of a kind Yog Script
/// declares; what's wrong with it reported.
fn definition(
    d: &Decl,
    mod_id: &str,
    interp: &mut Interp<'_>,
    report: &mut yog_analysis::Report<'_>,
) -> Option<ContentDef> {
    let DeclKind::Content {
        kind, name, body, ..
    } = &d.kind
    else {
        return None;
    };
    let k = yog_registry::content_kind(&kind.text)?;
    let id = yog_ids::qualify_in(mod_id, &yog_codegen::snake(&name.text));
    let mut def = (k.new)(&id);
    for m in body {
        let DeclKind::Prop(p) = &m.kind else {
            continue;
        };
        let value = match interp.value_of(&p.value) {
            Ok(v) => v,
            Err(panic) => {
                report.error(script_problem::NOT_COMPILED, p.value.span, panic.message);
                continue;
            }
        };
        let Some(data) = data_of(&value) else {
            report.error(
                script_problem::TYPE_MISMATCH,
                p.value.span,
                format!("`{}` isn't a value a definition holds", p.name.text),
            );
            continue;
        };
        if let Data::Map(fields) = &mut def {
            fields.insert(yog_codegen::snake(&p.name.text), data);
        }
    }
    if let Err(problems) = (k.check)(&def) {
        for p in problems {
            report.error(
                script_problem::TYPE_MISMATCH,
                name.span,
                format!("`{}`: {}", name.text, p.message),
            );
        }
        return None;
    }
    Some(ContentDef {
        name: name.text.clone(),
        kind: k.kind.to_string(),
        id,
        data: yog_data::binary::encode(&def),
    })
}

/// A value the interpreter computed as Yog Data, when it's data.
fn data_of(v: &Value) -> Option<Data> {
    Some(match v {
        Value::Null => Data::Null,
        Value::Bool(b) => Data::Bool(*b),
        Value::Int(i) => Data::Int(*i),
        Value::Float(f) => Data::Float(*f),
        Value::Str(s) => Data::Str(s.to_string()),
        Value::Char(c) => Data::Str(c.to_string()),
        Value::List(items) => {
            Data::List(items.borrow().iter().map(data_of).collect::<Option<_>>()?)
        }
        _ => return None,
    })
}
