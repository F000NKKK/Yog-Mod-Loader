//! Where Yog Script's runtime is on the JVM (`java-api`'s
//! `dev.yog.script`), and the names of it compiled code calls. A name of a
//! package or a class is composed of the one above it — Yog's Java root,
//! the runtime under it, Reflection under that — by literal macros.

use crate::system;
use crate::system::type_name;

/// Each part of the names below as a literal, a macro each, so `concat!`
/// joins them at build time: what joins a package's names, Yog's Java root
/// (`java-api`'s and `java-common`'s packages are under it), the runtime
/// under it, its Reflection, Java's packages compiled code names classes of.
macro_rules! sep {
    () => {
        "."
    };
}
macro_rules! java_root {
    () => {
        "dev.yog"
    };
}
macro_rules! runtime {
    () => {
        concat!(java_root!(), sep!(), "script")
    };
}
macro_rules! reflection {
    () => {
        concat!(runtime!(), sep!(), "reflection")
    };
}
macro_rules! java_util {
    () => {
        "java.util"
    };
}
macro_rules! java_math {
    () => {
        "java.math"
    };
}

/// What joins a Java package's names, a class's to it, a member's to a class.
pub const SEP: &str = sep!();

/// The runtime's package: the System library's, its sub-namespaces under
/// it by their lowercased names.
pub const RUNTIME: &str = runtime!();

/// Java's classes compiled code names.
pub const OBJECTS: &str = concat!(java_util!(), sep!(), "Objects");
pub const MAP: &str = concat!(java_util!(), sep!(), "Map");
/// `Yog.Text`'s text of a key (`t"key"`), and a text with its
/// arguments (`t"key"(a, b)`).
pub const TEXT_OF: &str = "Of";
pub const TEXT_WITH: &str = "With";
/// What runs a block of code in place of an expression (`java.lang`'s).
pub const RUNNABLE: &str = "Runnable";
pub const SUPPLIER: &str = concat!(java_util!(), sep!(), "function", sep!(), "Supplier");
/// The precision a `decimal`'s division keeps (.NET's 28–29 digits: 34).
pub const DIVISION_CONTEXT: &str =
    concat!(java_math!(), sep!(), "MathContext", sep!(), "DECIMAL128");
pub const ARRAY_LIST: &str = concat!(java_util!(), sep!(), "ArrayList");
/// Going through items: an `Iterable`'s iterator, whether it has one more,
/// the next; a string's chars as an array, an array's length.
pub const ITERATOR: &str = concat!(java_util!(), sep!(), "Iterator");
pub const ITERATOR_OF: &str = "iterator";
pub const HAS_NEXT: &str = "hasNext";
pub const NEXT: &str = "next";
pub const TO_CHAR_ARRAY: &str = "toCharArray";
pub const LENGTH: &str = "length";

/// Their members compiled code calls: `Objects`' equality, hash and
/// fallback, a map's entries.
pub const HASH: &str = "hash";
pub const REQUIRE_NON_NULL_ELSE: &str = "requireNonNullElse";
pub const ENTRY: &str = "entry";
pub const OF_ENTRIES: &str = "ofEntries";

/// What a Java-only helper of the runtime making or finding a value by its
/// key is named (`Library.of(c)`, `Mode.of(5)`); a library type's own
/// (`Type.Of(c)`, `Array.Of(items, int[].class)`) as its members are.
pub const OF: &str = "of";
pub const OF_MEMBER: &str = "Of";

/// The runtime's package of built-in types' members' companions
/// (`[Host]`), apart so `String` there doesn't hide Java's.
pub const HOST_PACKAGE: &str = "host";

/// What a temporary of compiled code is named with (`$t1`): a `$`, which
/// Yog Script's names never have.
pub const TEMP: &str = "$t";

/// The runtime's helpers compiled code calls (`Ys.at`, `Ys.noArm`).
pub const HELPERS: &str = "Ys";

/// The helper giving a value with another evaluated after it: a type's own
/// `x++`'s old value (`Ys.before(x, x = …)`), a number's `GetType()`.
pub const BEFORE: &str = "before";

/// A `ref` or `out` parameter's cell (`Ref<T>`), and its value.
pub const REF: &str = concat!(runtime!(), sep!(), "Ref");
pub const REF_VALUE: &str = "value";

/// The helper binding a pattern's name inside a condition
/// (`x instanceof Mode.Charging c && Ys.bound(r = c.Rate()) && r > 3`).
pub const BOUND: &str = "bound";

