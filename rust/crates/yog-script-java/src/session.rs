//! What a hot reload keeps of a live object (docs/design/storage.md S8):
//! its members that aren't `saved` (those are in its store already) —
//! `synced` and plain ones — each a record like a saved member's, written
//! by `$keep` as its old classes go and read by `$restore` into the new
//! classes' object of the same owner (`Session`). A member the codec can't
//! keep yet starts again at its first value.

use yog_script_analyze::ast::{Decl, DeclKind, Modifier};
use yog_script_ids::java::{IF, NEW, NULL, PUBLIC, RECORD};
use yog_script_ids::jvm::session::{CLASS, KEEP, KEPT, KEY_SEP, LIVE, RESTORE};
use yog_script_ids::jvm::state::{
    ACCEPT, BYTES, FROM_BYTES, READ, READ_RECORD, READ_TYPE, RECORD_PREFIX, VALUE, WRITE,
    WRITE_TYPE,
};

use super::Gen;
use crate::saved::Stored;

/// The members of `body` a hot reload keeps: instance fields and stored
/// properties neither `saved` nor `readonly`.
pub(crate) fn session_members(body: &[Decl]) -> Vec<Stored<'_>> {
    body.iter()
        .filter(|d| !d.has(Modifier::Saved) && !d.has(Modifier::Static))
        .filter(|d| !d.has(Modifier::Const) && !d.has(Modifier::Readonly))
        .filter_map(|d| match &d.kind {
            DeclKind::Field(f) => Some(Stored {
                ty: &f.ty,
                name: &f.name.text,
                was: None,
            }),
            DeclKind::Property { ty, name, .. } if yog_script_analyze::ast::stores_value(d) => {
                Some(Stored {
                    ty,
                    name: &name.text,
                    was: None,
                })
            }
            _ => None,
        })
        .collect()
}

/// The interface a class keeping session state implements.
pub(crate) fn kept_interface() -> String {
    format!("{CLASS}.{KEPT}")
}

/// `made` live under its owner's key: the mod's, `class`'s, `owner`'s.
pub(crate) fn live(made: &str, (module, class, owner): (&str, &str, &str)) -> String {
    format!("{CLASS}.{LIVE}({made}, {module} + \"{KEY_SEP}{class}{KEY_SEP}\" + {owner})")
}

impl Gen<'_> {
    /// `$keep`/`$restore` of `body`'s session members, keyed by name; the
    /// record of each one not `synced` declared (a synced one's is the
    /// saved state's already).
    pub(crate) fn session_state(&mut self, body: &[Decl]) {
        let synced: Vec<&str> = crate::saved::synced_members(body)
            .iter()
            .map(|m| m.name)
            .collect();
        let (mut keep, mut restore) = (Vec::new(), Vec::new());
        for member in session_members(body) {
            let Some(kept) = self.codec_form(member.ty, member.name) else {
                continue;
            };
            let (field, record) = (
                Self::ident(member.name),
                format!("{RECORD_PREFIX}{}", member.name),
            );
            if !synced.contains(&member.name) {
                for line in &kept.declared {
                    self.code.line(line);
                }
                self.code.line(format!(
                    "{PUBLIC} {RECORD} {record}({} {VALUE}) {{}}",
                    kept.java
                ));
            }
            let key = self.code.str(member.name);
            let bytes = self.temp();
            let value = format!("{FROM_BYTES}({bytes}, {record}.class).{VALUE}()");
            keep.push(format!(
                "{WRITE}.{ACCEPT}({key}, {BYTES}({NEW} {record}({})));",
                (kept.save)(&field)
            ));
            restore.push(format!(
                "byte[] {bytes} = {READ}.{READ_RECORD}({key}); {IF} ({bytes} != {NULL}) {{ {} }}",
                (kept.load)(&field, &value)
            ));
        }
        self.state_method(KEEP, &format!("{WRITE_TYPE} {WRITE}"), &keep);
        self.state_method(RESTORE, &format!("{READ_TYPE} {READ}"), &restore);
    }
}
