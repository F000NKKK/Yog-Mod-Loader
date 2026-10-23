//! Yog Script compiled to Java: a checked source written as one Java class
//! — its methods static, its types nested — on `java-common`'s runtime
//! (`dev.yog.script`) and bus (`dev.yog.api`), for `javac` to make the
//! bytecode the hosts load.
//!
//! What a source has that isn't written yet is reported (`YS0032`), never
//! written wrong.

mod calls;
mod classes;
mod decls;
mod exprs;
mod index;
mod stmts;
mod switch;
mod types;

use yog_codegen::Code;
use yog_codegen_java::Java;
use yog_data::Diagnostic;
use yog_ids::script_problem;
use yog_script_analyze::token::Span;
use yog_script_analyze::{Checked, Types};

/// A source's Java: its class's simple name and its text.
pub struct JavaClass {
    pub name: String,
    pub source: String,
}

/// The checked source `src` of file `name` as a Java class in `package`.
pub fn compile(
    name: &str,
    src: &str,
    checked: &Checked,
    package: &str,
) -> Result<JavaClass, Vec<Diagnostic>> {
    let class = class_name(name);
    let mut g = Gen {
        code: Code::new(),
        types: &checked.types,
        index: index::Index::collect(&checked.file.decls),
        report: yog_analysis::Report::new(name, src),
        file: file_name(name),
        temps: 0,
    };
    g.file(package, &class, &checked.file.decls);
    if g.report.out.is_empty() {
        Ok(JavaClass {
            name: class,
            source: g.code.finish(),
        })
    } else {
        Err(g.report.out)
    }
}

/// The class a source's file becomes: its stem, PascalCase, and `Script`.
pub fn class_name(file: &str) -> String {
    let stem = std::path::Path::new(file)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    format!("{}{CLASS_SUFFIX}", yog_codegen::pascal(stem))
}

/// What a source's class name ends with: never its mod's or a type's name.
const CLASS_SUFFIX: &str = "Script";

fn file_name(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_string()
}

pub(crate) struct Gen<'c> {
    code: Code<Java>,
    types: &'c Types,
    index: index::Index,
    report: yog_analysis::Report<'c>,
    /// The source's file name, as a panic says where.
    file: String,
    /// Temporaries made so far (`$t1`, …).
    temps: usize,
}

impl Gen<'_> {
    /// `what` isn't written as Java yet: reported at `at`.
    fn not_yet(&mut self, what: &str, at: Span) {
        self.report.error(
            script_problem::NOT_COMPILED,
            at,
            format!("{what} isn't compiled to Java yet"),
        );
    }

    /// A fresh temporary's name.
    fn temp(&mut self) -> String {
        self.temps += 1;
        format!("{TEMP}{}", self.temps)
    }

    /// Where `at` is, as a Java string: `"file.ys:12"`.
    fn at(&self, at: Span) -> String {
        self.code
            .str(&format!("{}:{}", self.file, self.report.line_of(at)))
    }

    /// `name` as a Java identifier.
    fn ident(name: &str) -> String {
        <Java as yog_codegen::Identifiers>::ident(name)
    }
}

/// What a temporary's name starts with: a `$`, which Yog Script names
/// never have.
const TEMP: &str = "$t";
