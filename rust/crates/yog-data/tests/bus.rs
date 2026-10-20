//! Yog Data as the bus carries it: written and read straight (no value
//! tree), streams, compact owned types, frames packed by position, views.

use yog_data::{Schema, Value, binary};

/// How a result is laid out (a unit enum: a frame packs it by index).
#[derive(Schema, Debug, Clone, PartialEq)]
enum Mode {
    Shaped,
    Shapeless,
    #[data(rename = "smelt")]
    Smelting,
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
    assert_eq!(
        binary::decode(&straight).unwrap(),
        binary::decode(&via_value).unwrap()
    );
    assert!(straight.len() < via_value.len());
    let mut d = binary::Decoder::open(&straight).unwrap();
    assert_eq!(
        Message::decode(&mut d).as_ref(),
        Some(&m),
        "read straight, not the slow way"
    );
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
    assert!(
        yog_data::read_bytes::<Message>(&bytes)
            .unwrap_err()
            .iter()
            .any(|p| p.message.contains("nope"))
    );
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
    let blocks: yog_data::Stream<Placed> = (0..500)
        .map(|i| Placed {
            id: format!("minecraft:{}", ["stone", "dirt"][i % 2]),
            count: i as u32,
        })
        .collect();
    // Strings the rest of the document has too: the stream's are its own.
    let area = Area {
        name: "minecraft:stone".into(),
        palette: vec!["minecraft:dirt".into()],
        blocks,
        after: "minecraft:stone".into(),
    };
    let bytes = yog_data::to_bytes(&area);
    let back: Area = yog_data::read_bytes(&bytes).unwrap();
    assert_eq!(back, area);
    assert_eq!(
        back.blocks.iter().nth(3),
        Some(Placed {
            id: "minecraft:dirt".into(),
            count: 3
        })
    );
    // The value way reads it as a list; the text way writes and reads it too.
    let value = binary::decode(&bytes).unwrap();
    assert_eq!(
        value
            .get("blocks")
            .and_then(|b| b.as_list())
            .map(<[Value]>::len),
        Some(500)
    );
    let text = yog_data::to_text(&area);
    assert_eq!(yog_data::from_text::<Area>(&text).unwrap().value, area);
    // Cut short anywhere, it's an error, never a panic.
    for len in 0..bytes.len() {
        assert!(
            yog_data::read_bytes::<Area>(&bytes[..len]).is_err(),
            "truncated at {len}"
        );
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
    let c = Compact {
        name: "a".into(),
        shared: "b".into(),
        ids: vec![1, 2].into_boxed_slice(),
        weights: vec![0.5, 1.25],
        scale: 2.5,
    };
    let bytes = yog_data::to_bytes(&c);
    assert_eq!(yog_data::read_bytes::<Compact>(&bytes).unwrap(), c);
    assert_eq!(
        yog_data::from_text::<Compact>(&yog_data::to_text(&c))
            .unwrap()
            .value,
        c
    );
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
        assert!(
            frame.len() < yog_data::to_bytes(&m).len(),
            "{} vs {}",
            frame.len(),
            yog_data::to_bytes(&m).len()
        );
        let c = Compact {
            name: "a".into(),
            shared: "a".into(),
            ids: vec![1, 2].into_boxed_slice(),
            weights: vec![0.5, 1.25],
            scale: 2.5,
        };
        assert_eq!(read_frame::<Compact>(&to_frame(&c)), Some(c));
        // A stream goes as its binary form inside the frame.
        let blocks: yog_data::Stream<Placed> = (0..10)
            .map(|i| Placed {
                id: "minecraft:stone".into(),
                count: i,
            })
            .collect();
        let area = Area {
            name: "pit".into(),
            palette: vec!["minecraft:stone".into()],
            blocks,
            after: "x".into(),
        };
        assert_eq!(read_frame::<Area>(&to_frame(&area)), Some(area));
    }

    /// A frame packed in the thread's reused buffers is the same frame — nested too.
    #[test]
    fn reused_buffers_make_the_same_frames() {
        use yog_data::frame::with_frame;
        let a = vec!["x".to_owned(), "y".to_owned(), "x".to_owned()];
        let b = vec![1u32, 2, 3];
        for _ in 0..3 {
            with_frame(&a, |fa| {
                assert_eq!(fa, to_frame(&a).as_slice());
                with_frame(&b, |fb| assert_eq!(fb, to_frame(&b).as_slice()));
            });
        }
    }

