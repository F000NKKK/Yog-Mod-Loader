
use super::*;

#[test]
fn book_pages_and_recipe_layouts_make_no_code() {
    for src in [
        r#"<Component name="123Layout" recipe_layout="123" width="10" height="10"><RecipeSlot slot="a"/></Component>"#,
        r#"<Component book_page="note"><BookText text="x"/></Component>"#,
    ] {
        assert_eq!(
            generate_for(src, "x", Some("m")).unwrap(),
            (String::new(), Vec::new())
        );
    }
}

fn screen(body: &str) -> String {
    format!(r#"<Screen id="mymod:s">{body}</Screen>"#)
}

#[test]
fn custom_components() {
    let (code, _) = generate_for(
        &screen(
            r##"<StatBar x="4" w="80" label="@@hp" value="0.5" on_select="pick" bg="#FF000000"/>"##,
        ),
        "s",
        None,
    )
    .unwrap();
    assert!(
            code.contains(r#"::yog_api::ui::component::build("StatBar", &[("label", &*"@hp"), ("value", "0.5"), ("on_select", "pick")])"#),
            "{code}"
        );
    assert!(
        code.contains(".bg(::yog_api::ui::Color(0xFF000000))")
            && code.contains(".w(Units::Px(80.0))"),
        "{code}"
    );
    assert!(!code.contains(".value("), "{code}");
    assert!(generate_for(&screen(r#"<statBar/>"#), "s", None).is_err());
    assert!(generate_for(&screen(r#"<StatBar><Label/></StatBar>"#), "s", None).is_err());
}

#[test]
fn surfaces() {
    let (code, _) = generate_for(
        &screen(r#"<Surface w="100" h="50" source="mod:grid"/>"#),
        "s",
        Some("example"),
    )
    .unwrap();
    assert!(
        code.contains(r#"widget::surface("example:grid")"#),
        "{code}"
    );
    let (code, _) = generate_for(&screen(r#"<Surface source="other:pad"/>"#), "s", None).unwrap();
    assert!(code.contains(r#"widget::surface("other:pad")"#), "{code}");
    assert!(generate_for(&screen(r#"<Surface/>"#), "s", None).is_err());
    assert!(generate_for(&screen(r#"<Surface source="pad"/>"#), "s", None).is_err());
    assert!(generate_for(&screen(r#"<Surface source="mod:pad"/>"#), "s", None).is_err());
    assert!(generate_for(&screen(r#"<Surface source="a:b" text="x"/>"#), "s", None).is_err());
}

#[test]
fn image_sources() {
    let (code, warnings) = generate_for(
            &screen(r##"<Image w="156" h="29" source="item:apple" stretch="UniformToFill" tint="#80FFFFFF"/>"##),
            "s",
            None,
        )
        .unwrap();
    assert!(code.contains(r#"widget::image("item:apple").stretch(::yog_api::ui::Stretch::UniformToFill).tint(::yog_api::ui::Color(0x80FFFFFF))"#), "{code}");
    assert!(
        code.contains(".w(Units::Px(156.0))") || code.contains("w(156"),
        "{code}"
    );
    assert!(warnings.is_empty(), "{warnings:?}");

    let (code, _) = generate_for(
        &screen(r#"<Image source="mod:gui/logo" uv="0 0 32 16"/>"#),
        "s",
        Some("mymod"),
    )
    .unwrap();
    assert!(
        code.contains(
            r#"widget::image("mymod:textures/gui/logo.png").region([0.0, 0.0, 32.0, 16.0])"#
        ),
        "{code}"
    );
    assert!(generate_for(&screen(r#"<Image source="mod:gui/logo"/>"#), "s", None).is_err());
    assert!(generate_for(&screen(r#"<Image source="sprite:nope"/>"#), "s", None).is_err());
    assert!(generate_for(&screen(r#"<Image/>"#), "s", None).is_err());
    assert!(
        generate_for(
            &screen(r#"<Image source="item:apple" stretch="big"/>"#),
            "s",
            None
        )
        .is_err()
    );
}

#[test]
fn raw_texture_paths_are_errors_naming_the_alias() {
    let err = generate_for(
        &screen(r#"<Image source="minecraft:textures/item/apple.png"/>"#),
        "s",
        None,
    )
    .unwrap_err();
    assert!(err.contains(r#"source="item:apple""#), "{err}");
    assert!(
        generate_for(&screen(r#"<Image texture="item:apple"/>"#), "s", None)
            .unwrap_err()
            .contains("no attribute `texture`")
    );
}

#[test]
fn image_backgrounds() {
    let code = generate_for(
        &screen(r##"<Column bg="sprite:panel"><Label bg="#FF000000"/></Column>"##),
        "s",
        None,
    )
    .unwrap()
    .0;
    assert!(code.contains(r#"bg_image("sprite:panel")"#), "{code}");
    assert!(
        code.contains("bg(::yog_api::ui::Color(0xFF000000))"),
        "{code}"
    );
    let err = generate_for(&screen(r#"<Column bg="sprite:nope"/>"#), "s", None).unwrap_err();
    assert!(
        err.contains("<Column> bg=\"sprite:nope\": unknown sprite"),
        "{err}"
    );
}

#[test]
fn inventory_slots() {
    let code = generate_for(
        &screen(r#"<Column><Slot index="2"/><PlayerInventory/></Column>"#),
        "s",
        None,
    )
    .unwrap()
    .0;
    assert!(code.contains("widget::slot(2)"), "{code}");
    assert!(code.contains("widget::player_inventory()"), "{code}");
    let code = generate_for(
        &screen(r#"<Slot index="1" accepts="minecraft:redstone #c:dusts"/>"#),
        "s",
        None,
    )
    .unwrap()
    .0;
    assert!(
        code.contains(r##"widget::slot_accepting(1, &["minecraft:redstone", "#c:dusts"])"##),
        "{code}"
    );
    let err = generate_for(
        &screen(r#"<Slot index="1" accepts="redstone"/>"#),
        "s",
        None,
    )
    .unwrap_err();
    assert!(err.contains("with its namespace"), "{err}");
    let err = generate_for(&screen(r#"<Slot index="first"/>"#), "s", None).unwrap_err();
    assert!(
        err.contains("expected the container slot's number"),
        "{err}"
    );
}

const SAMPLE: &str = r##"<?xml version="1.0" encoding="utf-8"?>
<!-- a comment -->
<Screen id="mymod:main" width="176" height="166" bg="#C0101010" padding="8">
  <Label id="title" x="8" y="6" text="Main &amp; Menu" color="#FFFFFF" no_wrap="true"/>
  <Button x="38" y="40" w="100" h="20" text="Play" bg="#FF555555" on_click="mymod:play"/>
  <Row x="8" y="70" w="50%" gap="2" align="center">
    <InvSlot item="minecraft:diamond" count="3"/>
    <Spacer flex="1"/>
  </Row>
  <ScrollView id="list" x="0" y="100" w="80" h="40">
    <Label text="A long line" ellipsis="true"/>
  </ScrollView>
</Screen>
"##;

#[test]
fn parses_the_subset() {
    let root = parse_xml(SAMPLE).unwrap();
    assert_eq!(root.name, "Screen");
    assert_eq!(root.children.len(), 4);
    assert_eq!(root.children[0].attr("text"), Some("Main & Menu"));
    assert_eq!(root.children[2].children[0].attr("count"), Some("3"));
    assert_eq!(root.children[1].line, 5);
}

#[test]
fn generates_builder_code() {
    let code = generate(SAMPLE, "main").unwrap();
    assert!(code.contains(r#"pub const ID: &str = "mymod:main";"#));
    assert!(code.contains(r#"widget::label("Main & Menu")"#));
    assert!(code.contains(".at(8.0, 6.0)"));
    assert!(code.contains(".color(::yog_api::ui::Color(0xFFFFFFFF))"));
    assert!(code.contains(".bg(::yog_api::ui::Color(0xC0101010))"));
    assert!(code.contains(".no_wrap()"));
    assert!(code.contains(".w(Units::Percent(50.0))"));
    assert!(code.contains("widget::inv_slot(\"minecraft:diamond\", 3)"));
    assert!(code.contains(r#"pub const MYMOD_PLAY: &str = "mymod:play";"#));
    assert!(code.contains(r#"pub const TITLE: &str = "title";"#));
    assert!(code.contains(".padding(8.0, 8.0, 8.0, 8.0)"));
    assert!(code.contains("widget::scroll_view()"));
    assert!(code.contains(".ellipsis()"));
    assert!(code.contains("pub fn with_last<R>"));
    assert!(code.starts_with("// <auto-generated>"));
    assert!(code.contains(r#"yog_api::ui_include!("main");"#));
    assert!(code.contains("pub fn close()"));
    // Children of a Row aren't positioned unless they say so.
    assert!(!code.contains("widget::spacer()\n                .at("));
}

#[test]
fn reports_errors_with_lines() {
    let bad = "<Screen id=\"a:b\">\n  <Label txt=\"x\"/>\n</Screen>";
    let err = generate(bad, "x.yui").unwrap_err();
    assert!(err.contains("line 2") && err.contains("txt"), "{err}");
    assert!(generate("<Screen id=\"a:b\"><Label/></Scren>", "x.yui").is_err());
    assert!(generate("<Column/>", "x.yui").is_err());
    assert!(generate("<Screen/>", "x.yui").unwrap_err().contains("id"));
    let bad_color = "<Screen id=\"a:b\" bg=\"red\"/>";
    assert!(
        generate(bad_color, "x.yui")
            .unwrap_err()
            .contains("#RRGGBB")
    );
}

/// What the IDE's Format Document writes (Visual Studio's XAML layout):
/// one attribute per line, comments among the children, ` />`.
#[test]
fn parses_formatted_markup() {
    let src = r##"<?xml version="1.0" encoding="utf-8"?>
<Screen id="mymod:main"
        width="176"
        height="166"
        bg="#C0101010">
    <!-- the title -->
    <Label id="title"
           x="8"
           y="6"
           text="Main"
           no_wrap="true" />

    <Row x="8" y="70" w="160">
        <!--
          a multi-line comment
        -->
        <Spacer flex="1" />
    </Row>
</Screen>
"##;
    let root = parse_xml(src).unwrap();
    assert_eq!(root.attr("height"), Some("166"));
    assert_eq!(root.children.len(), 2);
    assert_eq!(root.children[0].attr("no_wrap"), Some("true"));
    assert_eq!(root.children[0].line, 7);
    assert_eq!(root.children[1].children.len(), 1);
    let code = generate(src, "main.yui").unwrap();
    assert!(code.contains(".at(8.0, 6.0)"));
}

#[test]
fn text_box_and_font() {
    let src = r#"<Screen id="a:b">
  <TextBox id="search" w="100%" placeholder="Search..." max_length="64" on_change="search" on_submit="go" font="yog:font/uniform" shadow="false"/>
</Screen>"#;
    let code = generate(src, "x").unwrap();
    assert!(code.contains(r#"widget::text_box("")"#), "{code}");
    assert!(code.contains(r#".placeholder("Search...")"#));
    assert!(code.contains(".max_length(64)"));
    assert!(code.contains(r#".on_change("search")"#));
    assert!(code.contains(r#".on_submit("go")"#));
    assert!(code.contains(r#".font("yog:font/uniform")"#));
    assert!(code.contains(".shadow(false)"));
    assert!(code.contains(r#"pub const GO: &str = "go";"#));
    assert!(code.contains("pub fn text_box("));
    let bad = r#"<Screen id="a:b"><TextBox max_length="x"/></Screen>"#;
    assert!(generate(bad, "x").unwrap_err().contains("whole number"));
    let direct = r#"<Screen id="a:b"><Label text="x" font="minecraft:alt"/></Screen>"#;
    assert!(generate(direct, "x").unwrap_err().contains("yog:font/alt"));
}

#[test]
fn generated_paths() {
    let dir = std::env::temp_dir().join(format!("yog-yui-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src/screens")).unwrap();
    std::fs::write(dir.join("yog.toml"), "").unwrap();
    let yui = dir.join("src/screens/opts.yui");
    std::fs::write(&yui, r#"<Screen id="a:b"/>"#).unwrap();
    assert_eq!(project_root(&yui).as_deref(), Some(dir.as_path()));
    assert_eq!(include_name(&dir, &yui), "screens/opts");
    assert_eq!(generate_all(&dir).unwrap().screens, 1);
    let generated = dir.join(".yog-build/generated/screens/opts.g.rs");
    assert!(generated.is_file());
    std::fs::remove_file(&yui).unwrap();
    generate_all(&dir).unwrap();
    assert!(!generated.exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn const_names() {
    assert_eq!(const_name("mymod:btn-play"), "MYMOD_BTN_PLAY");
    assert_eq!(const_name("openMenu"), "OPEN_MENU");
    assert_eq!(const_name("1st"), "_1ST");
}

#[test]
fn text_starting_with_at_is_a_translation_key() {
    assert_eq!(
        text_expr("@lang:gui.done").unwrap(),
        "::yog_api::tr(\"gui.done\")"
    );
    assert_eq!(
        text_expr("@item:minecraft:stick").unwrap(),
        "::yog_api::tr(\"item.minecraft.stick\")"
    );
    assert_eq!(text_expr("@@home").unwrap(), "\"@home\"");
    assert_eq!(text_expr("plain").unwrap(), "\"plain\"");
    assert_eq!(text_expr("@").unwrap(), "\"@\"");
    // The old bare key, a missing text, something that isn't a text.
    assert!(
        text_expr("@gui.done")
            .unwrap_err()
            .contains("`@lang:gui.done`")
    );
    assert!(
        text_expr("@Strings.nope")
            .unwrap_err()
            .contains("no text `Strings.nope`")
    );
    assert!(
        text_expr("@image:sprite:x")
            .unwrap_err()
            .contains("isn't a text")
    );
    let mut refs = crate::yres::Refs::default();
    refs.texts.insert(
        "Strings.title".into(),
        ("mymod.title".into(), "Title".into()),
    );
    set_resource_refs(&refs);
    assert_eq!(
        text_expr("@Strings.title").unwrap(),
        "::yog_api::tr(\"mymod.title\")"
    );
    set_resource_refs(&crate::yres::Refs::default());
    let code = generate(
            r#"<Screen id="m:s" width="10" height="10"><Label text="@lang:m.title"/><Button text="@@x"/></Screen>"#,
            "s.yui",
        )
        .unwrap();
    assert!(
        code.contains("widget::label(::yog_api::tr(\"m.title\"))"),
        "{code}"
    );
    assert!(code.contains("widget::button(\"@x\")"), "{code}");
}

#[test]
fn color_attributes_resolve_yres_colors() {
    let mut refs = crate::yres::Refs::default();
    refs.colors.insert("Colors.text".into(), 0xFF12_3456);
    set_resource_refs(&refs);
    let code = generate(
        r#"<Screen id="m:s" width="10" height="10"><Label text="x" color="@Colors.text"/></Screen>"#,
        "s.yui",
    );
    let missing = generate(
        r#"<Screen id="m:s" width="10" height="10"><Label text="x" color="@Colors.nope"/></Screen>"#,
        "s.yui",
    );
    set_resource_refs(&crate::yres::Refs::default());
    let code = code.unwrap();
    assert!(code.contains("0xFF123456"), "{code}");
    assert!(missing.unwrap_err().contains("no color `Colors.nope`"));
}
