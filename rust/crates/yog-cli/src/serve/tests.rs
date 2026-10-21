//! The service as a tool calls it: requests of plain values, plain answers.

use yog_data::Value;

use super::{Request, ServiceError, answer};

/// `method` called with `params` (Yog Data text).
fn call(method: &str, params: &str) -> Result<Value, ServiceError> {
    answer(&Request {
        method: method.into(),
        params: yog_data::text::parse_value(params).unwrap(),
    })
}

fn at<'a>(v: &'a Value, path: &[&str]) -> &'a Value {
    path.iter().fold(v, |v, k| match k.parse::<usize>() {
        Ok(i) => &v.as_list().unwrap()[i],
        Err(_) => v.get(k).unwrap(),
    })
}

#[test]
fn yres_round_trips_through_the_service() {
    let text = r##"{ "title": "Hi", "n": 3, "logo": "@bytes:iVBORw0KGgo=", "c": { "type": "color", "value": "#FF0000", "comment": "red" } }"##;
    let parsed = answer(&Request {
        method: "yres.parse".into(),
        params: Value::Map([("text", Value::from(text))].into_iter().collect()),
    })
    .unwrap();
    assert_eq!(
        at(&parsed, &["entries", "2", "type"]).as_str(),
        Some("image")
    );
    assert_eq!(
        at(&parsed, &["entries", "2", "value"]).as_str(),
        Some("@bytes:iVBORw0KGgo=")
    );
    let named = call(
        "yres.parse",
        r#"{ text: "{ \"cad.0_x\": \"a\", \"1a\": \"b\" }", set: "Items", mod: "m" }"#,
    )
    .unwrap();
    assert_eq!(
        at(&named, &["entries", "0", "accessor"]).as_str(),
        Some("cad_0_x")
    );
    assert!(at(&named, &["entries", "0"]).get("nameProblem").is_none());
    assert!(
        at(&named, &["entries", "1", "nameProblem"])
            .as_str()
            .is_some()
    );
    assert_eq!(
        at(&named, &["entries", "0", "translationKey"]).as_str(),
        Some("item.m.cad.0_x")
    );
    let entries = at(&parsed, &["entries"]).clone();
    let written = answer(&Request {
        method: "yres.write".into(),
        params: Value::Map([("entries", entries)].into_iter().collect()),
    })
    .unwrap();
    let again = answer(&Request {
        method: "yres.parse".into(),
        params: Value::Map([("text", written)].into_iter().collect()),
    })
    .unwrap();
    assert_eq!(
        at(&again, &["entries", "3", "comment"]).as_str(),
        Some("red")
    );
    assert!(call("nope", "null").is_err());
}

#[test]
fn typed_values_are_classified_by_the_service() {
    let r = call(
        "typed.classify",
        r#"{ values: ["@item:stick", "@tag:item:c:dusts", "@Strings.a.b", "@@x", "plain", "@Item:x"] }"#,
    )
    .unwrap();
    let t = |i: &str| at(&r, &[i, "t"]).as_str();
    assert_eq!(t("0"), Some("ref"));
    assert_eq!(at(&r, &["0", "id"]).as_str(), Some("minecraft:stick"));
    assert_eq!(t("1"), Some("tag"));
    assert_eq!(t("2"), Some("res"));
    assert_eq!(at(&r, &["3", "v"]).as_str(), Some("@x"));
    assert_eq!(at(&r, &["4", "v"]).as_str(), Some("plain"));
    assert!(at(&r, &["5", "error"]).as_str().is_some());
}

#[test]
fn data_is_read_patched_and_formatted_through_the_service() {
    let text = "{\n  // c\n  a: 1,\n  b: \"@item:x\"\n}";
    let parse = |t: &str| {
        answer(&Request {
            method: "data.parse".into(),
            params: Value::Map([("text", Value::from(t))].into_iter().collect()),
        })
    };
    let value = parse(text).unwrap();
    assert_eq!(value.get("b").and_then(Value::as_str), Some("@item:x"));
    let mut next = value.clone();
    if let Value::Map(m) = &mut next {
        m.insert("a", Value::Int(2));
    }
    let patched = answer(&Request {
        method: "data.patch".into(),
        params: Value::Map(
            [("text", Value::from(text)), ("next", next)]
                .into_iter()
                .collect(),
        ),
    })
    .unwrap();
    assert_eq!(
        patched.as_str(),
        Some(text.replace("a: 1", "a: 2").as_str())
    );
    assert_eq!(parse("{ a: }").unwrap_err().problems[0].line, Some(1));
    assert_eq!(
        call("data.format", "{ value: [1, 2] }").unwrap().as_str(),
        Some("[1, 2]")
    );
}

#[test]
fn a_request_is_named_as_the_tools_write_it() {
    use yog_ids::serve_request as R;
    assert_eq!(
        yog_data::schema::field_names(&<Request as yog_data::Schema>::shape()),
        [R::METHOD, R::PARAMS]
    );
}

