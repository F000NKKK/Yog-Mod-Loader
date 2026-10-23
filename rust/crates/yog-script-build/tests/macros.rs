//! Procedural macros: written in Yog Script, run by the interpreter while
//! a source is checked.

use yog_ids::script_problem;
use yog_script_build::{MacroLibrary, check_source_using};

const LIBRARY: &str = "using System.Meta;\n\
    [ProcMacro]\nstring Twice(string input) => \"(\" + input + \") * 2\";\n\
    [DeriveMacro]\nstring Describe(Item item) {\n\
        let mut names = \"\";\n\
        foreach (let f in item.Fields) { names = names + f.Name + \" \"; }\n\
        return \"string \" + item.Name + \"Fields() => \\\"\" + names.Trim() + \"\\\";\";\n\
    }\n";

fn library() -> MacroLibrary {
    match MacroLibrary::load(&[("macros.ys".to_string(), LIBRARY.to_string())]) {
        Ok(lib) => lib,
        Err(problems) => panic!("{problems:#?}"),
    }
}

fn codes(src: &str) -> Vec<String> {
    let lib = library();
    check_source_using("a.ys", src, |_| Err(String::new()), Some(&lib))
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
