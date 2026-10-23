//! The binary form: round trips, and malformed bytes as errors.

use bumpalo::Bump;

use super::*;
use crate::model::value::{Map, Ref, Res, Value};

fn sample() -> Value {
    let mut m = Map::new();
    m.insert("name", Value::Str("Ruby".into()));
    m.insert(
        "n",
        Value::List(vec![
            Value::Int(0),
            Value::Int(-1),
            Value::Int(i64::MAX),
            Value::Int(i64::MIN),
            Value::Float(0.5),
            Value::Null,
            Value::Bool(true),
            Value::Bool(false),
        ]),
    );
    m.insert("item", Value::Ref(Ref::new("item", "minecraft:stick")));
    m.insert("tag", Value::Tag(Ref::new("item", "minecraft:planks")));
    m.insert(
        "title",
        Value::Res(Res {
            set: "Strings".into(),
            key: "title".into(),
        }),
    );
    m.insert("text", Value::Text("{b|hi}".into()));
    m.insert("icon", Value::Image("sprite:x".into()));
    m.insert("color", Value::Color(0x80FF0000));
    m.insert("lang", Value::Lang("gui.done".into()));
    m.insert("again", Value::Str("Ruby".into()));
    Value::Map(m)
}

#[test]
fn bytes_round_trip_in_both_forms() {
    let v = Value::Bytes(vec![0, 1, 2, 255]);
    assert_eq!(decode(&encode(&v)).unwrap(), v);
    let text = crate::text::to_string(&v);
    assert_eq!(text, "\"@bytes:AAEC/w==\"");
    assert_eq!(crate::typed::classify("@bytes:AAEC/w==").unwrap(), v);
    assert!(
        decode(&[b'Y', b'O', b'G', b'D', VERSION, tag::BYTES, 9]).is_err(),
        "a length past the end"
    );
}

#[test]
fn round_trips_every_type() {
    let v = sample();
    let bytes = encode(&v);
    assert_eq!(decode(&bytes).unwrap(), v);
    // "Ruby" is stored once.
    assert_eq!(bytes.windows(4).filter(|w| w == b"Ruby").count(), 1);
}

#[test]
fn the_borrowed_decode_reads_the_same_without_copying() {
    let v = sample();
    let bytes = encode(&v);
    let arena = Bump::new();
    let r = decode_ref(&bytes, &arena).unwrap();
    assert_eq!(r.to_value(), v);
    assert_eq!(r, v.borrow(&arena), "borrowing a Value gives the same");
    // Its strings point into the bytes.
    let span = bytes.as_ptr_range();
    let mut strings = Vec::new();
    fn walk<'a>(v: &ValueRef<'a>, out: &mut Vec<&'a str>) {
        match v {
            ValueRef::Str(s) | ValueRef::Text(s) | ValueRef::Image(s) | ValueRef::Lang(s) => {
                out.push(s)
            }
            ValueRef::List(l) => l.iter().for_each(|x| walk(x, out)),
            ValueRef::Map(m) => m.iter().for_each(|(k, x)| {
                out.push(k);
                walk(x, out)
            }),
            _ => {}
        }
    }
    walk(&r, &mut strings);
    assert!(!strings.is_empty() && strings.iter().all(|s| span.contains(&s.as_ptr())));
    // Malformed bytes are errors here too.
    for len in 0..bytes.len() {
        assert!(
            decode_ref(&bytes[..len], &arena).is_err(),
            "truncated at {len}"
        );
    }
}

#[test]
fn malformed_bytes_are_errors() {
    let bytes = encode(&sample());
    // Every truncation fails cleanly.
    for len in 0..bytes.len() {
        assert!(decode(&bytes[..len]).is_err(), "truncated at {len}");
    }
    let mut extra = bytes.clone();
    extra.push(0);
    assert!(
        decode(&extra)
            .unwrap_err()
            .message
            .contains("after the value")
    );
    assert!(
        decode(b"YOGD\x01\x00")
            .unwrap_err()
            .message
            .contains("version 1")
    );
    assert!(
        decode(b"YOGD\x02\x63")
            .unwrap_err()
            .message
            .contains("unknown tag")
    );
    assert!(
        decode(b"YOGD\x02\x05\x00")
            .unwrap_err()
            .message
            .contains("isn't in the table")
    );
    // A huge count with nothing behind it doesn't allocate.
    assert!(
        decode(b"YOGD\x02\x06\xff\xff\xff\xff\x0f")
            .unwrap_err()
            .message
            .contains("past the end")
    );
    let deep: Vec<u8> = b"YOGD\x02"
        .iter()
        .copied()
        .chain(std::iter::repeat_n([6u8, 1], 300).flatten())
        .chain([0])
        .collect();
    assert!(decode(&deep).unwrap_err().message.contains("nested deeper"));
}
