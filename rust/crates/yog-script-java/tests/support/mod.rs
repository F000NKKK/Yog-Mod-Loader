//! What the tests share: a source compiled to Java, its problems when it isn't.

use yog_script_java::compile;

/// Source `src` (`Sample.ys`, checked clean) as Java, or what's not compiled.
pub fn java(src: &str) -> Result<String, Vec<String>> {
    let checked = yog_script_analyze::check("Sample.ys", src);
    assert!(checked.problems.is_empty(), "{:#?}", checked.problems);
    compile("Sample.ys", src, &checked, "yogscript.test")
        .map(|c| c.source)
        .map_err(|p| {
            p.into_iter()
                .map(|d| format!("{:?} {}", d.code, d.message))
                .collect()
        })
}
