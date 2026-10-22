//! Mojang's mappings (ProGuard's text): the game's classes and members by
//! Mojang's names, and their obfuscated names.

use std::collections::HashMap;

/// A member by Mojang's names: its name and, for a method, descriptor.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct Named {
    pub(crate) name: String,
    /// A method's descriptor in Mojang's names; empty for a field.
    pub(crate) desc: String,
}

/// A class: its obfuscated name and its members' (internal names).
#[derive(Debug, Default)]
pub(crate) struct Class {
    pub(crate) named: String,
    pub(crate) obf: String,
    pub(crate) fields: Vec<(Named, String)>,
    pub(crate) methods: Vec<(Named, String)>,
}

/// What separates a name from its obfuscated one.
const ARROW: &str = " -> ";
/// What ends a class's line.
const CLASS_END: char = ':';
/// What starts a comment.
const COMMENT: char = '#';

/// `java.lang.String` → `java/lang/String`.
pub(crate) fn internal(dotted: &str) -> String {
    dotted.replace('.', "/")
}

/// A Java type as written (`int[]`, `net.minecraft.Foo`) → its descriptor.
pub(crate) fn type_desc(ty: &str) -> String {
    let (base, dims) = match ty.find('[') {
        Some(at) => (&ty[..at], (ty.len() - at) / 2),
        None => (ty, 0),
    };
    let one = match base {
        "void" => "V".to_owned(),
        "boolean" => "Z".to_owned(),
        "byte" => "B".to_owned(),
        "char" => "C".to_owned(),
        "short" => "S".to_owned(),
        "int" => "I".to_owned(),
        "long" => "J".to_owned(),
        "float" => "F".to_owned(),
        "double" => "D".to_owned(),
        class => format!("L{};", internal(class)),
    };
    "[".repeat(dims) + &one
}

/// A method's line (`12:14:boolean hurt(a.B,float)`, line numbers optional)
/// → its name and descriptor.
fn method(sig: &str) -> Option<Named> {
    let sig = sig.rsplit(':').next()?;
    let (ret, rest) = sig.split_once(' ')?;
    let (name, args) = rest.split_once('(')?;
    let args = args.strip_suffix(')')?;
    let params: String = args
        .split(',')
        .filter(|a| !a.is_empty())
        .map(type_desc)
        .collect();
    Some(Named {
        name: name.to_owned(),
        desc: format!("({params}){}", type_desc(ret)),
    })
}

/// Every class of `text`.
pub(crate) fn parse(text: &str) -> Vec<Class> {
    let mut out: Vec<Class> = Vec::new();
    for line in text.lines() {
        if line.starts_with(COMMENT) || line.trim().is_empty() {
            continue;
        }
        let Some((left, obf)) = line.split_once(ARROW) else {
            continue;
        };
        if !line.starts_with(' ') {
            out.push(Class {
                named: internal(left),
                obf: internal(obf.trim_end_matches(CLASS_END)),
                ..Class::default()
            });
            continue;
        }
        let Some(class) = out.last_mut() else {
            continue;
        };
        let left = left.trim();
        if left.contains('(') {
            if let Some(m) = method(left) {
                class.methods.push((m, obf.to_owned()));
            }
        } else if let Some((_, name)) = left.split_once(' ') {
            let field = Named {
                name: name.to_owned(),
                desc: String::new(),
            };
            class.fields.push((field, obf.to_owned()));
        }
    }
    out
}

/// Named class → obfuscated, for descriptors.
pub(crate) fn class_map(classes: &[Class]) -> HashMap<&str, &str> {
    classes
        .iter()
        .map(|c| (c.named.as_str(), c.obf.as_str()))
        .collect()
}

/// `desc` (Mojang's names) with its classes renamed by `map`.
pub(crate) fn remap_desc(desc: &str, map: &HashMap<&str, &str>) -> String {
    let mut out = String::with_capacity(desc.len());
    let mut rest = desc;
    while let Some(at) = rest.find('L') {
        out.push_str(&rest[..at]);
        let Some(end) = rest[at..].find(';') else {
            out.push_str(&rest[at..]);
            return out;
        };
        let name = &rest[at + 1..at + end];
        out.push('L');
        out.push_str(map.get(name).copied().unwrap_or(name));
        out.push(';');
        rest = &rest[at + end + 1..];
    }
    out.push_str(rest);
    out
}
