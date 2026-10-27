//! Procedural macros: written in Yog Script, run by the interpreter while
//! a source is checked.

use yog_ids::script_problem;
use yog_script_build::{MacroLibrary, Project, check_source_using};

const LIBRARY: &str = "using System.Meta;\n\
    [ProcMacro]\nstring Twice(string input) => \"(\" + input + \") * 2\";\n\
    [DeriveMacro]\nstring Describe(Item item) {\n\
        let mut names = \"\";\n\
        foreach (let f in item.Fields) { names = names + f.Name + \" \"; }\n\
        return \"string \" + item.Name + \"Fields() => \\\"\" + names.Trim() + \"\\\";\";\n\
    }\n\
    [Macro]\nclass Logged : Attribute {\n\
        public static string Expand(string args, Item item) =>\n\
            item.Source + \" string \" + item.Name + \"Log() => \\\"\" + args + \"\\\";\";\n\
    }\n";

fn library() -> MacroLibrary {
    match MacroLibrary::load(&[("Macros.ys".to_string(), LIBRARY.to_string())]) {
        Ok(lib) => lib,
        Err(problems) => panic!("{problems:#?}"),
    }
}

fn codes(src: &str) -> Vec<String> {
    let lib = library();
    check_source_using(
        "A.ys",
        src,
        |_| Err(String::new()),
        Project {
            macros: Some(&lib),
            ..Project::default()
        },
    )
    .into_iter()
    .filter_map(|d| d.code.map(|c| c.to_string()))
    .collect()
}

#[test]
fn a_function_like_macro_writes_code() {
    assert!(codes("i32 F() => Twice!(1 + 2);\n").is_empty());
    // What it writes is checked as code.
    assert_eq!(
        codes("string F() => Twice!(1 + 2);\n"),
        [script_problem::TYPE_MISMATCH]
    );
}

#[test]
fn a_derive_adds_after_its_type() {
    let src = "[Derive(Describe)]\nrecord Pos(i32 X, i32 Y);\nstring F() => PosFields();\n";
    assert!(codes(src).is_empty(), "{:?}", codes(src));
    assert_eq!(
        codes("[Derive(Nope)]\nrecord Pos(i32 X);\n"),
        [script_problem::MACRO]
    );
}

#[test]
fn an_attribute_macro_rewrites_its_declaration() {
    let src =
        "[Logged(hello)]\nrecord Pos(i32 X);\nstring F() => PosLog();\ni32 G() => new Pos(1).X;\n";
    assert!(codes(src).is_empty(), "{:?}", codes(src));
}
