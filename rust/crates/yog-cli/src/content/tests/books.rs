//! `.ybook`s and their page types and recipe layouts.

use super::*;

#[test]
fn compiles_a_book() {
    let book = r#"{
        "id": "guide",
        "name": "@Book.title",
        "categories": [{ "id": "mymod:basics", "name": "Basics", "icon": "@item:minecraft:book" }],
        "entries": [{
            "id": "mymod:ruby", "name": "@item:mymod:ruby", "category": "@category:mymod:basics",
            "pages": [
                { "type": "text", "text": "@@not a ref, {link=@entry:mymod:ruby|see}" },
                { "type": "spotlight", "item": "@item:mymod:ruby" },
                { "type": "entity", "entity": "@entity:minecraft:allay" }
            ]
        }]
    }"#;
    let ruby = r#"{ "name": "Ruby", "model": { "parent": "generated", "textures": { "layer0": "@texture:minecraft:item/emerald" } } }"#;
    let root = project(&[("content/guide.ybook", book), ("content/ruby.yitem", ruby)]);
    let mut refs = yres::Refs::default();
    refs.texts.insert(
        "Book.title".into(),
        ("mymod.book.title".into(), "Guide".into()),
    );
    let out = compile(&root, "mymod", &refs).unwrap();
    assert_eq!(out.books, 1);
    let src = out.source.unwrap().all();
    assert!(src.contains(r#"registry.register_book_data("mymod:guide", include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/generated/books/mymod__guide.ydata")));"#), "{src}");
    // The runtime reads the bytes by the book's schema; references come back as ids.
    assert_eq!(out.data_files[0].0, "books/mymod__guide.ydata");
    let data = yog_data::binary::decode(&out.data_files[0].1).unwrap();
    let text = yog_data::text::to_string(&data);
    assert!(
        text.contains("mymod.book.title") && text.contains("item.mymod.ruby"),
        "{text}"
    );
    assert!(
        text.contains("not a ref") && text.contains("i18n"),
        "{text}"
    );
    let read = yog_data::from_value::<yog_formats::book::Book>(&data)
        .unwrap()
        .value;
    assert_eq!(read.entries[0].category, "mymod:basics");
    assert!(
        matches!(&read.entries[0].pages[1], yog_formats::book::BookPage::Spotlight { item, .. } if item == "mymod:ruby"),
        "references become ids"
    );

    // Bare ids, missing content and version-only ids are errors, each on its line.
    let bad = r#"{ "id": "x",
        "categories": [{ "id": "c", "name": "C" }],
        "entries": [{ "id": "e", "name": "E", "category": "c",
            "pages": [
                { "type": "spotlight", "item": "minecraft:stick" },
                { "type": "spotlight", "item": "@item:minecraft:grass" },
                { "type": "recipe", "recipe": "@recipe:mymod:nope" },
                { "type": "entity", "entity": "@entity:create:train" }
            ] }] }"#;
    let e = compile(
        &project(&[("content/bad.ybook", bad)]),
        "mymod",
        &yres::Refs::default(),
    )
    .unwrap_err();
    assert!(
        e.contains(
            "bad.ybook:3:59: entries[0].category: `c`: name it by a reference — `@category:c`"
        ),
        "{e}"
    );
    assert!(
        e.contains("bad.ybook:5:") && e.contains("`@item:minecraft:stick`"),
        "{e}"
    );
    let newest = super::refs::versions().last().expect("supported versions");
    assert!(e.contains(&format!("minecraft:grass isn't in Minecraft {newest} (the mod's datapack; it's in 1.20.1) — there it's minecraft:short_grass")), "{e}");
    assert!(e.contains("no recipe mymod:nope in this mod"), "{e}");
    assert!(e.contains("[external.create]"), "{e}");

    let bad = project(&[(
        "content/bad.ybook",
        r#"{ "id": "x", "entries": [{ "pages": [{ "type": "nope" }] }] }"#,
    )]);
    let e = compile(&bad, "mymod", &yres::Refs::default()).unwrap_err();
    assert!(
        e.contains("entries[0].pages[0].type: unknown page type `nope`"),
        "{e}"
    );
    let bad = project(&[(
        "content/bad.ybook",
        r#"{ "id": "x", "name": "X", "categories": [{ "id": "c", "name": "C" }] }"#,
    )]);
    let e = compile(&bad, "mymod", &yres::Refs::default()).unwrap_err();
    assert!(
        e.contains("categories[0].id: `c`: write its namespace (`mymod:c`)"),
        "{e}"
    );
}

