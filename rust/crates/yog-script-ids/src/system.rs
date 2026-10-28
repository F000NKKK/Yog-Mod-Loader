//! The System library's names (`yog-script-analyze/src/system/**`): its
//! namespaces, the types and attributes the language and its tools know.

/// The language's library's root (`System`, `System.Collections`, …) and
/// the game's classes' (`System.Game`, a `.yns`'s); Yog's own are beside
/// it, under `Yog` — the language never depends on Yog.
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
/// What an `async` method gives and `await` waits for.
pub const TASK: &str = "Task";
/// `Yog.GameTime`: the game's ticks (its `Delay`, `NextTick`).
pub const GAME_TIME: &str = "GameTime";
/// `Yog.Text`: what a player reads, a translation key and its
/// arguments (`t"key"`'s type).
pub const TEXT_TYPE: &str = "Text";
/// What `lock` holds: `System.Threading.Lock<T>`.
pub const LOCK: &str = "Lock";
/// What holds something to let go of: dropped at its owner's scope's end.
pub const DISPOSABLE: &str = "IDisposable";
/// What a mod's content inherits: a block's, an item's.
pub const BLOCK_OBJECT: &str = "BlockObject";
pub const ITEM_OBJECT: &str = "ItemObject";
pub const ENTITY_OBJECT: &str = "EntityObject";
/// Who ran a command.
pub const SENDER: &str = "Sender";
pub const SENDER_REPLY: &str = "Reply";

/// A value type's `T?` as Reflection names it (`Nullable<Int32>`).
pub const NULLABLE: &str = "Nullable";

/// What `typeof(T)` and `x.GetType()` give: a type as a value (`System.Type`).
pub const TYPE: &str = type_name!();

/// `System.Type`'s name as a literal, for names composed of it (`concat!`).
macro_rules! type_name {
    () => {
        "Type"
    };
}
pub(crate) use type_name;

/// An expression tree of a delegate (`System.Linq.Expressions.Expression<TDelegate>`):
/// a lambda given where one is wanted is quoted.
pub const EXPRESSION: &str = "Expression";
/// What an expression tree's node is (`ExpressionType.Add`).
pub const EXPRESSION_TYPE: &str = "ExpressionType";

/// Every enum of values' base (`System.Enum`), and the attribute making
/// one's values bits (`[Flags]`).
pub const ENUM: &str = "Enum";
pub const FLAGS: &str = "Flags";

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
/// What an iterator may give besides an `IEnumerable<T>`: its walk, an
/// async sequence.
pub const ENUMERATOR: &str = "IEnumerator";
pub const ASYNC_ENUMERABLE: &str = "IAsyncEnumerable";
pub const ASYNC_ENUMERATOR: &str = "IAsyncEnumerator";
pub const KEY_VALUE_PAIR: &str = "KeyValuePair";

/// A tuple's type (`ValueTuple<T1, …, T7>`), as many items as it may hold.
pub const VALUE_TUPLE: &str = "ValueTuple";
pub const TUPLE_ITEMS: usize = 7;

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

/// The bus's attributes (`Yog.Native`): a call, an event, a method provided.
pub const SERVICE: &str = "Service";
pub const EVENT: &str = "Event";
pub const PROVIDE: &str = "Provide";

/// How values cross threads (`System.Threading.ThreadSafety`'s file,
/// `ys-concurrency.md` §2): the attributes and their enums' cases.
pub mod threads {
    /// `[Threads(ThreadSafety.…)]`: how a type crosses, where its fields don't say.
    pub const THREADS: &str = "Threads";
    pub const GUARDED: &str = "Guarded";
    pub const SHARED: &str = "Shared";
    pub const BOUND: &str = "Bound";
    /// `[Crosses("work", Crossing.…)]`: how what a parameter takes crosses.
    pub const CROSSES: &str = "Crosses";
    pub const MOVED: &str = "Moved";
    pub const SCOPED: &str = "Scoped";
    pub const CONCURRENT: &str = "Concurrent";
    /// `[ThreadStatic]`: a static field each thread has its own of.
    pub const THREAD_STATIC: &str = "ThreadStatic";
}

/// `[StructuredMessageTemplate(nameof(message))]`: a method's parameter a
/// message template (`System.Diagnostics.CodeAnalysis`).
pub const STRUCTURED_MESSAGE_TEMPLATE: &str = "StructuredMessageTemplate";

/// What `[LoggerMessage]` generates a `partial` method's body by (.NET's
/// `LoggerMessageAttribute` source generator): the attribute, its
/// arguments' names, and the parameters' types it looks for.
pub const LOGGER_MESSAGE: &str = "LoggerMessage";
/// The class of `LogInformation` & co.: a call of its a log a
/// `[LoggerMessage]` method writes faster (a warning, .NET's CA1848).
pub const LOGGER_EXTENSIONS: &str = "LoggerExtensions";

/// Content's ids, typed by kind (`ItemId`, `BlockId`, … each a kind's
/// name and this suffix), all a `ContentId<TSelf>`: `Yog`'s.
pub const CONTENT_ID: &str = "ContentId";
pub const CONTENT_ID_SUFFIX: &str = "Id";
/// A tag's id, of the kind of id it tags: `TagId<ItemId>`, a `ContentId` too.
pub const TAG_ID: &str = "TagId";

/// The attribute of a constructor that sets its type's `required` members.
pub const SETS_REQUIRED_MEMBERS: &str = "SetsRequiredMembers";
pub const LOGGER_MESSAGE_EVENT_ID: &str = "eventId";
pub const LOGGER_MESSAGE_LEVEL: &str = "level";
pub const LOGGER_MESSAGE_TEXT: &str = "message";
pub const LOGGER: &str = "ILogger";
pub const LOG_LEVEL: &str = "LogLevel";
pub const ERROR: &str = "Error";
/// What a generated method's state class is named after it: `MovedLogValues`.
pub const LOG_VALUES_SUFFIX: &str = "LogValues";
/// What a generated method's event id field is named after it: `_movedEventId`.
pub const EVENT_ID_SUFFIX: &str = "EventId";
