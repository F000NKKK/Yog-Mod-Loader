//! `#[derive(Schema)]` on a small book-like format: reading, problems with
//! their line and path, references, writing back, the binary round trip and
//! the shape.

use yog_data::schema::Found;
use yog_data::{
    Color, Id, Image, ItemIdOrTag, Ref, Res, Schema, TagId, Text, Value, binary, from_text,
    from_value, kinds, text,
};

/// How the result is laid out.
#[derive(Schema, Debug, Clone, PartialEq)]
enum Mode {
    Shaped,
    Shapeless,
    #[data(rename = "smelt")]
    Smelting,
}

/// A recipe of the book.
#[derive(Schema, Debug, Clone, PartialEq)]
struct Recipe {
    /// What it makes.
    result: Id<kinds::Item>,
    #[data(default = "one")]
    count: u32,
    mode: Mode,
    ingredients: Vec<ItemIdOrTag>,
}

fn one() -> u32 {
    1
}

#[derive(Schema, Debug, Clone, PartialEq)]
struct Spotlight {
    item: Id<kinds::Item>,
    #[data(default)]
    text: Option<Text>,
}

/// A page.
#[derive(Schema, Debug, Clone, PartialEq)]
#[data(tag = "type")]
enum Page {
    /// Text, wrapped.
    Text {
        text: Text,
        title: Option<Text>,
    },
    Spotlight(Spotlight),
    Recipe(Recipe),
    Empty,
}

#[derive(Schema, Debug, Clone, PartialEq)]
struct Entry {
    name: Text,
    icon: Image,
    #[data(rename = "color")]
    tint: Option<Color>,
    pages: Vec<Page>,
    #[data(skip)]
    read_count: u32,
    pos: [i32; 2],
}

const BOOK: &str = r##"{
  // An entry.
  name: "@Strings.entry.title",
  icon: "@image:item:minecraft:book",
  color: "#8d6acc",
  pos: [0, -2],
  pages: [
    { type: "text", text: "@@everyone {b|Hello}" },
    { type: "spotlight", item: "@item:minecraft:stick", text: "@item:minecraft:stick" },
    { type: "recipe", result: "@item:gm:ruby", mode: "smelt", ingredients: ["@tag:item:minecraft:planks", "@item:stick"], $note: "meta" },
    { type: "empty" },
  ],
}"##;

#[test]
fn reads_a_document_and_its_references() {
    let read = from_text::<Entry>(BOOK).unwrap_or_else(|e| panic!("{e}"));
    let e = read.value;
    assert_eq!(
        e.name,
        Text::Res(Res {
            set: "Strings".into(),
            key: "entry.title".into()
        })
    );
    assert_eq!(e.icon, Image::Source("item:minecraft:book".into()));
    assert_eq!(e.tint, Some(Color::Argb(0xFF8D6ACC)));
    assert_eq!(e.pos, [0, -2]);
    assert_eq!(
        e.pages[0],
        Page::Text {
            text: Text::Literal("@everyone {b|Hello}".into()),
            title: None
        }
    );
    assert_eq!(
        e.pages[1],
        Page::Spotlight(Spotlight {
            item: Id::new("minecraft:stick"),
            text: Some(Text::NameOf(Ref::new("item", "minecraft:stick")))
        })
    );
    let Page::Recipe(r) = &e.pages[2] else {
        panic!()
    };
    assert_eq!((r.count, &r.mode), (1, &Mode::Smelting));
    assert_eq!(
        r.ingredients,
        [
            ItemIdOrTag::Tag(TagId::new("minecraft:planks")),
            ItemIdOrTag::One(Id::new("minecraft:stick"))
        ]
    );
    assert_eq!(e.pages[3], Page::Empty);

    let refs: Vec<String> = read
        .refs
        .iter()
        .map(|Found { value, path }| format!("{path} {}", value.id))
        .collect();
    assert_eq!(
        refs,
        [
            "pages[1].item minecraft:stick",
            "pages[1].text minecraft:stick",
            "pages[2].result gm:ruby",
            "pages[2].ingredients[1] minecraft:stick"
        ]
    );
    assert_eq!(read.tags[0].path.to_string(), "pages[2].ingredients[0]");
    assert_eq!(read.resources[0].value.key, "entry.title");
}

