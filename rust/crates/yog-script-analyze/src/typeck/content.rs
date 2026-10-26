//! A content declaration (`block Lamp { … }`) is its file's behaviour:
//! what the block is — its strength, light, model, name — is in its file
//! (`content/**/lamp.yblock`), the one place it's said; the declaration
//! holds only fields, constructors and handlers, bound to that file by id.

use yog_ids::{Format, script_problem};

use super::Checker;
use crate::ast::{Decl, DeclKind, Name};

/// The path of content declaration `name`'s id (`Lamp`: `lamp`), in its
/// mod's namespace: the file it binds to is `lamp.yblock`.
pub fn content_path(name: &str) -> String {
    yog_codegen::snake(name)
}

impl Checker<'_, '_, '_> {
    /// Content declaration `name` of kind `word` (`body` its members):
    /// no properties, and its file there when the mod's files are known.
    pub(super) fn content_decl(&mut self, word: &str, name: &Name, body: &[Decl]) {
        // A recipe's declaration is the recipe (`.yrecipe`'s other spelling).
        if word == yog_ids::kind::RECIPE {
            return;
        }
        let Some(format) = Format::ALL.iter().find(|f| f.noun() == word) else {
            return;
        };
        let file = format.file(&content_path(&name.text));
        for d in body {
            if let DeclKind::Prop(p) = &d.kind {
                self.expr(&p.value);
                self.report.error(
                    script_problem::CONTENT_PROPERTY,
                    p.name.span,
                    format!(
                        "`{}` is said in `{file}`: a `{word}` declaration holds only its behaviour",
                        p.name.text
                    ),
                );
            }
        }
        let Some(content) = &self.content else {
            return;
        };
        let path = content_path(&name.text);
        let found = content
            .iter()
            .any(|(kind, id)| kind == word && yog_ids::split(id).1 == path);
        if !found {
            self.report.error(
                script_problem::NO_CONTENT_FILE,
                name.span,
                format!(
                    "no `{file}` under `{}/`: `{word} {}` is its behaviour",
                    yog_ids::project::CONTENT,
                    name.text
                ),
            );
        }
    }
}
