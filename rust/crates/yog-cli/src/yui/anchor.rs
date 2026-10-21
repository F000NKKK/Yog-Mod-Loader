//! Where a `<Screen>` sits in the game's window: its `anchor` — read the
//! same by the generated code and the designer.

use yog_ids::screen_anchor as a;
use yog_ui::layout::Align;

/// Each anchor's place on each axis: (horizontal, vertical).
const ANCHORS: [(&str, Align, Align); 9] = [
    (a::CENTER, Align::Center, Align::Center),
    (a::TOP_LEFT, Align::Start, Align::Start),
    (a::TOP, Align::Center, Align::Start),
    (a::TOP_RIGHT, Align::End, Align::Start),
    (a::LEFT, Align::Start, Align::Center),
    (a::RIGHT, Align::End, Align::Center),
    (a::BOTTOM_LEFT, Align::Start, Align::End),
    (a::BOTTOM, Align::Center, Align::End),
    (a::BOTTOM_RIGHT, Align::End, Align::End),
];

/// Where a screen sits: in the whole window, or placed on each axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Anchor {
    Fill,
    At(Align, Align),
}

impl Anchor {
    /// The anchor named `word`; none for an unknown word.
    pub(crate) fn parse(word: &str) -> Option<Anchor> {
        if word.trim() == a::FILL {
            return Some(Anchor::Fill);
        }
        ANCHORS
            .iter()
            .find(|(w, _, _)| *w == word.trim())
            .map(|&(_, h, v)| Anchor::At(h, v))
    }

    /// Every anchor's word.
    pub(crate) fn words() -> Vec<&'static str> {
        ANCHORS
            .iter()
            .map(|(w, _, _)| *w)
            .chain([a::FILL])
            .collect()
    }

    /// A screen's anchor, centred when it doesn't say.
    pub(crate) fn of(written: Option<&str>) -> Option<Anchor> {
        Anchor::parse(written.unwrap_or(a::CENTER))
    }
}
