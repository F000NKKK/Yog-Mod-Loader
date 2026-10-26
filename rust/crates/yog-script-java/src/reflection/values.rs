//! A type as a value in Java: `Type.Of(X.class)`, a generic one made of
//! its arguments (`.MakeGenericType(…)`), an array of its item's
//! (`.MakeArrayType(rank)`).

use yog_script_analyze::ast::{Type as Written, TypeKind};
use yog_script_analyze::{Type, builtin_type, system_name, system_type};
use yog_script_ids::jvm::reflection::{OF_TYPE, TYPE};
use yog_script_ids::members::{MAKE_ARRAY_TYPE, MAKE_GENERIC_TYPE};
use yog_script_ids::system::NULLABLE;

use crate::Gen;
use crate::index::Kind;

/// What names a Java class as a value.
const CLASS: &str = ".class";

impl Gen<'_> {
    /// Written type `t` as a `Type`; a type parameter as `param` reaches
    /// it (by its name), or nothing when it can't be.
    pub(crate) fn type_value(
        &self,
        t: &Written,
        param: &dyn Fn(&str) -> Option<String>,
    ) -> Option<String> {
        match &t.kind {
            TypeKind::Path { path, args } => {
                let name = path.last().map_or("", |n| n.text.as_str());
                if args.is_empty() {
                    if let Some(p) = param(name) {
                        return Some(p);
                    }
                    if let Some(b) = builtin_type(name).or_else(|| system_type(name)) {
                        return self.value_of(&b);
                    }
                }
                // A game's class, as written (`net.minecraft.world.Item`).
                if path.len() > 1 && path[0].text != yog_script_ids::system::ROOT {
                    let class: Vec<&str> = path.iter().map(|n| n.text.as_str()).collect();
                    return Some(of_class(&class.join(".")));
                }
                let args = args
                    .iter()
                    .map(|a| self.type_value(a, param))
                    .collect::<Option<Vec<_>>>()?;
                Some(made_of(
                    of_class(&self.index.place(name, args.len())),
                    &args,
                ))
            }
            // A value type's `?` is `Nullable<T>`; a reference's, the type.
            TypeKind::Nullable(inner) => {
                let v = self.type_value(inner, param)?;
                Some(if self.written_value_type(inner) {
                    self.nullable(v)
                } else {
                    v
                })
            }
            TypeKind::Array { item, rank } => self
                .type_value(item, param)
                .map(|i| format!("{i}.{MAKE_ARRAY_TYPE}({rank})")),
            TypeKind::Borrow { ty, .. } => self.type_value(ty, param),
            TypeKind::Tuple(items) => {
                let args = items
                    .iter()
                    .map(|(t, _)| self.type_value(t, param))
                    .collect::<Option<Vec<_>>>()?;
                Some(made_of(of_class(&self.tuple_place(args.len())), &args))
            }
            TypeKind::Both(_) | TypeKind::Lifetime(_) => None,
        }
    }

    /// Type `t` (as checked) as a `Type`, when it names no type parameter.
    pub(crate) fn value_of(&self, t: &Type) -> Option<String> {
        match t {
            Type::Param(_) | Type::Unknown => None,
            Type::Nullable(inner) => {
                let v = self.value_of(inner)?;
                Some(if self.value_type(inner) {
                    self.nullable(v)
                } else {
                    v
                })
            }
            Type::Array(item, rank) => self
                .value_of(item)
                .map(|i| format!("{i}.{MAKE_ARRAY_TYPE}({rank})")),
            Type::Own(name, args) | Type::Static(name, args) => {
                let args = args
                    .iter()
                    .map(|a| self.value_of(a))
                    .collect::<Option<Vec<_>>>()?;
                Some(made_of(
                    of_class(&self.index.place(name, args.len())),
                    &args,
                ))
            }
            Type::Tuple(items, _) => {
                let args = items
                    .iter()
                    .map(|a| self.value_of(a))
                    .collect::<Option<Vec<_>>>()?;
                Some(made_of(of_class(&self.tuple_place(args.len())), &args))
            }
            // A lambda's own type: `Action<…>` or `Func<…>`, as C# 10's.
            Type::Fn(params, ret) => {
                let gives = **ret != Type::Void;
                let all: Vec<&Type> = params.iter().chain(gives.then_some(&**ret)).collect();
                let args = all
                    .iter()
                    .map(|a| self.value_of(a))
                    .collect::<Option<Vec<_>>>()?;
                let name = if gives {
                    yog_script_ids::system::FUNC
                } else {
                    yog_script_ids::system::ACTION
                };
                Some(made_of(
                    of_class(&self.index.place(name, args.len())),
                    &args,
                ))
            }
            // A built-in type: its type in `System` (`Int32` for `i32`), by
            // its companion's class.
            builtin => {
                let name = system_name(builtin)?;
                Some(of_class(&self.class_of(name, 0)))
            }
        }
    }
}

impl Gen<'_> {
    /// `Nullable<T>` of type value `inner`.
    fn nullable(&self, inner: String) -> String {
        made_of(of_class(&self.index.place(NULLABLE, 1)), &[inner])
    }

    /// Whether `t` is a value type: a number, `bool`, `char`, `decimal`, a
    /// struct, an enum, a tuple.
    fn value_type(&self, t: &Type) -> bool {
        match t {
            Type::Own(name, args) => matches!(
                self.index.kind(&(name.clone(), args.len())),
                Some(Kind::Struct | Kind::Enum)
            ),
            Type::Tuple(..) | Type::Decimal => true,
            other => crate::types::primitive(other).is_some(),
        }
    }

    /// Whether written type `t` is a value type.
    fn written_value_type(&self, t: &Written) -> bool {
        match &t.kind {
            TypeKind::Path { path, args } => {
                let name = path.last().map_or("", |n| n.text.as_str());
                match builtin_type(name).or_else(|| system_type(name)) {
                    Some(b) if args.is_empty() => self.value_type(&b),
                    _ => matches!(
                        self.index.kind(&(name.to_string(), args.len())),
                        Some(Kind::Struct | Kind::Enum)
                    ),
                }
            }
            TypeKind::Tuple(_) => true,
            _ => false,
        }
    }
}

/// `Type.Of(class.class)`.
pub(crate) fn of_class(class: &str) -> String {
    format!("{TYPE}.{OF_TYPE}({class}{CLASS})")
}

/// Definition `def` made of `args`, when there are any.
fn made_of(def: String, args: &[String]) -> String {
    if args.is_empty() {
        def
    } else {
        format!(
            "{def}.{MAKE_GENERIC_TYPE}(new {TYPE}[] {{ {} }})",
            args.join(", ")
        )
    }
}
