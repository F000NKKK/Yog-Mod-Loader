//! A `saved` member of an enum carrying data (`Shape.Line(i32 Len)`):
//! through the codec as its case's name and its data's bytes (a record of
//! the case's fields) — what writes it is a chain of the cases with data,
//! what reads it back makes the case its name says (one without data by
//! `Enum.Parse`). By name, so reordering the cases keeps old saves.

use yog_script_analyze::ast::{Type as Written, TypeKind};
use yog_script_ids::java::{
    CASE as JAVA_CASE, CLASS, IF, INSTANCEOF, NEW, PRIVATE, PUBLIC, RECORD, RETURN, STATIC, SWITCH,
};
use yog_script_ids::jvm::STRING;
use yog_script_ids::jvm::state::{
    BYTES, CASE, CASE_DATA, CASE_VAR, ENUM_PARSE, FROM_BYTES, LOADED_CASE, NO_BYTES, RECORD_PREFIX,
    SAVED_CASE, TO_STRING, VALUE_SUFFIX, VALUE_VAR,
};

use super::Gen;
use crate::index::Kind;
use crate::saved::Kept;

impl Gen<'_> {
    /// Own enum `ty` names, when some of its cases carry data.
    pub(crate) fn data_enum_of(&self, ty: &Written) -> Option<String> {
        let TypeKind::Path { path, args } = &ty.kind else {
            return None;
        };
        let name = path.last()?.text.clone();
        let is_enum = args.is_empty() && self.index.kind(&(name.clone(), 0)) == Some(Kind::Enum);
        (is_enum && !self.index.data_cases(&name).is_empty()).then_some(name)
    }

    /// Member `member`'s form, of enum `of` (written `ty`) carrying data.
    pub(crate) fn data_enum_form(&self, ty: &Written, of: &str, member: &str) -> Option<Kept> {
        let cases = self.index.data_cases(of);
        if !cases
            .iter()
            .all(|(_, _, types)| types.iter().all(|t| self.plain(t)))
        {
            return None;
        }
        let java = self.written(ty, false);
        let value = format!("{RECORD_PREFIX}{member}{VALUE_SUFFIX}");
        let (saved, loaded) = (
            format!("{SAVED_CASE}{member}"),
            format!("{LOADED_CASE}{member}"),
        );
        let mut declared = vec![format!(
            "{PUBLIC} {RECORD} {value}({STRING} {CASE}, byte[] {CASE_DATA}) {{}}"
        )];
        let mut writes = Vec::new();
        let mut reads = Vec::new();
        for (case, fields, types) in &cases {
            let record = format!("{RECORD_PREFIX}{member}{case}");
            let params: Vec<String> = fields
                .iter()
                .zip(types.iter())
                .map(|(f, t)| format!("{} {}", self.written(t, false), Self::ident(f)))
                .collect();
            declared.push(format!(
                "{PUBLIC} {RECORD} {record}({}) {{}}",
                params.join(", ")
            ));
            let name = self.code.str(case);
            let from: Vec<String> = fields
                .iter()
                .map(|f| format!("{CASE_VAR}.{}", Self::ident(f)))
                .collect();
            writes.push(format!(
                "{IF} ({VALUE_VAR} {INSTANCEOF} {java}.{case} {CASE_VAR}) {RETURN} {NEW} {value}({name}, {BYTES}({NEW} {record}({})));",
                from.join(", ")
            ));
            let back: Vec<String> = fields
                .iter()
                .map(|f| format!("{CASE_VAR}.{}()", Self::ident(f)))
                .collect();
            reads.push(format!(
                "{JAVA_CASE} {name}: {{ var {CASE_VAR} = {FROM_BYTES}({VALUE_VAR}.{CASE_DATA}(), {record}.{CLASS}); {RETURN} {NEW} {java}.{case}({}); }}",
                back.join(", ")
            ));
        }
        declared.push(format!(
            "{PRIVATE} {STATIC} {value} {saved}({java} {VALUE_VAR}) {{"
        ));
        declared.extend(writes);
        declared.push(format!(
            "{RETURN} {NEW} {value}({VALUE_VAR}.{TO_STRING}(), {NO_BYTES}); }}"
        ));
        declared.push(format!("{PRIVATE} {STATIC} {java} {loaded}({value} {VALUE_VAR}) {{ {SWITCH} ({VALUE_VAR}.{CASE}()) {{"));
        declared.extend(reads);
        declared.push(format!(
            "}} {RETURN} ({java}) {ENUM_PARSE}({java}.{CLASS}, {VALUE_VAR}.{CASE}()); }}"
        ));
        Some(Kept {
            declared,
            java: value,
            save: Box::new(move |f| format!("{saved}({f})")),
            load: Box::new(move |f, v| format!("{f} = {loaded}({v});")),
        })
    }
}