#[test]
fn every_problem_is_reported_with_its_place() {
    let text = r##"{
  name: 5,
  icon: "@item:minecraft:book",
  pos: [1],
  colr: "#fff",
  pages: [
    { type: "txt", text: "x" },
    { type: "spotlight", item: "@block:minecraft:stone", txt: "y" },
    { type: "recipe", result: "@item:Gm:Ruby", count: -1, mode: "boil", ingredients: ["@tag:block:minecraft:logs"] },
    { text: "no type" },
  ],
}"##;
    let errors = from_text::<Entry>(text).unwrap_err().to_string();
    let expected = [
        "2:9: name: expected a text, found 5",
        "3:9: icon: expected an image, found a reference",
        "4:8: pos: expected 2 items, found 1",
        "5:3: colr: unknown field `colr` — did you mean `color`?",
        "7:13: pages[0].type: unknown `txt` — did you mean `text`?",
        "8:32: pages[1].item: `@block:minecraft:stone` is a reference to block — this takes `@item:`",
        "8:58: pages[1].txt: unknown field `txt` — did you mean `text`?",
        "9:31: pages[2].result: `@item:Gm:Ruby`: `Gm:Ruby` isn't an id (lower case `namespace:path`)",
        "9:55: pages[2].count: -1 is out of range (0..=4294967295)",
        "9:65: pages[2].mode: unknown `boil` (expected `shaped`, `shapeless`, `smelt`)",
        "9:87: pages[2].ingredients[0]: expected a tag of item, found a tag of block",
        "10:5: pages[3]: missing `type` (`text`, `spotlight`, `recipe`, `empty`)",
    ];
    assert_eq!(errors.lines().collect::<Vec<_>>(), expected, "\n{errors}");
}

#[test]
fn missing_fields_and_syntax_errors() {
    let e = from_text::<Spotlight>("{ text: \"x\" }")
        .unwrap_err()
        .to_string();
    assert_eq!(e, "1:1: missing `item`");
    let e = from_text::<Spotlight>("{ item: \"x\",, }")
        .unwrap_err()
        .to_string();
    assert_eq!(e, "1:13: expected a key");
}

#[test]
fn writes_back_what_it_read() {
    let entry = from_text::<Entry>(BOOK).unwrap().value;
    let written = entry.write();
    let again = from_value::<Entry>(&written)
        .unwrap_or_else(|p| panic!("{p:?}"))
        .value;
    assert_eq!(again, entry);
    // Through the text form...
    let txt = text::to_string_pretty(&written);
    assert!(
        txt.contains("\"type\": \"spotlight\",\n      \"item\": \"@item:minecraft:stick\""),
        "{txt}"
    );
    assert!(txt.contains("\"@@everyone {b|Hello}\""), "{txt}");
    assert!(!txt.contains("read_count"));
    assert_eq!(from_text::<Entry>(&txt).unwrap().value, entry);
    // ...and the binary one, which keeps the types.
    let bytes = binary::encode(&written);
    let decoded = binary::decode(&bytes).unwrap();
    assert!(matches!(decoded.get("name"), Some(Value::Res(_))));
    assert_eq!(from_value::<Entry>(&decoded).unwrap().value, entry);
}

#[test]
fn shapes_describe_the_format() {
    let d = Entry::shape().describe();
    let defs = d.get("defs").unwrap();
    let page = defs.get("Page").unwrap();
    assert_eq!(page.get("tag"), Some(&Value::from("type")));
    let variants = page.get("variants").unwrap().as_list().unwrap();
    assert_eq!(
        variants
            .iter()
            .map(|v| v.get("name").unwrap().as_str().unwrap())
            .collect::<Vec<_>>(),
        ["text", "spotlight", "recipe", "empty"]
    );
    assert_eq!(variants[0].get("doc"), Some(&Value::from("Text, wrapped.")));
    // A wrapped struct's fields are the variant's.
    let spot_fields = variants[1].get("fields").unwrap().as_list().unwrap();
    assert_eq!(
        spot_fields[0].get("shape").unwrap().get("of"),
        Some(&Value::List(vec![Value::from("item")]))
    );
    assert_eq!(spot_fields[1].get("required"), Some(&Value::Bool(false)));
    let fields = variants[2].get("fields").unwrap().as_list().unwrap();
    assert_eq!(fields[0].get("doc"), Some(&Value::from("What it makes.")));
    assert_eq!(fields[1].get("required"), Some(&Value::Bool(false)));
    assert_eq!(
        fields[3]
            .get("shape")
            .unwrap()
            .get("of")
            .unwrap()
            .get("kind"),
        Some(&Value::from("ref_or_tag"))
    );
}

