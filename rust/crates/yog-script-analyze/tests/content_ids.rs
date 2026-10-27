//! Content's typed ids: `new ItemId("…")`, its text checked; each a
//! `ContentId<TSelf>`; no `@kind:` literal.

use yog_ids::script_problem;
use yog_script_analyze::check;

fn codes(src: &str) -> Vec<String> {
    check("A.ys", src)
        .problems
        .into_iter()
        .filter_map(|d| d.code.map(|c| c.to_string()))
        .collect()
}

#[test]
fn typed_ids_are_made_of_their_text() {
    let fine = "using Yog;\n\
                string Path<T>(ContentId<T> id) => id.Path;\n\
                string Tagged<T>(TagId<T> tag) => tag.Id;\n\
                string F() {\n\
                    ItemId stick = new ItemId(\"minecraft:stick\");\n\
                    BlockId stone = new BlockId(\"stone\");\n\
                    SoundGroupId wood = new SoundGroupId(\"minecraft:wood\");\n\
                    TagId<ItemId> ingots = new TagId<ItemId>(\"c:ingots\");\n\
                    return stick.Namespace + Path(stone) + wood.Id + Path(ingots) + Tagged(ingots);\n\
                }\n";
    assert!(codes(fine).is_empty(), "{:?}", check("A.ys", fine).problems);
    let not_an_id = "ItemId F() => new ItemId(\"Not An Id\");\n";
    assert_eq!(codes(not_an_id), [script_problem::BAD_REFERENCE]);
    let mixed = "BlockId F() => new ItemId(\"stick\");\n";
    assert_eq!(codes(mixed), [script_problem::TYPE_MISMATCH]);
}

/// `@item:…` is no literal: said so, with the typed id to write.
#[test]
fn the_old_literal_says_what_to_write() {
    let problems = check("A.ys", "string F() => @item:minecraft:stick;\n").problems;
    let message = problems
        .first()
        .map(|p| p.message.clone())
        .unwrap_or_default();
    assert!(
        message.contains("new ItemId(\"minecraft:stick\")"),
        "{message}"
    );
}

/// `#…` is no literal either: said so, with the typed tag id to write.
#[test]
fn the_old_tag_literal_says_what_to_write() {
    let problems = check("A.ys", "string F() => #minecraft:logs;\n").problems;
    let message = problems
        .first()
        .map(|p| p.message.clone())
        .unwrap_or_default();
    assert!(
        message.contains("new TagId<ItemId>(\"minecraft:logs\")"),
        "{message}"
    );
    let mixed = "TagId<BlockId> F() => new TagId<ItemId>(\"c:ingots\");\n";
    assert_eq!(codes(mixed), [script_problem::TYPE_MISMATCH]);
}
