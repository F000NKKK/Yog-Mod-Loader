//! A `.yns` read as one game version: `#if MC …` (the compiler declares
//! `MC`, the toolkit's preprocessor applies it).

use yog_ids::script_problem;
use yog_script_analyze::index::ClassIndex;
use yog_script_analyze::{check, check_with};

fn codes(problems: Vec<yog_data::Diagnostic>) -> Vec<String> {
    problems
        .into_iter()
        .filter_map(|d| d.code.map(|c| c.to_string()))
        .collect()
}

/// A game of version `mc` with no classes.
fn game(mc: &str) -> ClassIndex {
    ClassIndex {
        mc: mc.into(),
        classes: vec![],
    }
}

const SRC: &str = "targets mc 1.20.1..26.2;\n\n#if MC >= 1.21\ni32 Armor() => 2;\n#else\ni32 Armor() => \"old\";\n#endif\n";

#[test]
fn each_version_reads_its_branch() {
    // One name in two branches is no duplicate: one branch is read a version.
    assert!(codes(check_with("a.yns", SRC, Some(&game("1.21.1"))).problems).is_empty());
    assert_eq!(
        codes(check_with("a.yns", SRC, Some(&game("1.20.1"))).problems),
        [script_problem::TYPE_MISMATCH]
    );
    // Without a version (an editor's check), the newest.
    assert!(codes(check("a.yns", SRC).problems).is_empty());
}

#[test]
fn directives_are_a_yns_files_and_well_formed() {
    assert_eq!(
        codes(check("A.ys", "#if MC >= 1.21\n#endif\n").problems),
        [script_problem::DIRECTIVE]
    );
    let open = "targets mc 1.20.1..26.2;\n#if MC >= 1.21\n";
    assert_eq!(
        codes(check("a.yns", open).problems),
        [script_problem::DIRECTIVE]
    );
    // A tag mid-line is the language's, not a directive.
    let tag = "bool F(Block b) => b in #minecraft:logs;\n";
    assert!(!codes(check("A.ys", tag).problems).contains(&script_problem::DIRECTIVE.to_string()));
}
