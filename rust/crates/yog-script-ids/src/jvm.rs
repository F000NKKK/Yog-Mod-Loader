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

/// The helper giving a type's own `x++` its old value (`Ys.before(x, x = …)`).
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
