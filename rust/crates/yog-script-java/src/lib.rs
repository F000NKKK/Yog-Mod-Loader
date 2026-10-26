//! Yog Script compiled to Java: a checked source written as one Java class
//! — its methods static, its types nested — on `java-common`'s runtime
//! (`dev.yog.script`) and bus (`dev.yog.api`), for `javac` to make the
//! bytecode the hosts load.
//!
//! What a source has that isn't written yet is reported (`YS0032`), never
//! written wrong.

mod arrays;
mod calls;
mod captures;
mod classes;
mod collections;
mod commands;
mod decimals;
mod decls;
mod delegates;
mod drops;
mod enums;
mod events;
mod exprs;
mod groups;
mod identity;
mod index;
mod indexers;
mod java_names;
mod kept;
mod library;
mod load;
mod locks;
mod machine;
mod macros;
mod messages;
mod operators;
mod patterns;
mod raisers;
mod reflection;
mod refs;
mod reified;
mod safe_calls;
mod statics;
mod stmts;
mod switch;
mod trees;
mod tuples;
mod type_args;
mod types;
mod with;

use yog_codegen::Code;
use yog_codegen_java::Java;
use yog_data::Diagnostic;
use yog_ids::script_problem;
use yog_script_analyze::token::Span;
use yog_script_analyze::{Checked, Types};

pub use library::{JavaFile, library};

/// A source's Java: its class's simple name and its text.
pub struct JavaClass {
    pub name: String,
    pub source: String,
    /// Its mods' classes by their binary names (`pkg.GlassyScript$Glassy`):
    /// what the jar lists for `ServiceLoader`.
    pub mods: Vec<String>,
}

/// The checked source `src` of file `name` as a Java class in `package`.
pub fn compile(
    name: &str,
    src: &str,
    checked: &Checked,
    package: &str,
) -> Result<JavaClass, Vec<Diagnostic>> {
    let target = Target {
        package,
        mod_id: None,
        others: &[],
    };
    compile_in((name, src), checked, &target)
}

/// What a source is compiled for: its Java package, its mod (whose id
/// names its messages' channels and its content), the mod's other sources
/// (each its file's name and declarations: their types reached in their
/// classes).
pub struct Target<'a> {
    pub package: &'a str,
    pub mod_id: Option<&'a str>,
    pub others: &'a [(&'a str, &'a [yog_script_analyze::ast::Decl])],
}