/// A plain-string model read through codecs, references and texts resolved.
#[derive(Schema, Debug, PartialEq)]
struct Plainly {
    #[data(with = "yog_data::with::RefTo<(kinds::Item, kinds::Block)>")]
    icon: String,
    #[data(with = "yog_data::with::Opt<yog_data::with::TextOf>")]
    title: Option<String>,
    #[data(
        with = "yog_data::with::List<yog_data::with::RefTo<kinds::Entry>>",
        default
    )]
    see: Vec<String>,
}

struct Game;

impl yog_data::Resolve for Game {
    fn reference(&mut self, r: &Ref, _: &yog_data::Path) -> Result<String, String> {
        if r.id.ends_with("nope") {
            Err(format!("no {} {}", r.kind, r.id))
        } else {
            Ok(r.id.clone())
        }
    }
    fn text(&mut self, t: &Text, _: &yog_data::Path) -> Result<String, String> {
        Ok(match t {
            Text::Res(r) => format!("gm.{}", r.key),
            Text::NameOf(r) => format!("{}.{}", r.kind, r.id.replace(':', ".")),
            Text::Literal(s) => s.clone(),
            Text::Lang(k) => k.clone(),
        })
    }
}

#[test]
fn codecs_keep_plain_types_and_resolve() {
    let v = text::parse_value(
        r#"{ icon: "@block:minecraft:stone", title: "@Strings.title", see: ["@entry:gm:a"] }"#,
    )
    .unwrap();
    let read = yog_data::from_value_with::<Plainly>(&v, &mut Game).unwrap();
    assert_eq!(
        read.value,
        Plainly {
            icon: "minecraft:stone".into(),
            title: Some("gm.title".into()),
            see: vec!["gm:a".into()]
        }
    );
    let v = text::parse_value(r#"{ icon: "minecraft:stone", title: "@item:minecraft:stick", see: ["@entry:gm:nope", "@item:x"] }"#).unwrap();
    let problems: Vec<String> = yog_data::from_value_with::<Plainly>(&v, &mut Game)
        .unwrap_err()
        .into_iter()
        .map(|p| format!("{}: {}", p.path, p.message))
        .collect();
    assert_eq!(
        problems,
        [
            "icon: `minecraft:stone`: name it by a reference — `@item:minecraft:stone`",
            "see[0]: no entry gm:nope",
            "see[1]: `@item:minecraft:x` is a reference to item — this takes `@entry:`",
        ]
    );
    let shape = Plainly::shape().describe();
    let fields = shape
        .get("defs")
        .unwrap()
        .get("Plainly")
        .unwrap()
        .get("fields")
        .unwrap()
        .as_list()
        .unwrap();
    assert_eq!(
        fields[0].get("shape").unwrap().get("of"),
        Some(&Value::List(vec![
            Value::from("item"),
            Value::from("block")
        ]))
    );
    assert_eq!(fields[2].get("required"), Some(&Value::Bool(false)));
}

#[derive(Schema, Debug, PartialEq)]
struct Ranged {
    #[data(range = "1..=99")]
    stack: u32,
    #[data(range = "-16..=32")]
    from: Option<[f64; 3]>,
}

#[test]
fn ranges_are_checked_and_described() {
    assert_eq!(
        from_text::<Ranged>("{ stack: 64, from: [0, 8, 32] }")
            .unwrap()
            .value,
        Ranged {
            stack: 64,
            from: Some([0.0, 8.0, 32.0])
        }
    );
    let e = from_text::<Ranged>("{ stack: 100, from: [0, 40.5, 0] }")
        .unwrap_err()
        .to_string();
    assert_eq!(
        e.lines().collect::<Vec<_>>(),
        [
            "1:10: stack: 100 is out of range (1..=99)",
            "1:21: from: 40.5 is out of range (-16..=32)"
        ]
    );
    let shape = Ranged::shape().describe();
    let fields = shape
        .get("defs")
        .unwrap()
        .get("Ranged")
        .unwrap()
        .get("fields")
        .unwrap()
        .as_list()
        .unwrap();
    assert_eq!(
        fields[0].get("range"),
        Some(&Value::List(vec![Value::Float(1.0), Value::Float(99.0)]))
    );
}

#[derive(Schema, Debug, PartialEq)]
struct Blob {
    #[data(with = "yog_data::with::Raw")]
    bytes: Vec<u8>,
}

#[test]
fn raw_bytes_round_trip_through_text() {
    let blob = Blob { bytes: vec![0, 1, 2, 255] };
    let text = yog_data::to_text(&blob);
    assert!(text.contains("@bytes:"), "{text}");
    assert_eq!(yog_data::from_text::<Blob>(&text).unwrap().value, blob);
}

/// What the bus carries: written and read straight, without a value tree.
#[derive(Schema, Debug, Clone, PartialEq)]
struct Message {
    name: String,
    count: u32,
    ratio: f32,
    tags: Vec<String>,
    mode: Mode,
    note: Option<String>,
    #[data(default)]
    extra: i64,
    #[data(with = "yog_data::with::Raw")]
    bytes: Vec<u8>,
}

#[test]
fn messages_go_straight_and_as_the_value_would() {
    let m = Message {
        name: "ruby".into(),
        count: 3,
        ratio: 0.5,
        tags: vec!["a".into(), "b".into(), "a".into()],
        mode: Mode::Smelting,
        note: None,
        extra: -7,
        bytes: vec![1, 2, 3],
    };
    let straight = yog_data::to_bytes(&m);
    // The same value either way; straight is never longer (a `f32` takes four bytes, not eight).
    let via_value = binary::encode(&m.write());
    assert_eq!(binary::decode(&straight).unwrap(), binary::decode(&via_value).unwrap());
    assert!(straight.len() < via_value.len());
    let mut d = binary::Decoder::open(&straight).unwrap();
    assert_eq!(Message::decode(&mut d).as_ref(), Some(&m), "read straight, not the slow way");
    assert!(d.at_end());
    assert_eq!(yog_data::read_bytes::<Message>(&straight).unwrap(), m);
    // A field left out takes its default; an unknown one sends it the slow way, which says so.
    let mut fewer = m.write();
    if let Value::Map(map) = &mut fewer {
        map.remove("extra");
        map.remove("note");
    }
    let fewer = binary::encode(&fewer);
    let mut d = binary::Decoder::open(&fewer).unwrap();
    assert_eq!(Message::decode(&mut d).map(|r| r.extra), Some(0));
    let mut more = m.write();
    if let Value::Map(map) = &mut more {
        map.insert("nope", Value::Int(1));
    }
    let bytes = binary::encode(&more);
    assert!(Message::decode(&mut binary::Decoder::open(&bytes).unwrap()).is_none());
    assert!(yog_data::read_bytes::<Message>(&bytes).unwrap_err().iter().any(|p| p.message.contains("nope")));
}

/// A big list kept written: items as they come, read when asked.
#[derive(Schema, Debug, Clone, PartialEq)]
struct Area {
    name: String,
    palette: Vec<String>,
    blocks: yog_data::Stream<Placed>,
    after: String,
}

#[derive(Schema, Debug, Clone, PartialEq)]
struct Placed {
    id: String,
    count: u32,
}

#[test]
fn streams_go_whole_and_read_lazily() {
    let blocks: yog_data::Stream<Placed> = (0..500).map(|i| Placed { id: format!("minecraft:{}", ["stone", "dirt"][i % 2]), count: i as u32 }).collect();
    // Strings the rest of the document has too: the stream's are its own.
    let area = Area { name: "minecraft:stone".into(), palette: vec!["minecraft:dirt".into()], blocks, after: "minecraft:stone".into() };
    let bytes = yog_data::to_bytes(&area);
    let back: Area = yog_data::read_bytes(&bytes).unwrap();
    assert_eq!(back, area);
    assert_eq!(back.blocks.iter().nth(3), Some(Placed { id: "minecraft:dirt".into(), count: 3 }));
    // The value way reads it as a list; the text way writes and reads it too.
    let value = binary::decode(&bytes).unwrap();
    assert_eq!(value.get("blocks").and_then(|b| b.as_list()).map(<[Value]>::len), Some(500));
    let text = yog_data::to_text(&area);
    assert_eq!(yog_data::from_text::<Area>(&text).unwrap().value, area);
    // Cut short anywhere, it's an error, never a panic.
    for len in 0..bytes.len() {
        assert!(yog_data::read_bytes::<Area>(&bytes[..len]).is_err(), "truncated at {len}");
    }
}

/// Compact owned types (no spare capacity) and 32-bit floats, written straight.
#[derive(Schema, Debug, Clone, PartialEq)]
struct Compact {
    name: Box<str>,
    shared: std::sync::Arc<str>,
    ids: Box<[u32]>,
    weights: Vec<f32>,
    scale: f32,
}

#[test]
fn compact_types_round_trip() {
    let c = Compact { name: "a".into(), shared: "b".into(), ids: vec![1, 2].into_boxed_slice(), weights: vec![0.5, 1.25], scale: 2.5 };
    let bytes = yog_data::to_bytes(&c);
    assert_eq!(yog_data::read_bytes::<Compact>(&bytes).unwrap(), c);
    assert_eq!(yog_data::from_text::<Compact>(&yog_data::to_text(&c)).unwrap().value, c);
}

/// Frames: packed by position for the bus.
mod frames {
    use yog_data::Schema;
    use yog_data::frame::{read_frame, to_frame};

    use super::{Area, Compact, Message, Mode, Placed};

    #[test]
    fn every_kind_of_field_round_trips() {
        let m = Message {
            name: "ruby".into(),
            count: 3,
            ratio: 0.5,
            tags: vec!["a".into(), "b".into(), "a".into()],
            mode: Mode::Smelting,
            note: Some("n".into()),
            extra: -7,
            bytes: vec![1, 2, 3],
        };
        let frame = to_frame(&m);
        assert_eq!(read_frame::<Message>(&frame), Some(m.clone()));
        // Smaller than the self-describing form: no keys, no tags.
        assert!(frame.len() < yog_data::to_bytes(&m).len(), "{} vs {}", frame.len(), yog_data::to_bytes(&m).len());
        let c = Compact { name: "a".into(), shared: "a".into(), ids: vec![1, 2].into_boxed_slice(), weights: vec![0.5, 1.25], scale: 2.5 };
        assert_eq!(read_frame::<Compact>(&to_frame(&c)), Some(c));
        // A stream goes as its binary form inside the frame.
        let blocks: yog_data::Stream<Placed> = (0..10).map(|i| Placed { id: "minecraft:stone".into(), count: i }).collect();
        let area = Area { name: "pit".into(), palette: vec!["minecraft:stone".into()], blocks, after: "x".into() };
        assert_eq!(read_frame::<Area>(&to_frame(&area)), Some(area));
    }

    #[test]
    fn repeated_strings_are_stored_once() {
        let tags: Vec<String> = (0..100).map(|_| "minecraft:stone".to_owned()).collect();
        let frame = to_frame(&tags);
        assert_eq!(frame.windows(15).filter(|w| w == b"minecraft:stone").count(), 1);
        assert_eq!(read_frame::<Vec<String>>(&frame), Some(tags));
    }

    #[derive(Schema, Debug, PartialEq)]
    struct Old {
        a: u32,
        b: String,
    }

    #[derive(Schema, Debug, PartialEq)]
    struct New {
        a: u32,
        b: String,
        #[data(default)]
        c: Vec<String>,
        d: Option<f64>,
    }

    #[derive(Schema, Debug, PartialEq)]
    struct Holds<T: Schema> {
        inner: T,
        after: String,
    }

    #[test]
    fn fields_are_appended_and_read_both_ways() {
        // A newer writer's fields (with strings of their own) are skipped by an older reader…
        let new = Holds { inner: New { a: 1, b: "x".into(), c: vec!["only new".into()], d: Some(2.0) }, after: "y".into() };
        let old: Holds<Old> = read_frame(&to_frame(&new)).unwrap();
        assert_eq!(old, Holds { inner: Old { a: 1, b: "x".into() }, after: "y".into() });
        // …and an older writer's record takes a newer reader's defaults.
        let back: Holds<New> = read_frame(&to_frame(&old)).unwrap();
        assert_eq!(back.inner, New { a: 1, b: "x".into(), c: Vec::new(), d: None });
    }

    #[test]
    fn numbers_pack_tight() {
        // Palette indices 0..=2: two bits each.
        let blocks: Vec<u16> = (0..32768).map(|i| (i % 3) as u16).collect();
        let frame = to_frame(&blocks);
        assert!(frame.len() < 32768 / 4 + 16, "{} bytes", frame.len());
        assert_eq!(read_frame::<Vec<u16>>(&frame), Some(blocks));
        let signed = vec![0i32, -1, 1, i32::MIN, i32::MAX];
        assert_eq!(read_frame::<Vec<i32>>(&to_frame(&signed)), Some(signed));
        let wide = vec![u64::MAX, 0, 1 << 63];
        assert_eq!(read_frame::<Vec<u64>>(&to_frame(&wide)), Some(wide));
        let zeros = vec![0u8; 100];
        assert_eq!(read_frame::<Vec<u8>>(&to_frame(&zeros)), Some(zeros));
        // Floats that are `f32`s take four bytes; one that isn't keeps them all eight.
        let halves: Vec<f64> = (0..1000).map(|i| f64::from(i) * 0.5).collect();
        assert!(to_frame(&halves).len() < 1000 * 4 + 16);
        assert_eq!(read_frame::<Vec<f64>>(&to_frame(&halves)), Some(halves));
        let exact = vec![0.1f64, 2.0];
        assert_eq!(read_frame::<Vec<f64>>(&to_frame(&exact)), Some(exact));
    }

    /// What a tool (and the Java codecs) read by the shape alone is the value itself.
    #[test]
    fn a_frame_reads_by_its_shape_alone() {
        use yog_data::frame::read_shaped;
        let m = Message {
            name: "ruby".into(),
            count: 3,
            ratio: 0.5,
            tags: vec!["a".into(), "b".into()],
            mode: Mode::Smelting,
            note: None,
            extra: -7,
            bytes: vec![1, 2, 3],
        };
        let value = read_shaped(&to_frame(&m), &Message::shape()).unwrap();
        assert_eq!(yog_data::read_value::<Message>(&value).unwrap(), m);
        let blocks: yog_data::Stream<Placed> = (0..3).map(|i| Placed { id: "minecraft:stone".into(), count: i }).collect();
        let area = Area { name: "pit".into(), palette: vec!["minecraft:stone".into()], blocks, after: "x".into() };
        let value = read_shaped(&to_frame(&area), &Area::shape()).unwrap();
        assert_eq!(yog_data::read_value::<Area>(&value).unwrap(), area);
        let c = Compact { name: "a".into(), shared: "a".into(), ids: vec![1, 2].into_boxed_slice(), weights: vec![0.5, 1.25], scale: 2.5 };
        let value = read_shaped(&to_frame(&c), &Compact::shape()).unwrap();
        assert_eq!(yog_data::read_value::<Compact>(&value).unwrap(), c);
    }

    #[test]
    fn malformed_frames_are_none() {
        let frame = to_frame(&Holds { inner: New { a: 1, b: "x".into(), c: vec!["c".into()], d: None }, after: "y".into() });
        for len in 0..frame.len() {
            assert!(read_frame::<Holds<New>>(&frame[..len]).is_none(), "cut at {len}");
        }
        // A count bigger than the bytes left is refused before anything is allocated.
        let mut p = yog_data::frame::Packer::new();
        p.put_u(u64::MAX >> 1);
        assert!(read_frame::<Vec<u32>>(&p.finish()).is_none());
    }
}

/// Views: a frame read without copying.
mod views {
    use yog_data::Schema;
    use yog_data::frame::{read_view, to_frame};

    #[derive(Schema, Debug, PartialEq)]
    #[data(view)]
    struct Hit {
        id: String,
        count: u32,
        weights: Vec<f64>,
        blocks: Vec<i16>,
        tags: Vec<String>,
        inner: Option<Inner>,
    }

    #[derive(Schema, Debug, PartialEq)]
    #[data(view)]
    struct Inner {
        name: String,
    }

    #[test]
    fn a_view_borrows_and_reads_in_place() {
        let hit = Hit {
            id: "minecraft:stone".into(),
            count: 7,
            weights: vec![0.5, 0.25],
            blocks: vec![-3, 0, 900],
            tags: vec!["a".into(), "b".into()],
            inner: Some(Inner { name: "in".into() }),
        };
        let frame = to_frame(&hit);
        let v = read_view::<Hit>(&frame).unwrap();
        assert_eq!(v.id, "minecraft:stone");
        // The string points into the frame: nothing copied.
        assert!(frame.as_ptr_range().contains(&v.id.as_ptr()));
        assert_eq!(v.count, 7);
        assert_eq!(v.weights.iter().collect::<Vec<f64>>(), [0.5, 0.25]);
        assert_eq!(v.blocks.iter().collect::<Vec<i16>>(), [-3, 0, 900]);
        assert_eq!(v.blocks.get(2), Some(900));
        assert_eq!(v.blocks.get(3), None);
        assert_eq!(v.tags, ["a", "b"]);
        assert_eq!(v.inner.map(|i| i.name), Some("in"));
    }
}
