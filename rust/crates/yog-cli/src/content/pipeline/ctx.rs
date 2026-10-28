//! The state checking one content file reports into.

use std::collections::BTreeSet;
use std::path::Path;
use yog_data::Path as At;
use yog_ids::Format;

use crate::content::CONTENT_DIR;
use crate::diagnostics::Diagnostic;
use crate::yres;

/// Per-file checking state.
pub(in crate::content) struct Ctx<'a> {
    pub(in crate::content) root: &'a Path,
    pub(in crate::content) refs: &'a yres::Refs,
    pub(in crate::content) file: String,
    pub(in crate::content) ns: String,
    pub(in crate::content) errors: &'a mut Vec<Diagnostic>,
    pub(in crate::content) warnings: &'a mut Vec<Diagnostic>,
    /// The mod's own textures' ids (`.ytexture` files).
    pub(in crate::content) own_textures: &'a BTreeSet<String>,
    /// `block.inventory` screen ids named so far: (file, line, id) —
    /// checked against the mod's screens once every file is read.
    pub(in crate::content) inventories: &'a mut Vec<(String, usize, String)>,
    /// References a file makes (a block's drops), checked once every file is read.
    pub(in crate::content) deferred: &'a mut Vec<Deferred>,
}

/// What a file names that's checked once every file is read.
pub(in crate::content) enum Wanted {
    /// Content of a kind: a level, a material, an item….
    Ref(yog_data::Ref),
    /// A tag of one of these kinds of content.
    Tag { kinds: Vec<String>, id: String },
}

/// A deferred check: where the file names it, and what.
pub(in crate::content) struct Deferred {
    pub(in crate::content) file: String,
    pub(in crate::content) line: usize,
    pub(in crate::content) path: String,
    pub(in crate::content) wanted: Wanted,
}

/// What every content file is checked against.
pub(in crate::content) struct Shared<'a> {
    pub(in crate::content) root: &'a Path,
    pub(in crate::content) refs: &'a yres::Refs,
    /// The mod's namespace (the game's when the mod id isn't one).
    pub(in crate::content) ns: &'a str,
    /// The mod's own textures' ids (`.ytexture` files).
    pub(in crate::content) own_textures: &'a BTreeSet<String>,
}

impl Shared<'_> {
    /// The id of the content file at `path`: `<ns>:<its path under
    /// content/, without the extension>`.
    pub(in crate::content) fn content_id(&self, path: &Path) -> String {
        let rel = path
            .strip_prefix(self.root.join(CONTENT_DIR))
            .unwrap_or(path)
            .with_extension("");
        yog_ids::join(self.ns, &crate::slash_path(&rel))
    }
}

/// What checking every content file reports into.
#[derive(Default)]
pub(in crate::content) struct Sink {
    pub(in crate::content) errors: Vec<Diagnostic>,
    pub(in crate::content) warnings: Vec<Diagnostic>,
    pub(in crate::content) inventories: Vec<(String, usize, String)>,
    pub(in crate::content) deferred: Vec<Deferred>,
}

impl Sink {
    /// The state checking file `file` reports into.
    pub(in crate::content) fn cx<'s>(&'s mut self, shared: &Shared<'s>, file: &str) -> Ctx<'s> {
        Ctx {
            root: shared.root,
            refs: shared.refs,
            file: file.to_string(),
            ns: shared.ns.to_string(),
            errors: &mut self.errors,
            warnings: &mut self.warnings,
            own_textures: shared.own_textures,
            inventories: &mut self.inventories,
            deferred: &mut self.deferred,
        }
    }
}

impl Ctx<'_> {
    pub(in crate::content) fn err(&mut self, line: usize, msg: impl std::fmt::Display) {
        self.errors
            .push(problem(msg).in_file(&self.file).at_line(line));
    }

    /// Checks `wanted`, named at `path` (line `line`), once every file is read.
    pub(in crate::content) fn defer(&mut self, line: usize, path: &At, wanted: Wanted) {
        self.deferred.push(Deferred {
            file: self.file.clone(),
            line,
            path: path.to_string(),
            wanted,
        });
    }

    pub(in crate::content) fn warn(&mut self, line: usize, msg: impl std::fmt::Display) {
        self.warnings
            .push(caveat(msg).in_file(&self.file).at_line(line));
    }

    /// A problem with the value at `path`.
    pub(in crate::content) fn err_at(&mut self, loc: Loc, path: &At, msg: impl std::fmt::Display) {
        let found = problem(msg).in_file(&self.file).at_line(loc(path));
        self.errors.push(found.at_path(path.clone()));
    }

    pub(in crate::content) fn warn_at(&mut self, loc: Loc, path: &At, msg: impl std::fmt::Display) {
        let found = caveat(msg).in_file(&self.file).at_line(loc(path));
        self.warnings.push(found.at_path(path.clone()));
    }

    /// Reports the rules a file breaks ([`yog_formats::rules`]): each an
    /// error or a warning, at its line.
    pub(in crate::content) fn broken_rules(&mut self, loc: Loc, broken: Vec<Diagnostic>) {
        for d in broken {
            let line = loc(&d.path);
            let d = d.in_file(&self.file).at_line(line);
            if d.is_error() {
                self.errors.push(d);
            } else {
                self.warnings.push(d);
            }
        }
    }

    /// Reports what reading a file through its schema found.
    pub(in crate::content) fn schema_errors(&mut self, errors: yog_data::Errors) {
        for e in errors.0 {
            self.errors.push(e.in_file(&self.file));
        }
    }

    /// A texture → the model's form: `#slot`, or a texture id — the mod's
    /// own must be one of its `.ytexture`s.
    pub(in crate::content) fn texture(
        &mut self,
        loc: Loc,
        path: &At,
        t: &yog_formats::content::Texture,
    ) -> Option<String> {
        use yog_formats::content::{SLOT_MARK, Texture};
        match t {
            Texture::Slot(slot) => Some(format!("{SLOT_MARK}{slot}")),
            Texture::Ref(id) => {
                let (ns, tex) = yog_ids::split(id);
                if ns == self.ns && !self.own_textures.contains(id) {
                    self.err_at(
                        loc,
                        path,
                        format!(
                            "`{}{id}`: no {}/{tex}.{}",
                            yog_ids::reference::TEXTURE,
                            yres::RESOURCES_DIR,
                            Format::Texture.ext()
                        ),
                    );
                    return None;
                }
                Some(id.clone())
            }
        }
    }
}

/// Where a path of the file being read is: its line.
pub(in crate::content) type Loc<'a> = &'a dyn Fn(&At) -> usize;

/// A content error (`YB0002`), where it is to be said by the caller.
pub(in crate::content) fn problem(msg: impl std::fmt::Display) -> Diagnostic {
    Diagnostic::error(yog_data::Atom::new(yog_ids::build_problem::CONTENT), msg)
}

/// A content warning (`YB0002`).
pub(in crate::content) fn caveat(msg: impl std::fmt::Display) -> Diagnostic {
    Diagnostic::warning(yog_data::Atom::new(yog_ids::build_problem::CONTENT), msg)
}
