//! The System library's names (`yog-script-analyze/src/system/**`): its
//! namespaces, the types and attributes the language and its tools know.

/// Every namespace of the library and of what a source reaches starts
/// here: `System`, `System.Yog` (Yog's objects), `System.Game` (the game's
/// classes, a `.yns`'s).
pub const ROOT: &str = "System";

/// `System.Collections.Generic`: every file's `using`, as .NET's implicit ones.
pub const COLLECTIONS_GENERIC: [&str; 3] = [ROOT, "Collections", "Generic"];

/// `System.Linq`, `System.Text`: every file's `using`s too.
pub const LINQ: [&str; 2] = [ROOT, "Linq"];
pub const TEXT: [&str; 2] = [ROOT, "Text"];

/// What a namespace's names are joined with (`System.Collections`).
pub const NAMESPACE_SEP: &str = ".";

/// What separates a library source's folders (`System/Meta/Item.ys`):
/// its folders are its namespace.
pub const FOLDER_SEP: char = '/';

/// The enum whose cases a source calls without its name (`Ok(v)`,
/// `Err(e)`), as Rust's prelude brings them.
pub const RESULT: &str = "Result";

/// The base of every attribute class.
pub const ATTRIBUTE: &str = "Attribute";

/// The attribute that runs derive macros on a type: `[Derive(Clone)]`.
pub const DERIVE: &str = "Derive";

/// What a derive macro is given and its fields (`System.Meta`).
pub const ITEM: &str = "Item";
pub const FIELD: &str = "Field";

/// The attributes marking macros' methods (`System.Meta`).
pub const PROC_MACRO: &str = "ProcMacro";
pub const DERIVE_MACRO: &str = "DeriveMacro";

/// The attribute marking a runtime method whose type parameters are
/// known when it runs (`OfType<T>`): on the JVM, handed to it.
pub const REIFIED: &str = "Reified";

/// The attribute marking an attribute class as an attribute macro.
pub const MACRO: &str = "Macro";

/// The attribute marking a library type the runtime implements by hand
/// (.NET's `[Intrinsic]`): no code is written for it.
pub const INTRINSIC: &str = "Intrinsic";

/// The attribute marking a built-in type's members (`System.String` for
/// `string`): the runtime runs them, on a companion of statics.
pub const HOST: &str = "Host";

/// The delegates a lambda's own type is named after, as C# 10's natural
/// types: `Action<A, B>` gives nothing, `Func<A, TResult>` a `TResult`.
pub const ACTION: &str = "Action";
pub const FUNC: &str = "Func";

/// Numbers' functions (`Math.Max`).
pub const MATH: &str = "Math";

/// The collections Yog's values come as: a list (`List<T>`), a map
/// (`Dictionary<string, T>`); what `foreach` goes through (`IEnumerable<T>`);
/// a dictionary's item.
pub const LIST: &str = "List";
pub const DICTIONARY: &str = "Dictionary";
pub const ENUMERABLE: &str = "IEnumerable";
pub const KEY_VALUE_PAIR: &str = "KeyValuePair";

/// `System.Array`: every array's members and statics (`Length`, `Sort`).
pub const ARRAY: &str = "Array";

/// The generic interfaces a one-dimensional `T[]` is, as C#'s.
pub const ARRAY_INTERFACES: [&str; 5] = [
    "IList",
    "ICollection",
    ENUMERABLE,
    "IReadOnlyList",
    "IReadOnlyCollection",
];

/// What an array goes through where one of its interfaces is wanted:
/// `System.Array`'s view of it as that (an array is its own interfaces on
/// the CLR; the JVM's needs one).
pub const ARRAY_VIEW: &str = "AsList";

/// What a screen's markup base is: `screen ChargerScreen(…) : Ui("charger.yui")`.
pub const UI: &str = "Ui";

/// What an event's arguments' type is named after it (`JoinArgs`), as
/// C#'s `EventArgs`.
pub const ARGS_SUFFIX: &str = "Args";

/// The bus's attributes (`System.Yog.Native`): a call, an event, a method provided.
pub const SERVICE: &str = "Service";
pub const EVENT: &str = "Event";
pub const PROVIDE: &str = "Provide";
