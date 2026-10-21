use super::placeholders::{Segment, lang_text, params, segments};
use super::*;

#[test]
fn rich_text_spans_are_not_placeholders() {
    let seg = segments("{#8d6acc|Media} and {b|x} for {name}").unwrap();
    assert_eq!(
        seg,
        [
            Segment::Text("{#8d6acc|Media} and {b|x} for ".into()),
            Segment::Arg("name".into())
        ]
    );
}

#[test]
fn parses_all_shapes_in_order() {
    let text = "{\n  // Comments are fine.\n  \"b\": \"B\",\n  \"a\": { \"value\": \"A \\u00e9\", \"comment\": \"note\", \"key\": \"x.y\" },\n  \"n\": 5,\n  \"f\": false,\n  \"c\": { \"type\": \"color\", \"value\": \"#FF0000\" },\n  \"r\": { \"type\": \"number\", \"value\": -1.5e2 },\n  \"at\": \"@@everyone\",\n}\n";
    let entries = parse(text).unwrap();
    assert_eq!(entries.len(), 7);
    assert_eq!(entries[0].name, "b");
    assert_eq!(entries[1].line, 4);
    assert_eq!(entries[1].value, Value::Str("A é".into()));
    assert_eq!(entries[1].key.as_deref(), Some("x.y"));
    assert_eq!(entries[2].kind, Kind::Number);
    assert_eq!(entries[3].value, Value::Bool(false));
    assert_eq!(entries[4].kind, Kind::Color);
    assert_eq!(entries[5].value, Value::Num("-150.0".into()));
    assert_eq!(entries[6].value, Value::Str("@everyone".into()));
    // Written back, it reads the same.
    let again = parse(&write(&entries)).unwrap();
    assert_eq!(
        again
            .iter()
            .map(|e| (&e.name, e.kind, &e.value, &e.key))
            .collect::<Vec<_>>(),
        entries
            .iter()
            .map(|e| (&e.name, e.kind, &e.value, &e.key))
            .collect::<Vec<_>>()
    );
}

#[test]
fn errors_carry_lines() {
    let err = parse("{\n  \"a\": \"1\",\n  \"a\": \"2\"\n}").unwrap_err();
    assert_eq!(err, "3:3: `a` is already set in this object");
    let err = parse("{\n  \"a\": { \"type\": \"bool\", \"value\": \"x\" }\n}").unwrap_err();
    assert_eq!(err, "2:35: a.value: a bool needs a true/false value");
    let err = parse("{\n\"a\": { \"value\": \"x\", \"note\": \"y\" }\n}").unwrap_err();
    assert!(err.contains("2:22: a.note: unknown field `note`"), "{err}");
    let err =
        parse("{\n\"a\": { \"type\": \"image\", \"value\": \"@bytes:AAAA\", \"key\": \"k\" }\n}")
            .unwrap_err();
    assert!(
        err.contains("a.key: only string entries have a `key`"),
        "{err}"
    );
    let err = parse("{\n  \"a\": \"1\"\n  \"b\": \"2\"\n}").unwrap_err();
    assert_eq!(err, "3:3: expected `,` or `}`");
    // Every problem at once.
    let err =
        parse("{\n  \"a\": [],\n  \"b\": { \"type\": \"img\", \"value\": \"@Strings.x\" }\n}")
            .unwrap_err();
    assert_eq!(
        err.lines().collect::<Vec<_>>(),
        [
            "2:8: a: expected a string, a number or true/false, found a list",
            "3:18: b.type: unknown `img` — did you mean `image`?",
            "3:34: b.value: expected plain text, found a resource (write `@@` for a literal `@`)",
        ]
    );
    assert!(parse("[]").unwrap_err().contains("expected `{`"));
    assert!(parse("\u{FEFF}{ }").unwrap().is_empty());
}

