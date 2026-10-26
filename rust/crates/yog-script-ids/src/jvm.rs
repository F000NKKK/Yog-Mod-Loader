//! Where Yog Script's runtime is on the JVM (`java-api`'s
//! `dev.yog.script`), and the names of it compiled code calls.

/// The runtime's package: the System library's, its sub-namespaces under
/// it by their lowercased names.
pub const RUNTIME: &str = "dev.yog.script";

/// The runtime's package of built-in types' members' companions
/// (`[Host]`), apart so `String` there doesn't hide Java's.
pub const HOST_PACKAGE: &str = "host";

/// The runtime's helpers compiled code calls (`Ys.at`, `Ys.noArm`).
pub const HELPERS: &str = "Ys";

/// The helper giving a value with another evaluated after it: a type's own
/// `x++`'s old value (`Ys.before(x, x = …)`), a number's `GetType()`.
pub const BEFORE: &str = "before";

/// What every mod's class implements, the runtime finding it by.
pub const MOD_INTERFACE: &str = "dev.yog.script.YsMod";

/// What a source's class is named after its file: `Glassy` + this.
pub const CLASS_SUFFIX: &str = "Script";

/// The bus's call for Java code (`java-common`'s `dev.yog.api.YogBus`).
pub const BUS_CALL: &str = "dev.yog.api.YogBus.call";

/// A request record's name: its method's, and this.
pub const REQUEST: &str = "Request";

/// What a struct's objects are copied with.
pub const COPY: &str = "copy";

/// What makes a library collection out of a Java one, keeping it
/// (`List.view`, `Dictionary.view`).
pub const VIEW: &str = "view";

/// `System.Array`'s companion's maker of a typed array out of what
/// generic code gave (`Object[]`): `Array.Of(items, int[].class)`.
pub const TYPED_ARRAY: &str = "Of";

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
pub const BIG_DECIMAL: &str = "java.math.BigDecimal";
pub const CLONEABLE: &str = "Cloneable";
/// What separates a package's segments in a class file's names
/// (`net/minecraft/world`), and a nested class from its outer one.
pub const INTERNAL_SEP: char = '/';
pub const NESTED: char = '$';

/// Java's string as a class file names it (a game class's descriptor).
pub const STRING_INTERNAL: &str = "java/lang/String";

/// Java's primitive types and the classes they box to, by the primitive.
pub const VOID: &str = "void";
pub const BOXED_VOID: &str = "Void";
pub const BYTE: &str = "byte";
pub const SHORT: &str = "short";
pub const INT: &str = "int";
pub const LONG: &str = "long";
pub const FLOAT: &str = "float";
pub const DOUBLE: &str = "double";
pub const BOOLEAN: &str = "boolean";
pub const CHAR: &str = "char";
pub const BOXED: [(&str, &str); 8] = [
    (BYTE, "Byte"),
    (SHORT, "Short"),
    (INT, "Integer"),
    (LONG, "Long"),
    (FLOAT, "Float"),
    (DOUBLE, "Double"),
    (BOOLEAN, "Boolean"),
    (CHAR, "Character"),
];

/// Reflection as compiled code reaches it: a type's description is in its
/// class's `$TYPE` (`Type.Declare(…)`), a type as a value is `Type.Of(X.class)`.
pub mod reflection {
    /// `System.Type`'s class, in full: a source's own `Type` doesn't hide it.
    pub const TYPE: &str = "dev.yog.script.Type";
    pub const FIELD: &str = "$TYPE";
    pub const OF: &str = "Of";
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
    pub const ABSTRACT: &str = "ABSTRACT";
    pub const SEALED: &str = "SEALED";
    pub const PRIMITIVE: &str = "PRIMITIVE";
    /// The runtime's hand-written types' descriptions: a companion each
    /// (`Int32Companion.TYPE`) in this package, found by their classes through
    /// `Library.of`; `System.Array`'s and an array's interfaces' there too.
    pub const PACKAGE: &str = "dev.yog.script.reflection.library";
    pub const COMPANION_SUFFIX: &str = "Companion";
    pub const COMPANION_FIELD: &str = "TYPE";
    pub const LIBRARY: &str = "Library";
    pub const LIBRARY_OF: &str = "of";
    pub const LIBRARY_ARRAY: &str = "array";
    pub const LIBRARY_ARRAY_INTERFACES: &str = "arrayInterfaces";
}