#[test]
fn a_source_is_saved_binary_and_read_as_text() {
    let dir = std::env::temp_dir().join(format!("yog-serve-source-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("ruby.yblock");
    let path = Value::from(file.to_string_lossy().as_ref());
    let save = |text: &str| {
        answer(&Request {
            method: "source.write".into(),
            params: Value::Map(
                [("path", path.clone()), ("text", Value::from(text))]
                    .into_iter()
                    .collect(),
            ),
        })
    };
    save("{\n  // Hard.\n  hardness: 3\n}").unwrap();
    assert!(yog_data::source::is_binary(&std::fs::read(&file).unwrap()));
    let read = answer(&Request {
        method: "source.read".into(),
        params: Value::Map([("path", path.clone())].into_iter().collect()),
    })
    .unwrap();
    let text = read.as_str().unwrap();
    let back = yog_data::text::parse_value(text).unwrap();
    assert_eq!(back.get("$hardness").and_then(Value::as_str), Some("Hard."));
    assert_eq!(back.get("hardness"), Some(&Value::Int(3)));
    assert_eq!(save("{ a: }").unwrap_err().problems[0].line, Some(1));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_parameter_that_doesnt_read_says_where() {
    let e = call(
        "content_texture",
        r#"{ path: "/nowhere/a.yblock", texture: "@item:stick" }"#,
    )
    .unwrap_err();
    assert_eq!(e.problems.len(), 1);
    assert_eq!(e.problems[0].path, "texture");
    assert!(e.message.contains("texture"), "{e:?}");
}

/// What the editors ask with their files' values as written.
#[test]
fn textures_and_advancements_take_references_as_written() {
    use base64::Engine as _;
    let dir = std::env::temp_dir().join(format!("yog-serve-refs-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("content")).unwrap();
    std::fs::write(dir.join("yog.toml"), "[mod]\nid = \"mymod\"\n").unwrap();
    let file = dir.join("content/ruby.yblock");
    std::fs::write(&file, "{}").unwrap();
    let png = yog_media::image::encode_png(&yog_media::image::Image {
        width: 1,
        height: 1,
        rgba: vec![1, 2, 3, 255],
    })
    .unwrap();
    let p = file.to_string_lossy();
    let png = base64::engine::general_purpose::STANDARD.encode(&png);
    call(
        "content_save_texture",
        &format!(r#"{{ path: "{p}", texture: "@texture:mymod:block/ruby", png: "{png}" }}"#),
    )
    .unwrap();
    let texture = |t: &str| {
        call(
            "content_texture",
            &format!(r#"{{ path: "{p}", texture: "{t}" }}"#),
        )
    };
    assert!(texture("@texture:mymod:block/ruby").is_ok());
    // A bare id isn't a reference: said, at the parameter.
    let bare = texture("mymod:block/ruby").unwrap_err();
    assert_eq!(bare.problems[0].path, "texture");
    assert_eq!(
        bare.problems[0].code,
        yog_ids::problem_code::NOT_A_REFERENCE
    );
    let missing = texture("@texture:mymod:block/none").unwrap_err();
    assert_eq!(missing.problems[0].code, yog_ids::problem_code::NOT_FOUND);
    let args = format!(
        r#"{{ args: {{ path: "{p}", selected: "mymod:next", nodes: [
            {{ id: "mymod:root", parent: null, icon: "@item:minecraft:stick", frame: "task", title: "Root", description: "@lang:gui.done", background: "minecraft:textures/block/calcite.png" }},
            {{ id: "mymod:next", parent: "@advancement:mymod:root", icon: "@block:minecraft:stone", frame: null, title: null, description: null, background: null }}
        ], hover: null }} }}"#
    );
    let drawn = call("advancement_render", &args);
    assert!(drawn.is_ok(), "{drawn:?}");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_source_being_edited_is_checked_where_it_goes_wrong() {
    let check = |text: &str| {
        call(
            "source.check",
            &format!(
                "{{ path: \"/m/content/ruby.yblock\", text: {} }}",
                yog_data::text::to_string(&Value::from(text))
            ),
        )
        .unwrap()
    };
    let problems = |v: &Value| at(v, &["problems"]).as_list().unwrap().to_vec();
    let clean = problems(&check(
        "{\n  // Hard.\n  model: {},\n  block: { hardness: 3 }\n}",
    ));
    assert!(clean.is_empty(), "{clean:?}");
    let unknown = problems(&check("{\n  model: {},\n  shiny: true\n}"));
    assert_eq!(unknown.len(), 1, "{unknown:?}");
    assert_eq!(
        unknown[0].get("code").and_then(Value::as_str),
        Some(yog_ids::problem_code::UNKNOWN_FIELD)
    );
    assert_eq!(unknown[0].get("line"), Some(&Value::Int(3)));
    assert_eq!(unknown[0].get("col"), Some(&Value::Int(3)));
    let syntax = problems(&check("{ hardness: }"));
    assert_eq!(
        syntax[0].get("code").and_then(Value::as_str),
        Some(yog_ids::problem_code::SYNTAX)
    );
}
