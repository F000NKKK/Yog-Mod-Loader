//! The service as a tool calls it: requests of plain values, plain answers.

use yog_data::Value;

use super::{Request, answer};

/// `method` called with `params` (Yog Data text).
fn call(method: &str, params: &str) -> Result<Value, Value> {
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
    assert_eq!(at(&parse("{ a: }").unwrap_err(), &["line"]), &Value::Int(1));
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
