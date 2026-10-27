use super::*;

#[test]
fn resolves_references_to_their_texts() {
    let mut texts = Texts {
        refs: HashMap::new(),
        lang: HashMap::new(),
    };
    texts
        .refs
        .insert("Book.title".to_string(), "Guide".to_string());
    texts
        .lang
        .insert("item.mymod.sapphire".to_string(), "Сапфир".to_string());
    let v = yog_data::text::parse_value(
        r#"{
            name: "@res:Book.title", subtitle: "@item:mymod:ruby_block", landing_text: "@@x",
            categories: [{ id: "m:c", name: "@item:mymod:sapphire", icon: "@item:mymod:sapphire" }],
            entries: [{ id: "m:e", name: "@res:Book.nope", category: "@category:m:c", pages: [] }],
            id: "m:b",
        }"#,
    )
    .unwrap();
    let mut r = Reader {
        texts: &texts,
        missing: Vec::new(),
    };
    let b = yog_data::from_value_with::<Book>(&v, &mut r)
        .unwrap_or_else(|p| panic!("{p:?}"))
        .value;
    assert_eq!(b.name, "Guide");
    assert_eq!(
        b.subtitle.as_deref(),
        Some("Ruby Block"),
        "no translation: made from the id"
    );
    assert_eq!(b.categories[0].name, "Сапфир", "a name in a text");
    assert_eq!(
        b.categories[0].icon.as_deref(),
        Some("mymod:sapphire"),
        "an id in an id field"
    );
    assert_eq!(b.entries[0].category, "m:c");
    assert_eq!(b.landing_text, "@x");
    assert_eq!(r.missing, ["@res:Book.nope"]);
}

#[test]
fn draws_what_it_can_of_a_broken_book() {
    let text = r#"{ id: "t:guide", name: "Guide",
            categories: [{ id: "t:c", name: "C" }],
            entries: [{ id: "t:e", category: "@category:t:c", pages: [
                { type: "spotlight", item: "" },
                { type: "text", text: "kept" },
            ] }] }"#;
    let (book, problem) = parse(text, None, None).unwrap();
    let problem = problem.unwrap();
    assert!(
        problem.starts_with("3:23: entries[0]: missing `name`"),
        "{problem}"
    );
    assert_eq!(
        book.entries[0].pages.len(),
        1,
        "the broken page is left out"
    );
}

#[test]
fn draws_and_clicks_through_a_book() {
    let text = r#"{ "id": "t:guide", "name": "Guide",
            "categories": [{ "id": "t:c", "name": "Basics" }],
            "entries": [{ "id": "t:e", "name": "First", "category": "@category:t:c",
                "pages": [{ "type": "text", "text": "one" }, { "type": "text", "text": "two" }] }] }"#;
    let args = |action| BookArgs {
        path: "/tmp/test.ybook".into(),
        text: text.into(),
        screen_w: 400,
        screen_h: 240,
        scale: 1,
        action,
        locale: None,
    };
    let r = render(&args(None));
    assert!(r.error.is_none() && r.image.is_some(), "{:?}", r.error);
    assert_eq!(r.shown, None);
    let r = render(&args(Some(BookAction::Show {
        entry: "t:e".into(),
        page: 1,
    })));
    assert_eq!(r.shown, Some(("t:e".into(), 1)));
    let r = render(&args(Some(BookAction::Home)));
    assert_eq!(r.shown, None);
    close("/tmp/test.ybook");
}

#[test]
fn a_book_page_component_is_drawn_as_the_book_shows_it() {
    let dir = std::env::temp_dir().join(format!("yog-idle-page-preview-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src/ui")).unwrap();
    std::fs::write(dir.join("yog.toml"), "[mod]\nid = \"mymod\"\n").unwrap();
    let yui = r#"<Component name="Note" book_page="note">
            <Property name="title" type="text"/>
            <Property name="body" type="book_text" default="Hello"/>
            <BookHeader text="{x:Bind title}"/>
            <BookText text="{x:Bind body}"/>
            <Drawn drawer="mymod:thing" h="32" data="{x:Bind title}"/>
        </Component>"#;
    let args = BookArgs {
        path: dir.join("src/ui/note.yui").to_string_lossy().into_owned(),
        text: yui.into(),
        screen_w: 480,
        screen_h: 270,
        scale: 1,
        action: None,
        locale: None,
    };
    let r = page_preview(&args);
    assert!(r.error.is_none(), "{:?}", r.error);
    assert!(r.image.is_some());
    assert_eq!(r.shown, Some(("mymod:page_preview".to_string(), 0)));
    let _ = std::fs::remove_dir_all(&dir);
}