/// The helpers the built-in macros are: `panic!` (and `todo!`,
/// `unreachable!`), `assert!`,
/// `assert_eq!`, `dbg!`; a `switch` no arm takes; a value as text.
pub const PANIC: &str = "panic";
pub const CHECK: &str = "check";
pub const CHECK_EQ: &str = "checkEq";
pub const DBG: &str = "dbg";
pub const NO_ARM: &str = "noArm";
pub const VALUE_OF: &str = "valueOf";

/// What every mod's class implements, the runtime finding it by.
pub const MOD_INTERFACE: &str = concat!(runtime!(), sep!(), "YsMod");

/// What a source's class is named after its file: `Glassy` + this.
pub const CLASS_SUFFIX: &str = "Script";

/// The bus's call for Java code (`java-common`'s `dev.yog.api.YogBus`).
macro_rules! bus_api {
    () => {
        concat!(java_root!(), sep!(), "api", sep!(), "YogBus")
    };
}
pub const BUS_CALL: &str = concat!(bus_api!(), sep!(), "call");
/// A bus event emitted, listened to (at the default priority).
pub const BUS_EMIT: &str = concat!(bus_api!(), sep!(), "emit");
pub const BUS_ON: &str = concat!(bus_api!(), sep!(), "on");
pub const BUS_PRIORITY: &str = "0";
/// A `[Provide]` method answering at its address; the file's class doing
/// what it does when the mods load, made as a mod is.
pub const BUS_PROVIDE: &str = concat!(bus_api!(), sep!(), "provide");
pub const LOAD: &str = "$Load";
/// A content's objects made by its class once registered (`BlockObject.register`).
pub const OBJECTS_REGISTER: &str = "register";
/// A `command`'s class (`ChargeCommand`), its ways of running declared
/// (`Commands.declare`), each a wrapper (`$runFill1`) reading the arguments
/// the game parsed (a list of text).
pub const COMMAND_SUFFIX: &str = "Command";
pub const COMMANDS_DECLARE: &str = concat!(
    runtime!(),
    sep!(),
    "yog",
    sep!(),
    "Commands",
    sep!(),
    "declare"
);
pub const RUN_WAY: &str = "$run";
/// A call across sides: its method's arguments' record (`ShowChargeMessage`),
/// sent to a player's client or the server, run where it arrives.
pub const MESSAGE_SUFFIX: &str = "Message";
pub const MESSAGES_TO_CLIENT: &str = concat!(
    runtime!(),
    sep!(),
    "yog",
    sep!(),
    "Messages",
    sep!(),
    "toClient"
);
pub const MESSAGES_TO_SERVER: &str = concat!(
    runtime!(),
    sep!(),
    "yog",
    sep!(),
    "Messages",
    sep!(),
    "toServer"
);
pub const MESSAGES_ON_CLIENT: &str = concat!(
    runtime!(),
    sep!(),
    "yog",
    sep!(),
    "Messages",
    sep!(),
    "onClient"
);
pub const MESSAGES_ON_SERVER: &str = concat!(
    runtime!(),
    sep!(),
    "yog",
    sep!(),
    "Messages",
    sep!(),
    "onServer"
);
pub const LIST: &str = concat!(java_util!(), sep!(), "List");
pub const PARSE_INT: &str = "Integer.parseInt";
pub const PARSE_FLOAT: &str = "Float.parseFloat";
pub const PARSE_DOUBLE: &str = "Double.parseDouble";
/// An `event`'s handlers (the runtime's `Event`): added (by a key),
/// removed by it, the list of them as they are; a method's key made of its
/// object and itself (`List.of(c, "Times#0")`).
pub const EVENT_CLASS: &str = concat!(runtime!(), sep!(), "Event");
pub const EVENT_ADD: &str = "add";
pub const EVENT_REMOVE: &str = "remove";
pub const EVENT_HANDLERS: &str = "handlers";
/// A list's size and its item at an index (`java.util.List`'s).
pub const LIST_SIZE: &str = "size";
pub const LIST_GET: &str = "get";
/// The file class's helper raising events of one signature (`$raise1`).
pub const RAISER: &str = "$raise";
pub const KEY_OF: &str = concat!(java_util!(), sep!(), "List", sep!(), "of");
pub const OVERLOAD_SEP: &str = "#";

/// A request record's name: its method's, and this.
pub const REQUEST: &str = "Request";

/// What a struct's objects are copied with.
pub const COPY: &str = "copy";

/// What makes a library collection out of a Java one, keeping it
/// (`List.view`, `Dictionary.view`).
pub const VIEW: &str = "view";

