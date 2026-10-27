//! Object initializers: `new T(args) { Member = value }` — values fitting
//! their members, `required` members set.

use yog_ids::script_problem;
use yog_script_analyze::check;

fn codes(src: &str) -> Vec<String> {
    check("A.ys", src)
        .problems
        .into_iter()
        .filter_map(|d| d.code.map(|c| c.to_string()))
        .collect()
}

/// `required` members (C#'s): a `new` sets them, its own and inherited,
/// unless its constructor is `[SetsRequiredMembers]`; an initializer's
/// values fit their members.
#[test]
fn required_members_are_set() {
    let types = "using System.Diagnostics.CodeAnalysis;\n\
                 public class Base { public required i32 Id; }\n\
                 public class Item : Base {\n\
                     public required string Name { get; init; }\n\
                     public i32 Count { get; set; }\n\
                     public Item() { }\n\
                     [SetsRequiredMembers]\n\
                     public Item(string name) { Name = name; Id = 0; }\n\
                 }\n";
    let set = format!("{types}Item A() => new Item {{ Id = 1, Name = \"a\" }};\nItem B() => new Item(\"b\");\n");
    assert!(codes(&set).is_empty(), "{:?}", codes(&set));
    let unset = format!("{types}Item A() => new Item {{ Name = \"a\" }};\n");
    assert_eq!(codes(&unset), [script_problem::REQUIRED]);
    let wrong = format!("{types}Item A() => new Item(\"b\") {{ Count = \"x\" }};\n");
    assert_eq!(codes(&wrong), [script_problem::TYPE_MISMATCH]);
}
