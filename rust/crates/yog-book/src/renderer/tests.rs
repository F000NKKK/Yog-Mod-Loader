//! The renderer's tests: flowing, recipes read and laid out, pages drawn.

use super::*;
use yog_formats::book::BookPage;

use flow::content_above_text;
use geometry::{BODY_H, PAGE_W, TEXT_W};
use spread::landing_fit;

/// Text that doesn't fit its page goes on to new text pages after it.
fn flow(book: Book) -> Book {
    flow_traced(book).0
}

fn j(json: &str) -> yog_data::Value {
    yog_data::json::from_json_text(json).unwrap()
}
use yog_formats::book::BookEntry;

fn book(pages: Vec<BookPage>) -> Book {
    let mut book = Book {
        id: "t:b".into(),
        name: "B".into(),
        ..Default::default()
    };
    book.entries.push(BookEntry {
        id: "e".into(),
        name: "E".into(),
        pages,
        ..Default::default()
    });
    book
}

#[test]
fn long_text_flows_onto_new_pages() {
    let long = "word ".repeat(400);
    let flowed = flow(book(vec![
        BookPage::Text {
            text: long.clone(),
            title: None,
            font: None,
            font_size: None,
        },
        BookPage::Empty,
    ]));
    let pages = &flowed.entries[0].pages;
    assert!(pages.len() > 3, "{}", pages.len());
    assert!(
        matches!(pages.last(), Some(BookPage::Empty)),
        "what came after stays after"
    );
    let line_h = yog_ui::text::LINE_H + yog_ui::text::LINE_GAP;
    for p in &pages[..pages.len() - 1] {
        let BookPage::Text { text, .. } = p else {
            panic!("{p:?}")
        };
        assert!(text.lines().count() as f32 * line_h <= BODY_H + 1.0);
        assert!(
            text.lines()
                .all(|l| yog_ui::text::str_width(l, 1.0) <= TEXT_W)
        );
    }
    let words: usize = pages
        .iter()
        .map(|p| match p {
            BookPage::Text { text, .. } => text.split_whitespace().count(),
            _ => 0,
        })
        .sum();
    assert_eq!(words, 400, "nothing lost");
}

#[test]
fn edits_keep_the_reader_where_they_were() {
    let mut b = book(vec![
        BookPage::Text {
            text: "one".into(),
            title: None,
            font: None,
            font_size: None,
        },
        BookPage::Text {
            text: "two".into(),
            title: None,
            font: None,
            font_size: None,
        },
    ]);
    b.categories.push(yog_formats::book::BookCategory {
        id: "c".into(),
        name: "C".into(),
        ..Default::default()
    });
    b.entries[0].category = "c".into();
    let mut r = BookRenderer::new(b.clone());
    assert_eq!(r.shown(), None, "opens on the landing page");
    assert!(r.show("e", 1));
    assert_eq!(r.shown(), Some(("e".into(), 1)));
    r.prepare(800.0, 600.0);
    assert!(r.ui.is_some());

    b.entries[0].pages.push(BookPage::Empty);
    r.set_book(b);
    assert_eq!(r.shown(), Some(("e".into(), 1)));
    assert!(!r.show("missing", 0));
}

#[test]
fn pages_are_counted_as_written() {
    let long = "word ".repeat(400);
    let mut b = book(vec![
        BookPage::Text {
            text: long,
            title: None,
            font: None,
            font_size: None,
        },
        BookPage::Empty,
    ]);
    b.categories.push(yog_formats::book::BookCategory {
        id: "c".into(),
        name: "C".into(),
        ..Default::default()
    });
    b.entries[0].category = "c".into();
    let mut r = BookRenderer::new(b);
    assert!(r.show("e", 1));
    assert_eq!(
        r.shown(),
        Some(("e".into(), 1)),
        "the second page as written"
    );
    assert!(r.state.page > 1, "past the first page's flowed text");
    r.handle_event("prev_page");
    assert_eq!(
        r.shown(),
        Some(("e".into(), 0)),
        "flowed text belongs to the first"
    );
}