/// `System.Array`'s companion's maker of a typed array out of what
/// generic code gave (`Object[]`): `Array.Of(items, int[].class)`.
pub const TYPED_ARRAY: &str = OF_MEMBER;

/// `java.lang.Object`'s members a type's own `Equals`, `GetHashCode` and
/// `ToString` stand behind, so Java's collections and strings use them.
pub const EQUALS: &str = "equals";
pub const HASH_CODE: &str = "hashCode";
pub const TO_STRING: &str = "toString";

/// What a method named after its type too joins them with (`ISet$Add`):
/// one Java couldn't tell from one it inherits.
pub const MEMBER_SEP: char = '$';

/// Java's types compiled code names.
pub const STRING: &str = "String";
pub const OBJECT: &str = "Object";
pub const BIG_DECIMAL: &str = concat!(java_math!(), sep!(), "BigDecimal");
pub const CLONEABLE: &str = "Cloneable";
/// What separates a package's segments in a class file's names
/// (`net/minecraft/world`), and a nested class from its outer one.
pub const INTERNAL_SEP: char = '/';
pub const NESTED: char = '$';

/// Java's string as a class file names it (a game class's descriptor).
pub const STRING_INTERNAL: &str = "java/lang/String";

/// Reflection as compiled code reaches it: a type's description is in its
/// class's `$TYPE` (`Type.Declare(…)`), a type as a value is `Type.Of(X.class)`.
pub mod reflection {
    use super::{OF, OF_MEMBER, system, type_name};

    /// `System.Type`'s class, in full: a source's own `Type` doesn't hide it.
    pub const TYPE: &str = concat!(runtime!(), sep!(), type_name!());
    pub const FIELD: &str = "$TYPE";
    pub const OF_TYPE: &str = OF_MEMBER;
    pub const DECLARE: &str = "Declare";
    /// The description's: the class it inherits, an interface, a type parameter.
    pub const BASE: &str = "Base";
    pub const INTERFACE: &str = "Interface";
    pub const PARAMETER: &str = "Parameter";
    /// What a declared type is, `Type`'s flags.
    pub const CLASS: &str = "CLASS";
    pub const STRUCT: &str = "STRUCT";
    pub const INTERFACE_TRAIT: &str = "INTERFACE";
    pub const ENUM: &str = "ENUM";
    pub const DELEGATE: &str = "DELEGATE";
    /// Abstract: a type's flag and a member's, of one name.
    pub const ABSTRACT: &str = "ABSTRACT";
    pub const SEALED: &str = "SEALED";
    pub const PRIMITIVE: &str = "PRIMITIVE";
    /// The runtime's hand-written types' descriptions: a companion each
    /// (`Int32Companion.TYPE`) in this package, found by their classes through
    /// `Library.of`; `System.Array`'s and an array's interfaces' there too.
    pub const LIBRARY_PACKAGE: &str = concat!(reflection!(), sep!(), "library");
    pub const COMPANION_SUFFIX: &str = "Companion";
    pub const COMPANION_FIELD: &str = "TYPE";
    pub const LIBRARY: &str = "Library";
    pub const LIBRARY_OF: &str = OF;
    pub const LIBRARY_ARRAY: &str = "array";
    pub const LIBRARY_ARRAY_INTERFACES: &str = "arrayInterfaces";
    /// A generic object keeping the type arguments it was made with:
    /// `Type.made(new Box<>(…), args)`, its class `Typed` by its `$types`.
    pub const TYPED: &str = concat!(runtime!(), sep!(), "Typed");
    pub const TYPES_FIELD: &str = "$types";
    pub const MADE: &str = "made";
    /// The source's class's constants of the arguments its objects are made
    /// with (`$kept1`): each list worked out once.
    pub const KEPT: &str = "$kept";
    /// What fills a type's description, beside `$TYPE` (`Box::$describe`).
    pub const DESCRIBE: &str = "$describe";
    pub const DESCRIPTION: &str = concat!(reflection!(), sep!(), "TypeDescription");
    /// The description's members, each a call: a field, a property, a method
    /// (its result and parameters after it), a constructor, a method's own
    /// type parameter.
    pub const FIELD_MEMBER: &str = "Field";
    pub const PROPERTY: &str = "Property";
    pub const METHOD: &str = "Method";
    pub const RETURNS: &str = "Returns";
    pub const TAKES: &str = "Takes";
    pub const CONSTRUCTOR: &str = "Constructor";
    pub const METHOD_PARAMETER: &str = "MethodParameter";
    /// An attribute's maker, named after `System.Attribute`: on the member
    /// described last; before any, on the type.
    pub const ATTRIBUTE: &str = system::ATTRIBUTE;
    /// What a member is, `MemberInfo`'s flags.
    pub const MEMBER_INFO: &str = concat!(reflection!(), sep!(), "MemberInfo");
    pub const STATIC: &str = "STATIC";
    pub const PRIVATE: &str = "PRIVATE";
    pub const PROTECTED: &str = "PROTECTED";
    pub const VIRTUAL: &str = "VIRTUAL";
    pub const INIT_ONLY: &str = "INIT_ONLY";
    pub const LITERAL: &str = "LITERAL";
    /// A parameter passed by reference: its type's (`Int32&`), how
    /// (`ParameterInfo`'s flags), its cells in a reflection call.
    pub const MAKE_BY_REF: &str = "MakeByRefType";
    pub const PARAMETER_INFO: &str = concat!(reflection!(), sep!(), "ParameterInfo");
    pub const PASSED_OUT: &str = "OUT";
    pub const CELLS: &str = "$cells";
    pub const CELL_OF: &str = OF;
    pub const CELLS_BACK: &str = "back";
    /// Generics kept: a type's parameter read off its object
    /// (`Type.argument(this, Box.class, 0)`), a method's handed to it
    /// (`$typeT`), a compiled generic method's description marked so.
    pub const ARGUMENT_OF: &str = "argument";
    pub const TYPE_ARG: &str = "$type";
    pub const TAKES_TYPES: &str = "TakesTypes";
}

