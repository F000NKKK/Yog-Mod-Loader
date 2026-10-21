//! The `.yfont` format: what a font is once built, whatever it was made
//! from (TrueType, a picture of glyphs, pixels drawn in an editor, a
//! script's rules). Yog Data, binary.
//!
//! Units are the font's own (`units_per_em` to an em), `y` up from the
//! baseline. A glyph is a drawing ([`Op`]: paths filled or stroked,
//! images) — never a notion of what it's for.

use yog_data::Schema;
use yog_graphics::Op;

/// A font: its metrics, its glyphs, which characters they draw and the
/// rules that turn text into glyphs ([`crate::shape`]).
#[derive(Schema, Debug, Clone, PartialEq, Default)]
pub struct Font {
    /// How many units an em is: a glyph drawn at size `s` is scaled by
    /// `s / units_per_em`.
    pub units_per_em: u32,
    /// How far above the baseline its tallest glyphs reach.
    pub ascent: i32,
    /// How far below the baseline its glyphs reach (negative: below).
    pub descent: i32,
    /// Space between one line's descent and the next one's ascent.
    #[data(default)]
    pub line_gap: i32,
    /// Its glyphs; a glyph is named by its index.
    pub glyphs: Vec<Glyph>,
    /// Which glyph draws each character.
    pub chars: Vec<CharGlyph>,
    /// Glyph sequences one glyph draws (`f` `i` → `fi`), the longest first.
    #[data(default)]
    pub ligatures: Vec<Ligature>,
    /// Contextual forms: glyphs replaced by what comes before them.
    #[data(default)]
    pub machines: Vec<Machine>,
    /// Extra space between pairs of glyphs.
    #[data(default)]
    pub kerning: Vec<Kern>,
    /// How a run of glyphs joined to each other ([`Glyph::entry`]) that
    /// grows too tall is fitted into the line.
    pub fit: Option<Fit>,
}

/// One glyph: how far it moves the pen, and its drawing.
#[derive(Schema, Debug, Clone, PartialEq, Default)]
pub struct Glyph {
    /// What it's called where the font is made (`a`, `heart`, `rune_ne`).
    #[data(default)]
    pub name: String,
    /// How far the pen moves after it.
    #[data(default)]
    pub advance: i32,
    /// Its drawing, in order; parts without a color take the text's.
    #[data(default)]
    pub ops: Vec<Op>,
    /// Where the glyph before it, if that one has an [`Glyph::exit`], joins it
    /// — joined glyphs are placed point to point, up and down too (cursive
    /// scripts; runes drawn stroke by stroke).
    pub entry: Option<[i32; 2]>,
    /// Where the glyph after it joins it.
    pub exit: Option<[i32; 2]>,
}

/// Character `char` (a Unicode scalar) is drawn by glyph `glyph`.
#[derive(Schema, Debug, Clone, Copy, PartialEq)]
pub struct CharGlyph {
    pub char: u32,
    pub glyph: u32,
}

/// Glyphs `glyphs`, in a row, are drawn by glyph `glyph`.
#[derive(Schema, Debug, Clone, PartialEq)]
pub struct Ligature {
    pub glyphs: Vec<u32>,
    pub glyph: u32,
}

/// A state machine over the glyphs of a run, left to right (as Apple's
/// `morx` tables): each glyph's class and the machine's state pick a
/// transition, which may put another glyph in its place and moves to its
/// next state. A glyph with no transition from the state leaves it as it is
/// and starts the machine over.
#[derive(Schema, Debug, Clone, PartialEq, Default)]
pub struct Machine {
    /// Glyphs of each class; a glyph in none has no transitions.
    pub classes: Vec<GlyphClass>,
    pub transitions: Vec<Transition>,
}

#[derive(Schema, Debug, Clone, PartialEq)]
pub struct GlyphClass {
    pub class: u32,
    pub glyphs: Vec<u32>,
}

/// From state `state` (0: the start), a glyph of class `class` moves the
/// machine to `next`, put glyph `glyph` in its place if there is one.
#[derive(Schema, Debug, Clone, Copy, PartialEq)]
pub struct Transition {
    pub state: u32,
    pub class: u32,
    pub next: u32,
    pub glyph: Option<u32>,
}

/// `left` followed by `right` are `x` units further apart.
#[derive(Schema, Debug, Clone, Copy, PartialEq)]
pub struct Kern {
    pub left: u32,
    pub right: u32,
    pub x: i32,
}

/// A run of joined glyphs taller than `height` units is scaled down to it,
/// centred on `center` units above the baseline.
#[derive(Schema, Debug, Clone, Copy, PartialEq)]
pub struct Fit {
    pub height: f32,
    pub center: f32,
}