#[test]
fn books_use_the_mods_own_page_types() {
    let ty = r#"<Component name="PatternPage" book_page="pattern">
        <Property name="name" type="book_text"/>
        <Property name="patterns" type="data" default="[]"/>
        <BookHeader text="{x:Bind name}"/>
        <Drawn drawer="mymod:pattern" h="64" data="{x:Bind patterns}"/>
    </Component>"#;
    let book = |page: &str| {
        format!(
            r#"{{ id: "guide", name: "G", categories: [{{ id: "mymod:c", name: "C" }}],
        entries: [{{ id: "mymod:e", name: "E", category: "@category:mymod:c", pages: [{page}] }}] }}"#
        )
    };
    let ok = book(
        r#"{ type: "mymod:pattern", name: "@item:minecraft:stick", patterns: [{ startdir: "EAST", signature: "qaq" }] }"#,
    );
    let c = compile(
        &project(&[
            ("src/ui/pattern_page.yui", ty),
            ("content/guide.ybook", &ok),
        ]),
        "mymod",
        &yres::Refs::default(),
    )
    .unwrap();
    let b = yog_data::read_bytes::<yog_formats::book::Book>(&c.data_files[0].1).unwrap();
    assert_eq!(b.page_types.len(), 1, "the type goes with the book");
    let yog_formats::book::BookPage::Custom(p) = &b.entries[0].pages[0] else {
        panic!("{:?}", b.entries[0].pages[0])
    };
    assert_eq!(
        p.fields.get("name").and_then(|n| n.as_str()),
        Some("item.minecraft.stick"),
        "texts resolve as the book's do"
    );
    assert!(b.i18n);

    let unknown = book(r#"{ type: "mymod:pattern", name: "N", color: 3 }"#);
    let e = compile(
        &project(&[
            ("src/ui/pattern_page.yui", ty),
            ("content/guide.ybook", &unknown),
        ]),
        "mymod",
        &yres::Refs::default(),
    )
    .unwrap_err();
    assert!(
        e.contains("mymod:pattern has no field `color` (name, patterns)"),
        "{e}"
    );
    let missing = book(r#"{ type: "mymod:nope" }"#);
    let e = compile(
        &project(&[("content/guide.ybook", &missing)]),
        "mymod",
        &yres::Refs::default(),
    )
    .unwrap_err();
    assert!(e.contains("no page type mymod:nope"), "{e}");
}

#[test]
fn books_lay_out_the_mods_own_recipe_kinds() {
    let recipe = r#"{ type: "mymod:infusion", slots: { core: ["@item:minecraft:diamond"] }, result: "@item:minecraft:emerald" }"#;
    let book = r#"{ id: "guide", name: "G", categories: [{ id: "mymod:c", name: "C" }],
        entries: [{ id: "mymod:e", name: "E", category: "@category:mymod:c",
            pages: [{ type: "recipe", recipe: "@recipe:mymod:infusion" }] }] }"#;
    let kind = r#"{ kind: "recipe_kind", id: "infusion", slots: { core: { required: true } } }"#;
    let layout = r#"<Component name="InfusionLayout" recipe_layout="infusion" width="96" height="24">
            <RecipeSlot slot="core" x="4" y="4" frame="true"/>
            <ItemSlot item="minecraft:beacon" x="40" y="4"/>
            <RecipeResult x="76" y="4" frame="true"/>
        </Component>"#;
    let files = [
        ("content/infusion.yrecipe", recipe),
        ("content/guide.ybook", book),
        ("content/infusion.ydef", kind),
    ];
    let e = compile(&project(&files), "mymod", &yres::Refs::default()).unwrap_err();
    assert!(
        e.contains("entries[0].pages[0].recipe: no layout for recipe kind mymod:infusion"),
        "{e}"
    );
    let c = compile(
        &project(&[
            files[0],
            files[1],
            files[2],
            ("src/ui/infusion.yui", layout),
        ]),
        "mymod",
        &yres::Refs::default(),
    )
    .unwrap();
    let book = yog_data::read_bytes::<yog_formats::book::Book>(&c.data_files[0].1).unwrap();
    assert_eq!(
        book.layouts[0].kinds,
        ["mymod:infusion"],
        "the book carries the mod's layouts"
    );
    // What a layout names is checked: the kinds, a mod kind's slots, the stations.
    let bad = r#"<Component recipe_layout="create:mixing infusion" width="1" height="1">
            <RecipeSlot slot="ring"/><ItemSlot item="minecraft:nope"/></Component>"#;
    let e = compile(
        &project(&[files[2], ("src/ui/bad.yui", bad)]),
        "mymod",
        &yres::Refs::default(),
    )
    .unwrap_err();
    assert!(e.contains("`create:mixing`: one of shaped"), "{e}");
    assert!(
        e.contains("mymod:infusion has no slot `ring` — core"),
        "{e}"
    );
    assert!(
        e.contains("<ItemSlot item=\"minecraft:nope\">: no item minecraft:nope in Minecraft"),
        "{e}"
    );
}
