//! Yog Script's types as Java's: numbers by their width (boxed where
//! Java's generics want an object), the source's and the library's types
//! where they are (`Index::place`), a lambda's own type as the library's
//! `Action` or `Func` of its arity, a game's array as Java's.

use yog_script_ids::system::CONTENT_ID;
use yog_script_analyze::ast::{Type as Written, TypeKind};
use yog_script_analyze::{Type, builtin_type, system_type, yog};

use super::Gen;

use yog_script_ids::java::{
    BOOLEAN, BOXED, BOXED_VOID, BYTE, CHAR, DOUBLE, FLOAT, INT, LONG, NEW, SHORT, VOID,
};
pub(crate) use yog_script_ids::jvm::RUNTIME;
use yog_script_ids::jvm::{BIG_DECIMAL, OBJECT, STRING};

impl Gen<'_> {
    /// `t` in Java; `boxed`: where only an object goes (a type argument).
    pub(crate) fn java(&self, t: &Type, boxed: bool) -> String {
        if let Some(p) = primitive(t) {
            return if boxed { boxed_name(p) } else { p }.to_string();
        }
        match t {
            Type::String | Type::Content(_) | Type::Tag => STRING.into(),
            Type::Decimal => BIG_DECIMAL.into(),
            Type::Void => if boxed { BOXED_VOID } else { VOID }.into(),
            Type::Unit(_) => self.java(&Type::F64, boxed),
            // A `ContentId<T>` is its text, as an `ItemId` is.
            Type::Own(name, _) if name == CONTENT_ID => STRING.into(),
            Type::Own(name, args) => self.generic(&self.index.place(name, args.len()), args),
            Type::Static(name, args) if args.is_empty() => self.index.place_named(name),
            Type::Static(name, args) => self.index.place(name, args.len()),
            Type::Param(name) => name.clone(),
            Type::Game(class) => class.replace(['/', '$'], "."),
            Type::Array(item, rank) => {
                format!("{}{}", self.java(item, false), crate::arrays::ranks(*rank))
            }
            Type::Nullable(inner) => self.java(inner, true),
            Type::Fn(params, ret) => self.fn_type(params, ret),
            Type::Tuple(items, _) => self.generic(&self.tuple_place(items.len()), items),
            // A handle is its subject; a payload its record.
            Type::Yog(name) if yog::is_handle(name) => STRING.into(),
            Type::Yog(name) => yog::data(name).unwrap_or_else(|| OBJECT.into()),
            _ => OBJECT.into(),
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
        let name = if gives {
            yog_script_ids::system::FUNC
        } else {
            yog_script_ids::system::ACTION
        };
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

    /// Type `name` (of `arity` arguments) as written, when it's Yog's — a
    /// payload's struct or a handle — and no type the source or the library
    /// declares is named so.
    pub(crate) fn yog_written(&self, name: &str, arity: usize) -> Option<Type> {
        let yogs = yog::is_handle(name) || yog::data(name).is_some();
        (arity == 0 && yogs && self.index.kind(&(name.to_string(), 0)).is_none())
            .then(|| Type::Yog(name.to_string()))
    }

    /// A type as written (a signature's, a field's) in Java.
    pub(crate) fn written(&self, t: &Written, boxed: bool) -> String {
        match &t.kind {
            TypeKind::Nullable(inner) => self.written(inner, true),
            TypeKind::Array { item, rank } => self.written_array(item, *rank),
            TypeKind::Borrow { ty, .. } => self.written(ty, boxed),
            TypeKind::Tuple(items) => {
                let items: Vec<String> = items.iter().map(|(t, _)| self.written(t, true)).collect();
                format!("{}<{}>", self.tuple_place(items.len()), items.join(", "))
            }
            // A path out of the library (a game class's, in a `.yns`) as written.
            TypeKind::Path { path, .. }
                if path.len() > 1 && path[0].text != yog_script_ids::system::ROOT =>
            {
                path.iter()
                    .map(|n| n.text.as_str())
                    .collect::<Vec<_>>()
                    .join(".")
            }
            TypeKind::Path { path, args } => {
                let name = path.last().map_or("", |n| n.text.as_str());
                // `i32`, or its type in `System` written (`Int32`, `Void`).
                if args.is_empty()
                    && let Some(b) = builtin_type(name).or_else(|| system_type(name))
                {
                    return self.java(&b, boxed);
                }
                if name == CONTENT_ID {
                    return STRING.into();
                }
                // A quantity (`Ticks`, a mod's `Energy`): a number.
                if args.is_empty() && self.index.is_unit(name) {
                    return self.java(&Type::Unit(name.to_string()), boxed);
                }
                // Yog's (`BlockPos`, a handle) where no type of the source's is named so.
                if let Some(t) = self.yog_written(name, args.len()) {
                    return self.java(&t, boxed);
                }
                let base = self.index.place(name, args.len());
                let inner: Vec<String> = args.iter().map(|a| self.written(a, true)).collect();
                if inner.is_empty() {
                    base
                } else {
                    format!("{base}<{}>", inner.join(", "))
                }
            }
            _ => OBJECT.into(),
        }
    }
}

/// A number, `bool` or `char` as Java's primitive.
pub(crate) fn primitive(t: &Type) -> Option<&'static str> {
    Some(match t {
        Type::I8 => BYTE,
        Type::I16 => SHORT,
        Type::I32 | Type::U8 | Type::U16 => INT,
        Type::I64 | Type::U32 | Type::U64 => LONG,
        Type::F32 => FLOAT,
        Type::F64 => DOUBLE,
        Type::Bool => BOOLEAN,
        Type::Char => CHAR,
        _ => return None,
    })
}

fn boxed_name(p: &str) -> &'static str {
    BOXED
        .iter()
        .find(|(primitive, _)| *primitive == p)
        .map_or(OBJECT, |(_, boxed)| boxed)
}

/// A new array of Java type `java` holding `items` (`{ a, b }`): made raw
/// and cast back when its item type is generic, so overloads still see it.
pub(crate) fn made_array(java: &str, items: &str) -> String {
    let made = crate::reified::erased(java);
    if made == java {
        format!("{NEW} {java} {items}")
    } else {
        format!("(({java}) {NEW} {made} {items})")
    }
}
