use super::*;

fn defines(pairs: &[(&str, &str)]) -> Defines {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

#[test]
fn what_the_conditions_leave_out_is_blanked_in_place() {
    let src = "a\n#if MC >= 1.21\nnew\n#elif MC == 1.20.1\nold\n#else\nother\n#endif\nz\n";
    let out = preprocess(src, &defines(&[("MC", "1.21.1")]));
    assert!(out.problems.is_empty(), "{:?}", out.problems);
    assert_eq!(out.text.len(), src.len());
    assert_eq!(
        out.text.split_whitespace().collect::<Vec<_>>(),
        ["a", "new", "z"]
    );
    let out = preprocess(src, &defines(&[("MC", "1.20.1")]));
    assert_eq!(
        out.text.split_whitespace().collect::<Vec<_>>(),
        ["a", "old", "z"]
    );
    let out = preprocess(src, &defines(&[("MC", "1.19")]));
    assert_eq!(
        out.text.split_whitespace().collect::<Vec<_>>(),
        ["a", "other", "z"]
    );
}

#[test]
fn conditions_combine_and_names_define() {
    let src = "#define DEBUG\n#if (MC > 1.9 && DEBUG) || !CLIENT\nyes\n#endif\n#undef DEBUG\n#if DEBUG\nno\n#endif\n";
    let out = preprocess(src, &defines(&[("MC", "1.21"), ("CLIENT", "true")]));
    assert!(out.problems.is_empty(), "{:?}", out.problems);
    assert!(out.directives);
    assert_eq!(out.text.split_whitespace().collect::<Vec<_>>(), ["yes"]);
}

#[test]
fn directives_out_of_place_are_found() {
    let kinds = |src: &str| {
        preprocess(src, &Defines::new())
            .problems
            .into_iter()
            .map(|(_, p)| p)
            .collect::<Vec<_>>()
    };
    assert_eq!(kinds("#endif\n"), [Problem::Unmatched]);
    assert_eq!(kinds("#if A\n"), [Problem::Unterminated]);
    assert_eq!(
        kinds("#if A\n#else\n#elif B\n#endif\n"),
        [Problem::Unmatched]
    );
    assert!(matches!(
        kinds("#if A >=\n#endif\n").as_slice(),
        [Problem::BadCondition(_)]
    ));
    // `#tag` mid-line and a `#` that isn't a directive's are the language's.
    assert!(!preprocess("x = #minecraft:logs;\n#minecraft\n", &Defines::new()).directives);
}