/// Expression trees as compiled code makes them: a quoted lambda is
/// `Expression.quote(delegate, names, types, $ps -> tree, lambda)`, its
/// calls' methods found as the compiler chose them (`methodOf`), a static
/// member by its name (`staticMember`).
pub mod trees {
    pub const QUOTE: &str = "quote";
    pub const METHOD_OF: &str = "methodOf";
    pub const STATIC_MEMBER: &str = "staticMember";
    /// A `new`'s constructor, a type's operator, a lambda inside a tree.
    pub const CONSTRUCTOR_OF: &str = "constructorOf";
    pub const OPERATOR_OF: &str = "operatorOf";
    pub const NESTED: &str = "nested";
    /// The tree's lambdas' parameter (`$ps0`, `$ps1` inside it): a lambda's
    /// parameters' nodes.
    pub const PARAMETERS: &str = "$ps";
}

/// Enums as compiled code has them (`System.Enum`'s runtime class): each a
/// class of its numbers' `TABLE`, a value made of a number by `of`, read
/// back by `value()`, two compared by `Enum.compare`.
pub mod enums {
    use super::{OF as JAVA_OF, system};

    pub const TABLE_CLASS: &str = "Table";
    pub const TABLE: &str = "TABLE";
    pub const TABLE_OF: &str = "table";
    pub const OF: &str = JAVA_OF;
    pub const VALUE: &str = "value";
    pub const COMPARE: &str = "compare";
    /// The table's: its base type's width, its cases carrying data, `[Flags]`.
    pub const WIDTH: &str = "Width";
    pub const CARRYING: &str = "Carrying";
    /// Named after the `[Flags]` attribute it stands for.
    pub const FLAGS: &str = system::FLAGS;
}

/// `async` methods as compiled code has them: a local class of the
/// runtime's `Async<T>` (`$Async`), its state (`$state`), its code
/// (`$step`, a `$loop` of a `switch`), what it awaits (`$suspend`) and
/// finishes with (`$finish`); the method making one (`$m`), starting it
/// (`run`, an `async void`'s `$start`) and giving its task (`$task`); its
/// fields of the method's locals (`x$v1`) and parameters (`x$p`), and
/// of the tasks it awaits (temporaries).
pub mod tasks {
    macro_rules! tasks {
        () => {
            concat!(runtime!(), sep!(), "threading", sep!(), "tasks")
        };
    }
    pub const ASYNC: &str = concat!(tasks!(), sep!(), "Async");
    pub const MACHINE: &str = "$Async";
    pub const STATE: &str = "$state";
    pub const STEP: &str = "$step";
    pub const LOOP: &str = "$loop";
    pub const SUSPEND: &str = "$suspend";
    pub const FINISH: &str = "$finish";
    pub const MADE: &str = "$m";
    pub const RUN: &str = "run";
    pub const START: &str = "$start";
    pub const TASK: &str = "$task";
    pub const LOCAL_FIELD: &str = "$v";
    pub const PARAM_FIELD: &str = "$p";
    /// What a task gives (`Task<T>`'s), and a `Task`'s failure thrown again.
    pub const RESULT: &str = "Result";
    pub const CHECK: &str = "check";
}
