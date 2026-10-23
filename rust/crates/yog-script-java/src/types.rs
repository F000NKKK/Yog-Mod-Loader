//! Yog Script's types as Java's: numbers by their width (boxed where
//! Java's generics want an object), collections as `java.util`'s, the
//! library's as the runtime's (`dev.yog.script`), delegates as
//! `java.util.function`'s.

use yog_script_analyze::Type;

/// Where the compiled code's runtime is (`java-common`'s `dev.yog.script`).
pub(crate) const RUNTIME: &str = "dev.yog.script";
/// The library's types the runtime gives, by their Yog Script names.
const LIBRARY: &[&str] = &["Result", "Error"];

/// `t` in Java; `boxed`: where only an object goes (a type argument).
pub(crate) fn java(t: &Type, boxed: bool) -> String {
    if let Some(p) = primitive(t) {
        return if boxed {
            boxed_name(p).to_string()
        } else {
            p.to_string()
        };
    }
    match t {
        Type::String | Type::Text | Type::Content(_) | Type::Tag => "String".into(),
        Type::Decimal => "java.math.BigDecimal".into(),
        Type::Void => if boxed { "Void" } else { "void" }.into(),
        Type::Unit(_) => if boxed { "Double" } else { "double" }.into(),
        Type::Own(name, args) => generic(&own(name), args),
        Type::Static(name) | Type::Param(name) => own(name),
        Type::Game(class) => class.replace(['/', '$'], "."),
        Type::List(item) => generic("java.util.List", std::slice::from_ref(item)),
        Type::Map(k, v) => generic("java.util.Map", &[(**k).clone(), (**v).clone()]),
        Type::Nullable(inner) => java(inner, true),
        Type::Fn(params, ret) => delegate(params, ret).0,
        _ => "Object".into(),
    }
}

/// An own type's Java name: the runtime's for the library's.
pub(crate) fn own(name: &str) -> String {
    if LIBRARY.contains(&name) {
        format!("{RUNTIME}.{name}")
    } else {
        name.to_string()
    }
}

fn generic(base: &str, args: &[Type]) -> String {
    if args.is_empty() {
        return base.to_string();
    }
    let args: Vec<String> = args.iter().map(|a| java(a, true)).collect();
    format!("{base}<{}>", args.join(", "))
}

/// A delegate's Java interface and the method that calls it.
pub(crate) fn delegate(params: &[Type], ret: &Type) -> (String, &'static str) {
    let args: Vec<String> = params.iter().map(|p| java(p, true)).collect();
    let r = java(ret, true);
    let f = "java.util.function";
    match (params.len(), *ret == Type::Void) {
        (0, true) => ("Runnable".into(), "run"),
        (0, false) => (format!("{f}.Supplier<{r}>"), "get"),
        (1, true) => (format!("{f}.Consumer<{}>", args[0]), "accept"),
        (1, false) => (format!("{f}.Function<{}, {r}>", args[0]), "apply"),
        (2, true) => (
            format!("{f}.BiConsumer<{}, {}>", args[0], args[1]),
            "accept",
        ),
        _ => (
            format!(
                "{f}.BiFunction<{}, {}, {r}>",
                args[0],
                args.get(1).cloned().unwrap_or_default()
            ),
            "apply",
        ),
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

/// A type as written (a signature's, a field's) in Java.
pub(crate) fn written(t: &yog_script_analyze::ast::Type, boxed: bool) -> String {
    use yog_script_analyze::ast::TypeKind;
    match &t.kind {
        TypeKind::Nullable(inner) => written(inner, true),
        TypeKind::Borrow { ty, .. } => written(ty, boxed),
        TypeKind::Path { path, args } => {
            let name = path
                .iter()
                .map(|n| n.text.as_str())
                .collect::<Vec<_>>()
                .join(".");
            let inner: Vec<String> = args.iter().map(|a| written(a, true)).collect();
            let with = |base: String| {
                if inner.is_empty() {
                    base
                } else {
                    format!("{base}<{}>", inner.join(", "))
                }
            };
            match by_name(&name, t) {
                Some(Named::Type(ty)) => java(&ty, boxed),
                Some(Named::Generic(base)) => with(base),
                None => with(own(&name)),
            }
        }
        _ => "Object".into(),
    }
}

fn written_args(t: &yog_script_analyze::ast::Type) -> Vec<&yog_script_analyze::ast::Type> {
    match &t.kind {
        yog_script_analyze::ast::TypeKind::Path { args, .. } => args.iter().collect(),
        _ => Vec::new(),
    }
}

/// What a written name is: a built-in type, or a generic one of `java.util`'s
/// or `java.util.function`'s.
enum Named {
    Type(Type),
    Generic(String),
}

fn by_name(name: &str, t: &yog_script_analyze::ast::Type) -> Option<Named> {
    use yog_script_analyze::words;
    let builtins: [(&words::Word, Type); 17] = [
        (&words::i8::WORD, Type::I8),
        (&words::i16::WORD, Type::I16),
        (&words::i32::WORD, Type::I32),
        (&words::i64::WORD, Type::I64),
        (&words::u8::WORD, Type::U8),
        (&words::u16::WORD, Type::U16),
        (&words::u32::WORD, Type::U32),
        (&words::u64::WORD, Type::U64),
        (&words::f32::WORD, Type::F32),
        (&words::f64::WORD, Type::F64),
        (&words::decimal::WORD, Type::Decimal),
        (&words::bool::WORD, Type::Bool),
        (&words::char::WORD, Type::Char),
        (&words::string::WORD, Type::String),
        (&words::object::WORD, Type::Object),
        (&words::dynamic::WORD, Type::Dynamic),
        (&words::void::WORD, Type::Void),
    ];
    if let Some((_, ty)) = builtins.into_iter().find(|(w, _)| w.text == name) {
        return Some(Named::Type(ty));
    }
    let n = written_args(t).len();
    let generic = match (name, n) {
        (LIST, _) => "java.util.List",
        (MAP, _) => "java.util.Map",
        (SET, _) => "java.util.Set",
        (FUNC, 1) => "java.util.function.Supplier",
        (FUNC, 2) => "java.util.function.Function",
        (FUNC, 3) => "java.util.function.BiFunction",
        (ACTION, 0) => "Runnable",
        (ACTION, 1) => "java.util.function.Consumer",
        (ACTION, 2) => "java.util.function.BiConsumer",
        _ => return None,
    };
    Some(Named::Generic(generic.to_string()))
}

/// The library's collections and delegates, by the names a source writes.
const LIST: &str = "List";
const MAP: &str = "Dictionary";
const SET: &str = "HashSet";
const FUNC: &str = "Func";
const ACTION: &str = "Action";
