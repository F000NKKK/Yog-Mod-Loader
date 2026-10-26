//! What Java asks every object — `equals`, `hashCode`, `toString` — as a
//! type answers it: its own `Equals`, `GetHashCode`, `ToString` when it
//! declares them; a struct's fields when it doesn't (.NET's `ValueType`),
//! so a set or a dictionary's key finds a struct by its value.

use yog_script_analyze::ast::{Decl, DeclKind, Method, Modifier};
use yog_script_ids::java::INT;
use yog_script_ids::jvm::{EQUALS, HASH, HASH_CODE, OBJECT, OBJECTS, STRING, TO_STRING};
use yog_script_ids::members::{EQUALS as YS_EQUALS, GET_HASH_CODE, TO_STRING as YS_TO_STRING};

use super::Gen;
use yog_script_ids::java::{BOOLEAN, INSTANCEOF, OVERRIDE, PUBLIC, RETURN, TRUE};

/// What Java's `equals` names the other object.
const OTHER: &str = "other";

impl Gen<'_> {
    /// Type `name`'s `equals`, `hashCode`, `toString` (`fields`: what a
    /// struct is compared by, when it has no `Equals` of its own).
    pub(crate) fn identity(&mut self, name: &str, body: &[Decl], fields: Option<&[&str]>) {
        let fields: Option<Vec<String>> =
            fields.map(|f| f.iter().map(|f| Self::ident(f)).collect());
        if own_method(body, YS_EQUALS, 1) {
            self.equals(name, &format!("{YS_EQUALS}(that)"));
        } else if let Some(fields) = &fields {
            let same: Vec<String> = fields
                .iter()
                .map(|f| format!("{OBJECTS}.{EQUALS}({f}, that.{f})"))
                .collect();
            self.equals(name, &all_of(&same));
        }
        if own_method(body, GET_HASH_CODE, 0) {
            self.bridge(INT, HASH_CODE, &format!("{GET_HASH_CODE}()"));
        } else if let Some(fields) = &fields {
            let hash = format!("{OBJECTS}.{HASH}({})", fields.join(", "));
            self.bridge(INT, HASH_CODE, &hash);
        }
        if own_method(body, YS_TO_STRING, 0) {
            self.bridge(STRING, TO_STRING, &format!("{YS_TO_STRING}()"));
        }
    }

    /// `equals`: another `name` (`that`) for which `same` holds.
    fn equals(&mut self, name: &str, same: &str) {
        self.code.line(OVERRIDE);
        self.code
            .open(format!("{PUBLIC} {BOOLEAN} {EQUALS}({OBJECT} {OTHER})"));
        self.code.line(format!(
            "{RETURN} {OTHER} {INSTANCEOF} {name} that && {same};"
        ));
        self.code.close("");
    }

    /// Java's `java()`, giving `value`.
    fn bridge(&mut self, ret: &str, java: &str, value: &str) {
        self.code.line(OVERRIDE);
        self.code.open(format!("{PUBLIC} {ret} {java}()"));
        self.code.line(format!("{RETURN} {value};"));
        self.code.close("");
    }
}

/// `a && b && …`; `true` of none.
fn all_of(conditions: &[String]) -> String {
    if conditions.is_empty() {
        TRUE.to_string()
    } else {
        conditions.join(" && ")
    }
}

/// Whether `body` declares method `name` of `params` parameters, its own
/// (not static).
fn own_method(body: &[Decl], name: &str, params: usize) -> bool {
    body.iter().any(|d| {
        matches!(&d.kind, DeclKind::Method(Method { name: n, params: p, .. })
            if n.text == name && p.len() == params)
            && !d.has(Modifier::Static)
    })
}
