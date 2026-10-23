//! An obfuscated jar's names made Mojang's, by the side's ProGuard
//! mappings: each class, field and method (by its obfuscated descriptor),
//! and the classes in every descriptor.

use std::collections::HashMap;

use crate::classfile::ClassFile;
use yog_mappings::proguard::{self, Class};
use yog_script_analyze::index::Member;

/// Obfuscated names → Mojang's.
pub(super) struct Remap {
    classes: HashMap<String, String>,
    fields: HashMap<(String, String), String>,
    /// (class, name, descriptor), all obfuscated.
    methods: HashMap<(String, String, String), String>,
}

impl Remap {
    pub(super) fn new(mappings: &[Class]) -> Remap {
        let to_obf = proguard::class_map(mappings);
        let mut fields = HashMap::new();
        let mut methods = HashMap::new();
        for c in mappings {
            for (named, obf) in &c.fields {
                fields.insert((c.obf.clone(), obf.clone()), named.name.clone());
            }
            for (named, obf) in &c.methods {
                let desc = proguard::remap_desc(&named.desc, &to_obf);
                methods.insert((c.obf.clone(), obf.clone(), desc), named.name.clone());
            }
        }
        let classes = mappings
            .iter()
            .map(|c| (c.obf.clone(), c.named.clone()))
            .collect();
        Remap {
            classes,
            fields,
            methods,
        }
    }

    fn class(&self, name: &str) -> String {
        self.classes
            .get(name)
            .cloned()
            .unwrap_or_else(|| name.to_owned())
    }

    fn desc(&self, desc: &str) -> String {
        let mut out = String::with_capacity(desc.len());
        let mut rest = desc;
        while let Some(at) = rest.find('L') {
            out.push_str(&rest[..at]);
            let Some(end) = rest[at..].find(';') else {
                out.push_str(&rest[at..]);
                return out;
            };
            out.push('L');
            out.push_str(&self.class(&rest[at + 1..at + end]));
            out.push(';');
            rest = &rest[at + end + 1..];
        }
        out.push_str(rest);
        out
    }

    /// `c` with Mojang's names.
    pub(super) fn apply(&self, c: ClassFile) -> ClassFile {
        let field = |m: Member| Member {
            name: self
                .fields
                .get(&(c.name.clone(), m.name.clone()))
                .cloned()
                .unwrap_or(m.name),
            desc: self.desc(&m.desc),
            access: m.access,
        };
        let method = |m: Member| Member {
            name: self
                .methods
                .get(&(c.name.clone(), m.name.clone(), m.desc.clone()))
                .cloned()
                .unwrap_or(m.name),
            desc: self.desc(&m.desc),
            access: m.access,
        };
        ClassFile {
            name: self.class(&c.name),
            access: c.access,
            extends: c.extends.as_deref().map(|e| self.class(e)),
            implements: c.implements.iter().map(|i| self.class(i)).collect(),
            fields: c.fields.clone().into_iter().map(field).collect(),
            methods: c.methods.clone().into_iter().map(method).collect(),
        }
    }
}