#[test]
fn reads_yog_recipes() {
    let shaped = parse_recipe(&j(r##"{"type":"shaped","pattern":["AB"],"key":{"A":"m:a","B":["m:x","#c:t"]},"result":"m:out","count":2}"##)).unwrap();
    assert_eq!(
        (shaped.kind.as_str(), shaped.output.as_str(), shaped.count),
        ("shaped", "m:out", 2)
    );
    let grid = &shaped.slots["grid"];
    assert_eq!(
        (grid.len(), &grid[1]),
        (9, &vec!["m:x".to_string(), "#c:t".to_string()]),
        "cells by place on the 3×3 grid"
    );
    let blast = parse_recipe(&j(
        r##"{"type":"blasting","ingredient":"m:ore","result":"m:ingot"}"##,
    ))
    .unwrap();
    assert_eq!(
        (
            blast.slots["input"][0][0].as_str(),
            blast.values["cook_time"].as_str()
        ),
        ("m:ore", "100")
    );
    let own = parse_recipe(&j(r##"{"type":"m:infusion","slots":{"ring":["m:a",["m:b","m:c"]]},"values":{"time":40},"result":"m:out"}"##)).unwrap();
    assert_eq!(
        (own.slots["ring"].len(), own.values["time"].as_str()),
        (2, "40")
    );
    assert!(
        parse_recipe(&j(r#"{"type":"minecraft:crafting_shaped"}"#)).is_none(),
        "vanilla JSON isn't the book's"
    );
}

#[test]
fn recipe_pages_are_drawn_by_layouts() {
    let theme = BookTheme::default();
    let mut recipes = HashMap::new();
    recipes.insert("m:r".to_string(), parse_recipe(&j(r##"{"type":"m:infusion","slots":{"ring":["m:a","m:b"]},"values":{"time":40},"result":"m:out"}"##)).unwrap());
    recipes.insert("m:s".to_string(), parse_recipe(&j(r##"{"type":"smithing","template":"m:t","base":"m:b","addition":"m:a","result":"m:out"}"##)).unwrap());
    recipes.insert(
        "m:p".to_string(),
        parse_recipe(&j(
            r##"{"type":"brewing","from":"m:awkward","ingredient":"m:dust","result":"m:clarity"}"##,
        ))
        .unwrap(),
    );
    let layout: yog_formats::book::layout::RecipeLayout = yog_data::from_text(
        r#"{ kinds: ["m:infusion"], size: [96, 40], parts: [
            { type: "slots", slot: "ring", at: [[0, 0], [20, 0]], frame: true },
            { type: "result", at: [76, 10] },
            { type: "text", text: "{time} ticks", at: [0, 30] },
        ] }"#,
    )
    .unwrap()
    .value;
    let draw = |id: &str| {
        let (mut bg, mut ov) = (Vec::new(), Vec::new());
        let page = BookPage::Recipe {
            recipe: id.into(),
            text: None,
        };
        let book = Book {
            layouts: vec![layout.clone()],
            ..Default::default()
        };
        let mut c = widgets::Canvas {
            theme: &theme,
            sheet: BOOK_SHEET,
            sx: 1.0,
            sy: 1.0,
            sprites: &mut bg,
            overlays: &mut ov,
        };
        let cx = page::Context {
            recipes: &recipes,
            book: &book,
            number: 0,
            at: yog_gfx::Vec2::new(0.0, 0.0),
        };
        page::build(&mut c, &cx, Some(&page));
        (bg, ov)
    };
    let (bg, ov) = draw("m:r");
    let lx = PAGE_W / 2.0 - 1.0 - 48.0;
    let items: Vec<(f32, f32)> = ov
        .iter()
        .filter_map(|o| match o {
            OverlayCmd::McItems { at, .. } | OverlayCmd::McItem { item_id: _, at } => {
                Some((at.x, at.y))
            }
            _ => None,
        })
        .collect();
    assert_eq!(items, [(lx, 10.0), (lx + 20.0, 10.0), (lx + 76.0, 20.0)]);
    assert_eq!(bg.len(), 2, "a frame behind each ring slot");
    assert!(
        ov.iter()
            .any(|o| matches!(o, OverlayCmd::McText { text, .. } if text == "40 ticks"))
    );
    // The game's kinds: built-in layouts (smithing had none before).
    let (bg, ov) = draw("m:s");
    assert!(
        bg.iter().any(|b| b.src.x == 11.0 && b.src.y == 135.0),
        "Patchouli's smithing strip"
    );
    assert_eq!(
        ov.iter()
            .filter(|o| matches!(o, OverlayCmd::McItems { .. }))
            .count(),
        3
    );
    // Brewing: Yog's own strip — ingredient, potion, the stand, result.
    let (bg, ov) = draw("m:p");
    assert!(
        bg.iter().any(|b| b.src.x == 11.0 && b.src.y == 181.0),
        "the brewing strip"
    );
    // Its two slots (ingredient, potion); the stand and the result are drawn apart.
    assert_eq!(
        ov.iter()
            .filter(|o| matches!(o, OverlayCmd::McItems { .. }))
            .count(),
        2
    );
}

#[test]
fn multiblocks_turn_and_animate() {
    let pattern = vec![vec!["G".to_string()], vec!["S ".to_string()]];
    let mapping = [
        ("G".to_string(), "minecraft:glass".to_string()),
        ("S".to_string(), "minecraft:stone".to_string()),
    ]
    .into();
    let blocks = yog_formats::book::multiblock_blocks(&pattern, &mapping);
    assert_eq!(
        blocks,
        [
            ((0, 1, 0), "minecraft:glass".to_string()),
            ((0, 0, 0), "minecraft:stone".to_string())
        ]
    );

    let mut b = book(vec![BookPage::Multiblock {
        name: None,
        pattern,
        mapping,
        text: None,
    }]);
    b.categories.push(yog_formats::book::BookCategory {
        id: "c".into(),
        name: "C".into(),
        ..Default::default()
    });
    b.entries[0].category = "c".into();
    let mut r = BookRenderer::new(b);
    r.prepare(800.0, 600.0);
    assert!(!r.animated(), "the landing page is still");
    assert_eq!(landing_fit("short"), 1.0);
    assert!(
        landing_fit(&"many words here ".repeat(60)) < 1.0,
        "long landing texts shrink"
    );
    r.show("e", 0);
    r.prepare(800.0, 600.0);
    assert!(r.animated(), "a structure turns");
}

#[test]
fn short_text_stays_put() {
    let pages = vec![BookPage::Text {
        text: "Hello".into(),
        title: Some("T".into()),
        font: None,
        font_size: None,
    }];
    let flowed = flow(book(pages.clone()));
    assert_eq!(flowed.entries[0].pages.len(), 1);
}

#[test]
fn a_drawn_page_hands_its_box_and_data_to_the_mods_drawer() {
    let data = yog_data::json::from_json(&serde_json::json!({ "op_id": "hexcasting:manual" }));
    let page: BookPage = yog_data::read_value(&yog_data::json::from_json(&serde_json::json!({
        "type": "drawn", "drawer": "hexcasting:pattern", "title": "Manual", "height": 64,
        "caption": "{u|entity} → {u|num}", "text": "Casts.", "data": { "op_id": "hexcasting:manual" }
    })))
    .unwrap();
    assert!(
        matches!(&page, BookPage::Drawn { data: d, height, .. } if *d == data && *height == 64.0)
    );
    assert_eq!(content_above_text(&page), Some(14.0 + 68.0 + 14.0));
    let mut b = book(vec![page.clone()]);
    b.categories.push(yog_formats::book::BookCategory {
        id: "c".into(),
        name: "C".into(),
        ..Default::default()
    });
    b.entries[0].category = "c".into();
    let mut r = BookRenderer::new(b);
    r.show("e", 0);
    r.prepare(800.0, 600.0);
    let drawn: Vec<_> = r
        .overlays
        .iter()
        .filter_map(|o| match o {
            OverlayCmd::Drawn { drawer, data, at } => {
                Some((drawer.clone(), data.clone(), at.w / at.h))
            }
            _ => None,
        })
        .collect();
    assert_eq!(drawn.len(), 1);
    assert_eq!(
        (drawn[0].0.as_str(), &drawn[0].1),
        ("hexcasting:pattern", &data)
    );
    assert!(
        (drawn[0].2 - PAGE_W / 64.0).abs() < 0.01,
        "the page's width, 64 high"
    );
    // Its Yog Data keeps the data.
    let back: BookPage = yog_data::read_bytes(&yog_data::to_bytes(&page)).unwrap();
    assert!(matches!(back, BookPage::Drawn { data: d, .. } if d == data));
}

#[test]
fn a_mods_page_type_lays_out_its_pages() {
    use yog_formats::book::page_type::{PageElement as E, PageProperty, PageType, PropKind};
    let ty = PageType {
        id: "hexcasting:pattern".into(),
        properties: vec![
            PageProperty {
                name: "name".into(),
                kind: PropKind::BookText,
                default: None,
            },
            PageProperty {
                name: "patterns".into(),
                kind: PropKind::Data,
                default: Some(yog_data::Value::List(vec![])),
            },
        ],
        body: vec![
            E::Header {
                text: "{x:Bind name}".into(),
                visible: None,
            },
            E::Drawn {
                drawer: "hexcasting:pattern".into(),
                height: 64.0,
                data: "{x:Bind patterns}".into(),
                visible: Some("{x:Bind patterns}".into()),
            },
        ],
    };
    let page = |json: &str| {
        BookPage::Custom(
            yog_data::read_value(&yog_data::json::from_json_text(json).unwrap()).unwrap(),
        )
    };
    let mut b = book(vec![
        page(
            r#"{"type":"hexcasting:pattern","name":"Manual","patterns":[{"startdir":"EAST","signature":"qaq"}]}"#,
        ),
        page(r#"{"type":"hexcasting:pattern","name":"Bare"}"#),
    ]);
    b.page_types.push(ty);
    b.categories.push(yog_formats::book::BookCategory {
        id: "c".into(),
        name: "C".into(),
        ..Default::default()
    });
    b.entries[0].category = "c".into();
    let mut r = BookRenderer::new(b);
    r.show("e", 0);
    r.prepare(800.0, 600.0);
    let drawn: Vec<_> = r
        .overlays
        .iter()
        .filter_map(|o| match o {
            OverlayCmd::Drawn { drawer, data, .. } => Some((drawer.clone(), data.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(
        drawn.len(),
        1,
        "the second page has no patterns: no picture"
    );
    assert_eq!(drawn[0].0, "hexcasting:pattern");
    assert!(
        matches!(&drawn[0].1, yog_data::Value::List(l) if l.len() == 1),
        "the field's whole value"
    );
}

#[test]
fn a_mods_page_flows_its_long_text_onto_text_pages() {
    use yog_formats::book::page_type::{PageElement as E, PageProperty, PageType, PropKind};
    let ty = PageType {
        id: "m:p".into(),
        properties: vec![
            PageProperty {
                name: "patterns".into(),
                kind: PropKind::Data,
                default: None,
            },
            PageProperty {
                name: "text".into(),
                kind: PropKind::BookText,
                default: None,
            },
        ],
        body: vec![
            E::Drawn {
                drawer: "m:d".into(),
                height: 64.0,
                data: "{x:Bind patterns}".into(),
                visible: None,
            },
            E::Text {
                text: "{x:Bind text}".into(),
                visible: None,
            },
        ],
    };
    let long = "word ".repeat(120);
    let page = format!(r#"{{"type":"m:p","patterns":[1],"text":"{long}"}}"#);
    let mut b = book(vec![BookPage::Custom(
        yog_data::read_value(&yog_data::json::from_json_text(&page).unwrap()).unwrap(),
    )]);
    b.page_types.push(ty);
    let flowed = flow(b);
    let pages = &flowed.entries[0].pages;
    assert!(pages.len() >= 2, "{pages:?}");
    let BookPage::Custom(first) = &pages[0] else {
        panic!("{:?}", pages[0])
    };
    let kept = first.fields.get("text").and_then(|v| v.as_str()).unwrap();
    let line_h = yog_ui::text::LINE_H + yog_ui::text::LINE_GAP;
    assert!(
        kept.lines().count() as f32 * line_h <= BODY_H - 68.0 + 1.0,
        "under the picture: {kept}"
    );
    let words: usize = kept.split_whitespace().count()
        + pages[1..]
            .iter()
            .map(|p| match p {
                BookPage::Text { text, .. } => text.split_whitespace().count(),
                _ => 0,
            })
            .sum::<usize>();
    assert_eq!(words, 120, "nothing lost");
}

#[test]
fn a_translated_book_is_flowed_from_its_words_not_its_keys() {
    // A key too long for a page: flowed as it is, it'd be cut in pieces.
    fn tr(k: &str) -> Option<String> {
        (k.len() == 3000 && k.starts_with("mymod.")).then(|| "Short.".into())
    }
    let key = format!("mymod.{}", "x".repeat(2994));
    let mut b = book(vec![BookPage::Text {
        text: key,
        title: None,
        font: None,
        font_size: None,
    }]);
    b.i18n = true;
    let mut r = BookRenderer::new(b);
    r.set_translator(tr);
    r.localize();
    let pages = &r.book().entries[0].pages;
    assert_eq!(pages.len(), 1, "{pages:?}");
    assert!(
        matches!(&pages[0], BookPage::Text { text, .. } if text == "Short."),
        "{pages:?}"
    );
}
