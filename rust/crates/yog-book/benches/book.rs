//! A large book (the size of Hex Casting's: 80 entries, ~550 pages):
//! reading its JSON, flowing its text onto pages, drawing a spread
//! (against a graphics table that draws nothing).

use criterion::{Criterion, criterion_group, criterion_main};
use yog_book::{Book, BookCategory, BookEntry, BookPage, BookRenderer};
use yog_gfx::GfxContext;

fn big_book() -> Book {
    let words = "Media is a form of {accent|mental energy} external to a mind; all living creatures generate trace amounts of it. ";
    let categories = (0..9)
        .map(|c| BookCategory {
            id: format!("b:c{c}"),
            name: format!("Category {c}"),
            ..Default::default()
        })
        .collect();
    let entries = (0..80)
        .map(|e| BookEntry {
            id: format!("b:e{e}"),
            name: format!("Entry {e}"),
            category: format!("b:c{}", e % 9),
            pages: (0..7)
                .map(|p| match p % 3 {
                    0 => BookPage::Text {
                        text: words.repeat(6),
                        title: None,
                        font: None,
                        font_size: None,
                    },
                    1 => BookPage::Spotlight {
                        item: "minecraft:amethyst_shard".into(),
                        title: None,
                        text: Some(words.repeat(2)),
                    },
                    _ => BookPage::Recipe {
                        recipe: "b:r".into(),
                        text: Some(words.into()),
                    },
                })
                .collect(),
            ..Default::default()
        })
        .collect();
    Book {
        id: "b:book".into(),
        name: "Big Book".into(),
        categories,
        entries,
        ..Default::default()
    }
}

fn book(c: &mut Criterion) {
    let json = big_book().to_json();
    c.bench_function("book/parse_json", |b| {
        b.iter(|| serde_json::from_str::<Book>(&json).unwrap())
    });
    let parsed: Book = serde_json::from_str(&json).unwrap();
    c.bench_function("book/flow", |b| {
        b.iter(|| BookRenderer::new(parsed.clone()))
    });

    let table = yog_gfx::testing::null_api(480.0, 270.0);
    // SAFETY: `table` outlives `ctx`, and every function in it does nothing.
    let ctx = unsafe { GfxContext::from_raw(&table) };
    let mut r = BookRenderer::new(parsed);
    r.add_recipe("b:r", r#"{"type":"shaped","pattern":["AA","AA"],"key":{"A":"minecraft:stick"},"result":"minecraft:crafting_table"}"#);
    r.show("b:e40", 1);
    c.bench_function("book/render_spread", |b| {
        b.iter(|| r.render(&ctx, 480.0, 270.0))
    });
}

criterion_group!(benches, book);
criterion_main!(benches);