#[test]
fn names_and_keys() {
    assert_eq!(accessor_name("screen.title"), "screen_title");
    assert_eq!(accessor_name("ModName"), "mod_name");
    assert!(valid_name("ruby.tooltip") && !valid_name("1a") && !valid_name("a..b"));
    assert!(valid_locale("pt_br") && valid_locale("lol_us") && !valid_locale("EN_US"));
    assert_eq!(default_key("Items", "m", "ruby"), "item.m.ruby");
    assert_eq!(default_key("Strings", "m", "title"), "m.title");
    assert_eq!(
        yog_formats::yres::split_file_name("Strings.ru_ru.yres"),
        Some(("Strings", Some("ru_ru")))
    );
    assert_eq!(
        yog_formats::yres::split_file_name("Icons.yres"),
        Some(("Icons", None))
    );
}

#[test]
fn placeholders_become_minecraft_format() {
    let segs = segments("{shown} of {total}: 100% {{ok}}").unwrap();
    let ps = params(&segs).unwrap();
    assert_eq!(ps, ["shown", "total"]);
    assert_eq!(lang_text(&segs, &ps).unwrap(), "%1$s of %2$s: 100%% {{ok}}");
    let ru = segments("из {total} показано {shown}").unwrap();
    assert_eq!(lang_text(&ru, &ps).unwrap(), "из %2$s показано %1$s");
    assert!(lang_text(&segments("{other}").unwrap(), &ps).is_err());
    let segs = segments("{1} and {0}").unwrap();
    assert_eq!(params(&segs).unwrap(), ["0", "1"]);
    assert!(params(&segments("{0} {name}").unwrap()).is_err());
    assert!(params(&segments("{1}").unwrap()).is_err());
    assert!(segments("a { b").is_err());
    assert!(segments("a } b").is_err());
}

