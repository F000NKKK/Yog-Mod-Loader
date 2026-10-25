//! Yog Script's types as Java's: numbers by their width (boxed where
//! Java's generics want an object), the source's and the library's types
//! where they are (`Index::place`), a lambda's own type as the library's
//! `Action` or `Func` of its arity, a game's array as Java's.

use yog_script_analyze::ast::{Type as Written, TypeKind};
use yog_script_analyze::{Type, builtin_type, system, yog};

use super::Gen;

/// Where the compiled code's runtime is (`java-api`'s `dev.yog.script`).
pub(crate) const RUNTIME: &str = "dev.yog.script";

impl Gen<'_> {
    /// `t` in Java; `boxed`: where only an object goes (a type argument).
    pub(crate) fn java(&self, t: &Type, boxed: bool) -> String {
        if let Some(p) = primitive(t) {
            return if boxed { boxed_name(p) } else { p }.to_string();
        }
        match t {
            Type::String | Type::Text | Type::Content(_) | Type::Tag => "String".into(),
            Type::Decimal => "java.math.BigDecimal".into(),
            Type::Void => if boxed { "Void" } else { "void" }.into(),
            Type::Unit(_) => if boxed { "Double" } else { "double" }.into(),
            Type::Own(name, args) => self.generic(&self.index.place(name, args.len()), args),
            Type::Static(name) => self.index.place_named(name),
            Type::Param(name) => name.clone(),
            Type::Game(class) => class.replace(['/', '$'], "."),
            Type::Array(item) => format!("{}[]", self.java(item, false)),
            Type::Nullable(inner) => self.java(inner, true),
            Type::Fn(params, ret) => self.fn_type(params, ret),
            // A handle is its subject; a payload its record.
            Type::Yog(name) if yog::is_handle(name) => "String".into(),
            Type::Yog(name) => yog::data(name).unwrap_or_else(|| "Object".into()),
            _ => "Object".into(),
        }
    }

    /// A lambda's or a method's own type: the library's `Action` (when it
    /// gives nothing) or `Func` of its arity, as C# 10's natural types.
    pub(crate) fn fn_type(&self, params: &[Type], ret: &Type) -> String {
        let gives = *ret != Type::Void;
        let mut args = params.to_vec();
        if gives {
            args.push(ret.clone());
        }
        let name = if gives { system::FUNC } else { system::ACTION };
        self.generic(&self.index.place(name, args.len()), &args)
    }

    /// Own type `name` (written without arguments) in Java.
    pub(crate) fn own(&self, name: &str) -> String {
        self.index.place_named(name)
    }

    fn generic(&self, base: &str, args: &[Type]) -> String {
        if args.is_empty() {
            return base.to_string();
        }
        let args: Vec<String> = args.iter().map(|a| self.java(a, true)).collect();
        format!("{base}<{}>", args.join(", "))
    }

    /// A type as written (a signature's, a field's) in Java.
    pub(crate) fn written(&self, t: &Written, boxed: bool) -> String {
        match &t.kind {
            TypeKind::Nullable(inner) => self.written(inner, true),
            TypeKind::Borrow { ty, .. } => self.written(ty, boxed),
            // A path out of the library (a game class's, in a `.yns`) as written.
            TypeKind::Path { path, .. } if path.len() > 1 && path[0].text != system::ROOT => path
                .iter()
                .map(|n| n.text.as_str())
                .collect::<Vec<_>>()
                .join("."),
            TypeKind::Path { path, args } => {
                let name = path.last().map_or("", |n| n.text.as_str());
                if args.is_empty()
                    && let Some(b) = builtin_type(name)
                {
                    return self.java(&b, boxed);
                }
                let base = self.index.place(name, args.len());
                let inner: Vec<String> = args.iter().map(|a| self.written(a, true)).collect();
                if inner.is_empty() {
                    base
                } else {
                    format!("{base}<{}>", inner.join(", "))
                }
            }
            _ => "Object".into(),
        }
    }
}

/// A number, `bool` or `char` as Java's primitive.
pub(crate) fn primitive(t: &Type) -> Option<&'static str> {
    Some(match t {
        Type::I8 => "byte",
        Type::I16 => "short",
        Type::I32 | Type::U8 | Type::U16 => "int",
        Type::I64 | Type::U32 | Type::U64 => "long",
        Type::F32 => "float",
        Type::F64 => "double",
        Type::Bool => "boolean",
        Type::Char => "char",
        _ => return None,
    })
}

fn boxed_name(p: &str) -> &'static str {
    match p {
        "byte" => "Byte",
        "short" => "Short",
        "int" => "Integer",
        "long" => "Long",
        "float" => "Float",
        "double" => "Double",
        "boolean" => "Boolean",
        _ => "Character",
    }
}
