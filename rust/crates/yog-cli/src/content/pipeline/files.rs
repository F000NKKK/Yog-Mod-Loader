//! The checked books, entities, bodies and fonts: their data and where
//! it's kept under `.yog-build/generated/`.

use yog_ids::Format;
use yog_ids::sep::PATH;

use crate::content::pipeline::file::generated;

/// Their folders under `.yog-build/generated/`.
const BOOKS_DIR: &str = "books";
const ENTITIES_DIR: &str = yog_ids::word::ENTITIES;
const FONTS_DIR: &str = "fonts";
const BODY_DIR: &str = "body";
const MODELS_DIR: &str = "models";
const ANIMS_DIR: &str = "anims";

/// One checked `.ybook`.
pub(in crate::content) struct BookFile {
    pub(in crate::content) id: String,
    /// The book as the runtime reads it (`yog_formats::book::Book`'s schema), as
    /// binary Yog Data: the mod hands these bytes over as they are.
    pub(in crate::content) data: Vec<u8>,
}

/// One checked `.yentity`.
pub(in crate::content) struct EntityFile {
    pub(in crate::content) id: String,
    pub(in crate::content) file: String,
    /// Its model and clips, ids with their namespace.
    pub(in crate::content) model: Option<String>,
    pub(in crate::content) animations: Option<String>,
    /// `yog_registry::EntityDef`, as binary Yog Data.
    pub(in crate::content) data: Vec<u8>,
}

impl EntityFile {
    /// Its file under `.yog-build/generated/`.
    pub(in crate::content) fn path(&self) -> String {
        generated(ENTITIES_DIR, &self.id, Format::Data)
    }
}

/// One checked `.ymodel` or `.yanim`.
pub(in crate::content) struct BodyFile {
    pub(in crate::content) model: bool,
    /// `<mod>:<path under content/ without the extension>`.
    pub(in crate::content) id: String,
    /// As `yog_body` reads it: binary Yog Data.
    pub(in crate::content) data: Vec<u8>,
}

/// One checked `.yfont`.
pub(in crate::content) struct FontFile {
    /// `<mod>:<path under content/ without the extension>`.
    pub(in crate::content) id: String,
    /// The built `.yfont`, as binary Yog Data.
    pub(in crate::content) data: Vec<u8>,
}

impl FontFile {
    /// Its file under `.yog-build/generated/`.
    pub(in crate::content) fn path(&self) -> String {
        generated(FONTS_DIR, &self.id, Format::Font)
    }
}

impl BodyFile {
    /// Its file under `.yog-build/generated/`.
    pub(in crate::content) fn path(&self) -> String {
        let dir = if self.model { MODELS_DIR } else { ANIMS_DIR };
        generated(&[BODY_DIR, dir].join(PATH), &self.id, Format::Data)
    }
}

impl BookFile {
    /// Its file under `.yog-build/generated/`.
    pub(in crate::content) fn path(&self) -> String {
        generated(BOOKS_DIR, &self.id, Format::Data)
    }
}