fn temp_mod(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("yog-yres-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join(RESOURCES_DIR)).unwrap();
    dir
}

#[test]
fn compiles_a_mod() {
    let root = temp_mod("ok");
    let res = root.join(RESOURCES_DIR);
    std::fs::write(
        res.join("Strings.yres"),
        r#"{"title": {"value": "Mods", "comment": "c"}, "count": "{n} mods", "gone": "G"}"#,
    )
    .unwrap();
    std::fs::write(
        res.join("Strings.ru_ru.yres"),
        r#"{"title": "Моды", "count": "Модов: {n}", "x": "X"}"#,
    )
    .unwrap();
    std::fs::write(
        res.join("Icons.yres"),
        r##"{"images.logo": "@bytes:iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==", "accent": {"type": "color", "value": "#FF8800"}, "sounds.click": {"value": "@bytes:T2dnUw==", "event": "click"}, "max": 3, "on": true}"##,
    )
    .unwrap();
    std::fs::write(res.join("Items.yres"), r#"{"ruby": "Ruby"}"#).unwrap();
    std::fs::create_dir_all(root.join("assets/m/lang")).unwrap();
    std::fs::write(
        root.join("assets/m/lang/en_us.json"),
        r#"{"hand.key": "Hand"}"#,
    )
    .unwrap();

    let c = compile(&root, "m").unwrap();
    let out: BTreeMap<&str, String> = c
        .outputs
        .iter()
        .map(|(p, b)| (p.as_str(), String::from_utf8_lossy(b).into_owned()))
        .collect();
    assert_eq!(
        out["assets/m/lang/en_us.json"],
        "{\n  \"hand.key\": \"Hand\",\n  \"item.m.ruby\": \"Ruby\",\n  \"m.title\": \"Mods\",\n  \"m.count\": \"%1$s mods\",\n  \"m.gone\": \"G\"\n}\n"
    );
    assert_eq!(
        out["assets/m/lang/ru_ru.json"],
        "{\n  \"m.title\": \"Моды\",\n  \"m.count\": \"Модов: %1$s\"\n}\n"
    );
    assert!(
        out["assets/m/textures/images/logo.png"].starts_with("\u{89}PNG")
            || out.contains_key("assets/m/textures/images/logo.png")
    );
    assert_eq!(out["assets/m/sounds/sounds/click.ogg"], "OggS");
    assert!(out["assets/m/sounds.json"].contains("\"m:sounds/click\""));
    assert_eq!(c.refs.images["Icons.images.logo"], "m:images/logo");
    assert!(c.refs.pictures.contains_key("Icons.images.logo"));
    assert_eq!(
        c.refs.texts["Strings.title"],
        ("m.title".to_string(), "Mods".to_string())
    );
    let strings = &c.sources["Strings"];
    assert!(
        strings.contains("pub const TITLE_KEY: &'static str = \"m.title\";"),
        "{strings}"
    );
    assert!(
        strings.contains("pub fn count(n: impl ::std::fmt::Display)"),
        "{strings}"
    );
    assert!(
        strings.contains("::yog_api::tr_args(Self::COUNT_KEY, &[&n])"),
        "{strings}"
    );
    let icons = &c.sources["Icons"];
    assert!(
        icons.contains("pub const fn images_logo() -> &'static str {\n        \"m:images/logo\""),
        "{icons}"
    );
    assert!(
        icons.contains("pub const ACCENT: u32 = 0xFFFF8800;"),
        "{icons}"
    );
    assert!(icons.contains("pub const MAX: i64 = 3;"), "{icons}");
    assert!(icons.contains("pub const ON: bool = true;"), "{icons}");
    assert!(icons.contains("\"m:click\""), "{icons}");
    assert_eq!(c.warnings.len(), 2, "{:?}", c.warnings);
    assert!(c.warnings[0].contains("1 text(s) not translated") && c.warnings[0].contains("gone"));
    assert!(c.warnings[1].contains("left out: x"));

    write_sources(&root, &c).unwrap();
    let icons_rs = std::fs::read_to_string(root.join(GENERATED_DIR).join("Icons.g.rs")).unwrap();
    assert!(icons_rs.starts_with("// <auto-generated>"), "{icons_rs}");
    assert!(icons_rs.contains("pub struct Icons;"), "{icons_rs}");
    let index = std::fs::read_to_string(root.join(GENERATED_FILE)).unwrap();
    assert!(
        index.contains("/generated/resources/Icons.g.rs\"));"),
        "{index}"
    );
    assert!(
        index.contains("/generated/resources/Strings.g.rs\"));"),
        "{index}"
    );
    let (name, key) = resolve_text(&c.refs, "@Strings.title").unwrap();
    assert_eq!((name.as_str(), key.as_deref()), ("Mods", Some("m.title")));
    assert!(resolve_text(&c.refs, "@Strings.nope").is_err());
    assert_eq!(
        resolve_image(&c.refs, "@Icons.images.logo").unwrap(),
        "m:images/logo"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn reports_every_problem_with_file_and_line() {
    let root = temp_mod("errors");
    let res = root.join(RESOURCES_DIR);
    std::fs::write(
        res.join("Strings.yres"),
        "{\n  \"a\": \"A\",\n  \"b\": { \"value\": \"B\", \"key\": \"m.a\" },\n  \"img\": { \"type\": \"image\", \"value\": \"@bytes:AAAA\" },\n  \"bad name\": \"x\",\n  \"col\": { \"type\": \"color\", \"value\": \"red\" }\n}",
    )
    .unwrap();
    std::fs::write(res.join("Strings.en_us.yres"), "{}").unwrap();
    std::fs::write(res.join("Strings.de_de.yres"), "{\n  \"a\": 1\n}").unwrap();
    std::fs::write(res.join("Other.ru_ru.yres"), "{}").unwrap();
    let err = compile(&root, "m").unwrap_err();
    for needle in [
        "resources/Strings.yres:3: translation key \"m.a\" is also used at resources/Strings.yres:2",
        "resources/Strings.yres:4: \"img\": not a picture this reads",
        "resources/Strings.yres:5: \"bad name\" isn't a valid name",
        "resources/Strings.yres:6: \"col\": `red` isn't a color",
        "Strings.en_us.yres: the neutral file Strings.yres is already en_us",
        "resources/Strings.de_de.yres:2: \"a\": culture variants hold texts only",
        "resources/Other.ru_ru.yres: no neutral Other.yres",
    ] {
        assert!(err.contains(needle), "missing {needle:?} in:\n{err}");
    }
    let _ = std::fs::remove_dir_all(&root);
}
