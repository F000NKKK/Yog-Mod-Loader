//! Flowing text: what doesn't fit its page goes on to new text pages after
//! it, and each shown page remembers which page as written it came from.

use std::collections::HashMap;

use yog_formats::book::page_type::{CustomPage, PageElement, PageType};
use yog_formats::book::{Book, BookPage};

use super::geometry::{
    BODY_H, BODY_Y, GAP, ITEM_ROOM, MB_NAME_H, MB_VIEW, PICTURE_PAGE_ABOVE, PICTURE_TITLE_ROOM,
    RECIPE_H, RELATIONS_HEAD, RELATIONS_LINE, SPOT_TEXT_Y, TEXT_W,
};

/// How much of the body a page's own content takes above its text.
pub(super) fn content_above_text(page: &BookPage) -> Option<f32> {
    let room = |shown: bool, h: f32| if shown { h } else { 0.0 };
    Some(match page {
        BookPage::Text { title, .. } => room(title.is_some(), BODY_Y),
        BookPage::Spotlight { .. } => SPOT_TEXT_Y,
        BookPage::Recipe { .. } => RECIPE_H,
        BookPage::Image { .. } | BookPage::Entity { .. } => PICTURE_PAGE_ABOVE,
        BookPage::Drawn {
            title,
            height,
            caption,
            ..
        } => {
            room(title.is_some(), PICTURE_TITLE_ROOM)
                + room(*height > 0.0, height + GAP)
                + room(caption.is_some(), PICTURE_TITLE_ROOM)
        }
        BookPage::Multiblock { name, .. } => MB_VIEW + GAP + room(name.is_some(), MB_NAME_H),
        BookPage::Relations { entries, .. } => {
            RELATIONS_HEAD + entries.len() as f32 * RELATIONS_LINE
        }
        _ => return None,
    })
}

fn page_text_mut(page: &mut BookPage) -> Option<&mut String> {
    match page {
        BookPage::Text { text, .. } => Some(text),
        BookPage::Spotlight { text, .. }
        | BookPage::Recipe { text, .. }
        | BookPage::Image { text, .. }
        | BookPage::Entity { text, .. }
        | BookPage::Multiblock { text, .. }
        | BookPage::Relations { text, .. }
        | BookPage::Drawn { text, .. } => text.as_mut(),
        _ => None,
    }
}

/// `book` with its texts' own words (see [`crate::markup`]) as Yog text's.
pub(crate) fn styled(book: &Book) -> Book {
    book.localized(|t| crate::markup::resolve(t, &book.accent))
}

/// `book` flowed, and for each entry where each of its pages came from: the
/// index of the page in the book as written.
pub(crate) fn flow_traced(mut book: Book) -> (Book, HashMap<String, Vec<usize>>) {
    let mut origins = HashMap::new();
    let line_h = yog_ui::text::LINE_H + yog_ui::text::LINE_GAP;
    let fits = |h: f32| (((h + yog_ui::text::LINE_GAP) / line_h).floor() as usize).max(1);
    for entry in &mut book.entries {
        let mut pages = Vec::with_capacity(entry.pages.len());
        let mut from = Vec::with_capacity(entry.pages.len());
        for (written, page) in std::mem::take(&mut entry.pages).into_iter().enumerate() {
            let split = split_page(page, &book.page_types, &fits);
            from.extend(std::iter::repeat_n(written, split.len()));
            pages.extend(split);
        }
        entry.pages = pages;
        origins.insert(entry.id.clone(), from);
    }
    (book, origins)
}

/// The binding a mod's page type's text shows (`{x:Bind name}`).
const BIND_OPEN: &str = "{x:Bind ";
const BIND_CLOSE: char = '}';

/// A mod's page's room for its text: the property its last text shows
/// (`<BookText text="{x:Bind text}"/>`, nothing else in it), and the
/// height the rest of the page takes, in book pixels.
fn custom_text(page: &CustomPage, types: &[PageType]) -> Option<(String, f32)> {
    let t = types.iter().find(|t| t.id == page.kind)?;
    let line_h = yog_ui::text::LINE_H + yog_ui::text::LINE_GAP;
    let shown: Vec<&PageElement> = t
        .body
        .iter()
        .filter(|el| match el {
            PageElement::Header { visible, .. }
            | PageElement::Text { visible, .. }
            | PageElement::Item { visible, .. }
            | PageElement::Image { visible, .. }
            | PageElement::Drawn { visible, .. } => t.visible(page, visible),
            PageElement::Spacer { .. } => true,
        })
        .collect();
    let at = shown
        .iter()
        .rposition(|el| matches!(el, PageElement::Text { .. }))?;
    let PageElement::Text { text, .. } = shown[at] else {
        return None;
    };
    let name = text
        .trim()
        .strip_prefix(BIND_OPEN)?
        .strip_suffix(BIND_CLOSE)?
        .trim();
    if !matches!(page.fields.get(name), Some(yog_data::Value::Str(_))) {
        return None;
    }
    let others: f32 = shown
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != at)
        .map(|(_, el)| match el {
            PageElement::Header { .. } => PICTURE_TITLE_ROOM,
            PageElement::Text { text, .. } => lines(&t.text(page, text)) as f32 * line_h + GAP,
            PageElement::Item { .. } => ITEM_ROOM,
            PageElement::Image { height, .. }
            | PageElement::Drawn { height, .. }
            | PageElement::Spacer { height } => height + GAP,
        })
        .sum();
    Some((name.to_string(), others))
}

/// The lines `text` wraps to on a page.
fn lines(text: &str) -> usize {
    text.split('\n')
        .map(|p| yog_ui::text::wrap_text(p, TEXT_W, 1.0).len().max(1))
        .sum()
}

/// `page`, and the text pages its overflowing text goes on to.
fn split_page(
    mut page: BookPage,
    types: &[PageType],
    fits: &dyn Fn(f32) -> usize,
) -> Vec<BookPage> {
    let (above, text) = match &mut page {
        BookPage::Custom(c) => {
            let Some((name, above)) = custom_text(c, types) else {
                return vec![page];
            };
            let Some(yog_data::Value::Str(text)) = c.fields.get_mut(&name) else {
                return vec![page];
            };
            (above, text)
        }
        _ => {
            let Some(above) = content_above_text(&page) else {
                return vec![page];
            };
            let Some(text) = page_text_mut(&mut page) else {
                return vec![page];
            };
            (above, text)
        }
    };
    let lines: Vec<String> = text
        .split('\n')
        .flat_map(|para| {
            if para.is_empty() {
                vec![String::new()]
            } else {
                yog_ui::text::wrap_text(para, TEXT_W, 1.0)
            }
        })
        .collect();
    let first = fits(BODY_H - above);
    if lines.len() <= first {
        return vec![page];
    }
    *text = lines[..first].join("\n");
    let rest = lines[first..]
        .chunks(fits(BODY_H))
        .map(|chunk| BookPage::Text {
            text: chunk.join("\n"),
            title: None,
            font: None,
            font_size: None,
        });
    std::iter::once(page).chain(rest).collect()
}