/// [`compile`] for `target`: its content's objects bound to its mod's
/// content, its calls across sides messages on its mod's channels.
pub fn compile_in(
    (name, src): (&str, &str),
    checked: &Checked,
    target: &Target<'_>,
) -> Result<JavaClass, Vec<Diagnostic>> {
    let package = target.package;
    let class = class_name(name);
    let others: Vec<(String, &[yog_script_analyze::ast::Decl])> = target
        .others
        .iter()
        .map(|(file, decls)| (format!("{package}.{}", class_name(file)), *decls))
        .collect();
    let mut g = Gen::new(
        &checked.types,
        index::Index::collect_project(&checked.file.decls, &others),
        name,
        src,
    );
    g.mod_id = target.mod_id.map(str::to_string);
    g.file(package, &class, &checked.file.decls);
    if g.report.out.is_empty() {
        let mut mods: Vec<String> = mods(&checked.file.decls)
            .into_iter()
            .map(|m| format!("{package}.{class}${m}"))
            .collect();
        if g.has_load {
            // A nested class's binary name: its outer one's, `$`, its own.
            mods.push(format!(
                "{package}.{class}{}{}",
                yog_script_ids::jvm::NESTED,
                yog_script_ids::jvm::LOAD
            ));
        }
        Ok(JavaClass {
            name: class,
            source: g.code.finish(),
            mods,
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

/// The `mod`s a source declares, by name.
fn mods(decls: &[yog_script_analyze::ast::Decl]) -> Vec<String> {
    use yog_script_analyze::ast::DeclKind;
    let mut out = Vec::new();
    for d in decls {
        match &d.kind {
            DeclKind::Mod { name, .. } => out.push(name.text.clone()),
            kind => out.extend(mods(kind.members())),
        }
    }
    out
}

use yog_script_ids::jvm::CLASS_SUFFIX;
use yog_script_ids::jvm::MOD_INTERFACE;
use yog_script_ids::jvm::TEMP;

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
    /// Types are written nested in the source's class (a source's), or
    /// each as a file's own (the library's).
    nested: bool,
    /// The type whose members are being written, by name and arity.
    owner: Option<crate::java_names::TypeRef>,
    /// The type arguments objects are made with, each list a constant of
    /// the source's class (`$types1`): worked out once, not per object.
    kept_types: Vec<String>,
    /// The file's event raisers, written at its class's end (`raisers`).
    raisers: Vec<crate::raisers::Raiser>,
    /// The loops being written, innermost last: a loop written in steps
    /// what leaves its body for a `continue` (`break $t1;`, a split one's
    /// jump); Java's own none.
    continues: Vec<Option<String>>,
    /// The method's `ref` and `out` parameters: cells (`x.value`).
    cells: std::collections::HashSet<String>,
    /// What the call being written gives back to its `ref` and `out`
    /// arguments' places after it (`a = $t1.value`).
    given_back: Vec<String>,
    /// The call being written is the runtime's: its `ref` arguments passed
    /// as values, the value it gives the place's (`xs = Array.Resize(xs, n)`).
    runtime_call: bool,
    /// The type parameters in reach, innermost last: each name and its
    /// value (`$typeT`, `Type.argument(this, Box.class, 0)`).
    type_params: Vec<(String, String)>,
    /// The type being written's own type parameters, by name.
    owner_generics: Vec<String>,
    /// The `async` method being written's machine (none in a lambda of it).
    machine: Option<machine::Machine>,
    /// The method's names that are its machine's fields, innermost last.
    renamed: Vec<(String, String)>,
    /// Where `this` is when the code is the machine's: its method's class's.
    this_name: Option<String>,
    /// The loops being written, innermost last: a loop split in states
    /// the jump a `break` leaves it by; Java's own none.
    breaks: Vec<Option<String>>,
    /// The code being written has no object (a static member's, the file's).
    statik: bool,
    /// The source's class's simple name.
    class_name: String,
    /// The body being written's locals a lambda captures and something
    /// changes: each a cell (`captures.rs`).
    boxed: std::collections::HashSet<String>,
    /// What the file does when the mods load, for its `$Load`: content
    /// registered, `[Provide]` methods answering.
    on_load: Vec<String>,
    /// The members being written are a command's (not the file's own).
    in_command: bool,
    /// The mod the source is of: its messages' channels' namespace.
    mod_id: Option<String>,
    /// The file has a `$Load`.
    has_load: bool,
    /// The code being written is a `$Statics`' (`statics.rs`): its type's
    /// static fields are its own.
    in_statics: bool,
}

impl<'c> Gen<'c> {
    /// A generator for source `src` of file `name`, its expressions typed
    /// `types`, its declarations and the library's `index`.
    fn new(types: &'c Types, index: index::Index, name: &'c str, src: &'c str) -> Self {
        Gen {
            code: Code::new(),
            types,
            index,
            report: yog_analysis::Report::new(name, src),
            file: file_name(name),
            temps: 0,
            nested: true,
            owner: None,
            kept_types: Vec::new(),
            raisers: Vec::new(),
            continues: Vec::new(),
            cells: std::collections::HashSet::new(),
            given_back: Vec::new(),
            runtime_call: false,
            type_params: Vec::new(),
            owner_generics: Vec::new(),
            machine: None,
            renamed: Vec::new(),
            this_name: None,
            breaks: Vec::new(),
            statik: true,
            class_name: String::new(),
            boxed: std::collections::HashSet::new(),
            on_load: Vec::new(),
            has_load: false,
            in_statics: false,
            mod_id: None,
            in_command: false,
        }
    }
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

/// Java's word `word` (`public`, `static`) as it stands before what it
/// marks: followed by a space.
pub(crate) fn modifier(word: &str) -> String {
    format!("{word} ")
}
