//! The Java language's own words the compiler writes — its keywords, its
//! literals, its primitive types, its marks (`.class`, `@Override`) — one
//! place each (`jvm` is where the runtime is; this, what Java is).

// Its literals.
pub const NULL: &str = "null";
pub const TRUE: &str = "true";
pub const FALSE: &str = "false";
/// A number's zero, a `char`'s.
pub const ZERO: &str = "0";
pub const ZERO_CHAR: &str = "'\\0'";

// Its keywords.
pub const ABSTRACT: &str = "abstract";
pub const BREAK: &str = "break";
pub const CLASS: &str = "class";
pub const CONTINUE: &str = "continue";
pub const DEFAULT: &str = "default";
pub const DO: &str = "do";
pub const ELSE: &str = "else";
pub const ENUM: &str = "enum";
pub const EXTENDS: &str = "extends";
pub const FINAL: &str = "final";
pub const FOR: &str = "for";
pub const IF: &str = "if";
pub const IMPLEMENTS: &str = "implements";
pub const INSTANCEOF: &str = "instanceof";
pub const INTERFACE: &str = "interface";
pub const NEW: &str = "new";
pub const PACKAGE: &str = "package";
pub const PERMITS: &str = "permits";
pub const PRIVATE: &str = "private";
pub const PROTECTED: &str = "protected";
pub const PUBLIC: &str = "public";
pub const RECORD: &str = "record";
pub const RETURN: &str = "return";
pub const SEALED: &str = "sealed";
pub const STATIC: &str = "static";
pub const SUPER: &str = "super";
pub const THIS: &str = "this";
pub const THROW: &str = "throw";
pub const TRY: &str = "try";
pub const CATCH: &str = "catch";
pub const VAR: &str = "var";
pub const WHILE: &str = "while";

// Its marks: a class as a value, an array's rank, a type argument not
// known, a `long` literal's suffix, the shift filling with zeros.
pub const CLASS_LITERAL: &str = ".class";
pub const RANK: &str = "[]";
pub const WILDCARD: &str = "?";
pub const LONG_SUFFIX: &str = "L";
pub const UNSIGNED_SHR: &str = ">>>";
/// Bits together (`a | b`): `Type`'s and `MemberInfo`'s flags joined.
pub const BIT_OR: &str = "|";
/// A Java source file's extension.
pub const SOURCE_EXT: &str = ".java";

// Its annotations.
pub const OVERRIDE: &str = "@Override";
pub const FUNCTIONAL: &str = "@FunctionalInterface";
/// Java's word that casts its generics can't check are known safe.
pub const UNCHECKED: &str = "@SuppressWarnings(\"unchecked\")";
pub const UNCHECKED_RAW: &str = "@SuppressWarnings({\"unchecked\", \"rawtypes\"})";

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