    #[test]
    fn repeated_strings_are_stored_once() {
        let tags: Vec<String> = (0..100).map(|_| "minecraft:stone".to_owned()).collect();
        let frame = to_frame(&tags);
        assert_eq!(
            frame
                .windows(15)
                .filter(|w| w == b"minecraft:stone")
                .count(),
            1
        );
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
        let new = Holds {
            inner: New {
                a: 1,
                b: "x".into(),
                c: vec!["only new".into()],
                d: Some(2.0),
            },
            after: "y".into(),
        };
        let old: Holds<Old> = read_frame(&to_frame(&new)).unwrap();
        assert_eq!(
            old,
            Holds {
                inner: Old {
                    a: 1,
                    b: "x".into()
                },
                after: "y".into()
            }
        );
        // …and an older writer's record takes a newer reader's defaults.
        let back: Holds<New> = read_frame(&to_frame(&old)).unwrap();
        assert_eq!(
            back.inner,
            New {
                a: 1,
                b: "x".into(),
                c: Vec::new(),
                d: None
            }
        );
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
        let blocks: yog_data::Stream<Placed> = (0..3)
            .map(|i| Placed {
                id: "minecraft:stone".into(),
                count: i,
            })
            .collect();
        let area = Area {
            name: "pit".into(),
            palette: vec!["minecraft:stone".into()],
            blocks,
            after: "x".into(),
        };
        let value = read_shaped(&to_frame(&area), &Area::shape()).unwrap();
        assert_eq!(yog_data::read_value::<Area>(&value).unwrap(), area);
        let c = Compact {
            name: "a".into(),
            shared: "a".into(),
            ids: vec![1, 2].into_boxed_slice(),
            weights: vec![0.5, 1.25],
            scale: 2.5,
        };
        let value = read_shaped(&to_frame(&c), &Compact::shape()).unwrap();
        assert_eq!(yog_data::read_value::<Compact>(&value).unwrap(), c);
    }

    #[test]
    fn malformed_frames_are_none() {
        let frame = to_frame(&Holds {
            inner: New {
                a: 1,
                b: "x".into(),
                c: vec!["c".into()],
                d: None,
            },
            after: "y".into(),
        });
        for len in 0..frame.len() {
            assert!(
                read_frame::<Holds<New>>(&frame[..len]).is_none(),
                "cut at {len}"
            );
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
    use yog_data::frame::{read_frame, read_view, to_frame};

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
        // A list read lazily, item by item.
        assert_eq!(v.tags.iter().collect::<Vec<&str>>(), ["a", "b"]);
        assert_eq!(
            (v.tags.len(), v.tags.get(1), v.tags.get(2)),
            (2, Some("b"), None)
        );
        assert_eq!(v.inner.map(|i| i.name), Some("in"));
    }

    #[derive(Schema, Debug, PartialEq)]
    #[data(view)]
    struct Shelf {
        rows: Vec<Vec<Inner>>,
        after: u32,
    }

    #[test]
    fn lists_are_stepped_over_and_read_lazily() {
        let shelf = Shelf {
            rows: vec![
                vec![Inner { name: "a".into() }, Inner { name: "b".into() }],
                vec![],
                vec![Inner { name: "c".into() }],
            ],
            after: 42,
        };
        let frame = to_frame(&shelf);
        let v = read_view::<Shelf>(&frame).unwrap();
        // The field after the lists is reached without reading them.
        assert_eq!(v.after, 42);
        assert_eq!(v.rows.len(), 3);
        let names: Vec<Vec<&str>> = v
            .rows
            .iter()
            .map(|row| row.iter().map(|i| i.name).collect())
            .collect();
        assert_eq!(names, [vec!["a", "b"], vec![], vec!["c"]]);
        assert_eq!(
            v.rows.get(2).and_then(|r| r.get(0)).map(|i| i.name),
            Some("c")
        );
        assert!(v.rows.get(3).is_none());
        assert_eq!(read_frame::<Shelf>(&frame), Some(shelf));
    }
}