impl Font {
    /// Checks the references and metrics that must be valid before a font is
    /// shaped or rasterized.
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if self.units_per_em == 0 {
            errors.push("units_per_em must be greater than zero".to_string());
        }
        if self.glyphs.is_empty() {
            errors.push("a font must have at least one glyph".to_string());
        }
        self.validate_chars(&mut errors);
        self.validate_ligatures(&mut errors);
        for (i, machine) in self.machines.iter().enumerate() {
            self.validate_machine(i, machine, &mut errors);
        }
        self.validate_kerning(&mut errors);
        if let Some(fit) = self.fit
            && (!fit.height.is_finite() || fit.height <= 0.0 || !fit.center.is_finite())
        {
            errors.push("fit.height must be positive and fit values must be finite".to_string());
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Whether glyph `glyph` is one of the font's.
    fn has_glyph(&self, glyph: u32) -> bool {
        (glyph as usize) < self.glyphs.len()
    }

    fn validate_chars(&self, errors: &mut Vec<String>) {
        for (i, m) in self.chars.iter().enumerate() {
            if char::from_u32(m.char).is_none() {
                errors.push(format!("chars[{i}].char is not a Unicode scalar value"));
            }
            if !self.has_glyph(m.glyph) {
                errors.push(format!(
                    "chars[{i}].glyph {} does not name a glyph",
                    m.glyph
                ));
            }
        }
    }

    fn validate_ligatures(&self, errors: &mut Vec<String>) {
        for (i, ligature) in self.ligatures.iter().enumerate() {
            if ligature.glyphs.len() < 2 {
                errors.push(format!("ligatures[{i}] must contain at least two glyphs"));
            }
            for &glyph in ligature
                .glyphs
                .iter()
                .chain(std::iter::once(&ligature.glyph))
            {
                if !self.has_glyph(glyph) {
                    errors.push(format!("ligatures[{i}] references missing glyph {glyph}"));
                }
            }
        }
    }

    /// Machine `i`: each class once, each glyph in one class, transitions
    /// between known classes, once each, to known glyphs.
    fn validate_machine(&self, i: usize, machine: &Machine, errors: &mut Vec<String>) {
        let mut classes = std::collections::HashSet::new();
        let mut members = std::collections::HashSet::new();
        for class in &machine.classes {
            if !classes.insert(class.class) {
                errors.push(format!("machines[{i}] repeats class {}", class.class));
            }
            for &glyph in &class.glyphs {
                if !self.has_glyph(glyph) {
                    errors.push(format!(
                        "machines[{i}] class {} references missing glyph {glyph}",
                        class.class
                    ));
                }
                if !members.insert(glyph) {
                    errors.push(format!(
                        "machines[{i}] assigns glyph {glyph} to more than one class"
                    ));
                }
            }
        }
        let mut transitions = std::collections::HashSet::new();
        for t in &machine.transitions {
            if !classes.contains(&t.class) {
                errors.push(format!(
                    "machines[{i}] transition references missing class {}",
                    t.class
                ));
            }
            if !transitions.insert((t.state, t.class)) {
                errors.push(format!(
                    "machines[{i}] repeats transition from state {} for class {}",
                    t.state, t.class
                ));
            }
            if let Some(glyph) = t.glyph
                && !self.has_glyph(glyph)
            {
                errors.push(format!(
                    "machines[{i}] transition references missing glyph {glyph}"
                ));
            }
        }
    }

    fn validate_kerning(&self, errors: &mut Vec<String>) {
        let mut kerning = std::collections::HashSet::new();
        for kern in &self.kerning {
            if !self.has_glyph(kern.left) || !self.has_glyph(kern.right) {
                errors.push(format!(
                    "kerning pair ({}, {}) references a missing glyph",
                    kern.left, kern.right
                ));
            }
            if !kerning.insert((kern.left, kern.right)) {
                errors.push(format!(
                    "kerning pair ({}, {}) is repeated",
                    kern.left, kern.right
                ));
            }
        }
    }

    /// A built font from its file's bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Font, String> {
        let font = yog_data::read_bytes::<Font>(bytes).map_err(|e| format!("{e:?}"))?;
        font.validate().map_err(|errors| errors.join("; "))?;
        Ok(font)
    }

    /// The font's file.
    pub fn to_bytes(&self) -> Vec<u8> {
        yog_data::binary::encode(&self.write())
    }

    /// The glyph that draws `c`.
    pub fn glyph_of(&self, c: char) -> Option<u32> {
        let c = c as u32;
        self.chars.iter().find(|m| m.char == c).map(|m| m.glyph)
    }
}
