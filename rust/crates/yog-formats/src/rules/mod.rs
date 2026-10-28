//! What a file must keep that its schema alone doesn't say — fields that
//! don't go together, a size that can't be zero — read off the file alone,
//! no other file or the project needed. [`crate::problems`] adds them to the
//! schema's, so an editor shows them as the file is edited; the build reads
//! the same.

mod block;
mod element;
mod item;
mod kind;
mod model;
mod states;

use yog_data::{Atom, Diagnostic, Path};

/// The rules a format's file keeps beyond its schema.
pub trait Rules {
    /// Every rule it breaks, into `out`.
    fn rules(&self, _out: &mut Vec<Diagnostic>) {}
}

/// A broken rule at `path`.
fn error(path: Path, message: impl std::fmt::Display) -> Diagnostic {
    Diagnostic::error(Atom::new(yog_ids::build_problem::CONTENT), message).at_path(path)
}

/// Something that won't do what it says, at `path`.
fn warning(path: Path, message: impl std::fmt::Display) -> Diagnostic {
    Diagnostic::warning(Atom::new(yog_ids::build_problem::CONTENT), message).at_path(path)
}

/// The formats whose files keep no rules beyond their schema.
macro_rules! no_rules {
    ($($t:ty),* $(,)?) => {$( impl Rules for $t {} )*};
}

no_rules!(
    yog_registry::EntityDef,
    crate::recipe::RecipeFile,
    crate::sound::SoundFile,
    yog_registry::Particle,
    crate::video::Video,
    crate::book::Book,
    crate::def::Def,
    crate::yres::YresFile,
    yog_body::format::ModelFile,
    yog_body::format::AnimFile,
    yog_font::Font,
    crate::texture::TextureFile,
    crate::material::MaterialFile,
);

#[cfg(test)]
mod tests;
