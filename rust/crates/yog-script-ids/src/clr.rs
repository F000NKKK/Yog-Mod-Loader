//! The CLR's names for what C# writes with signs and words: operators'
//! methods, an indexer's, a delegate's call, the built-in types' own.

/// An indexer's member name (`x[i]`): C#'s `IndexerName` default; its
/// accessors `get_Item` / `set_Item`, the accessor and the name joined.
pub const INDEXER: &str = "Item";
pub const ACCESSOR_SEP: char = '_';

/// What calling a delegate calls (`handler(x)` is `handler.Invoke(x)`).
pub const INVOKE: &str = "Invoke";

/// The operators' methods (ECMA-335 §I.10.3).
pub const OP_ADDITION: &str = "op_Addition";
pub const OP_SUBTRACTION: &str = "op_Subtraction";
pub const OP_MULTIPLY: &str = "op_Multiply";
pub const OP_DIVISION: &str = "op_Division";
pub const OP_MODULUS: &str = "op_Modulus";
pub const OP_BITWISE_AND: &str = "op_BitwiseAnd";
pub const OP_BITWISE_OR: &str = "op_BitwiseOr";
pub const OP_EXCLUSIVE_OR: &str = "op_ExclusiveOr";
pub const OP_LEFT_SHIFT: &str = "op_LeftShift";
pub const OP_RIGHT_SHIFT: &str = "op_RightShift";
pub const OP_EQUALITY: &str = "op_Equality";
pub const OP_INEQUALITY: &str = "op_Inequality";
pub const OP_LESS_THAN: &str = "op_LessThan";
pub const OP_LESS_THAN_OR_EQUAL: &str = "op_LessThanOrEqual";
pub const OP_GREATER_THAN: &str = "op_GreaterThan";
pub const OP_GREATER_THAN_OR_EQUAL: &str = "op_GreaterThanOrEqual";
pub const OP_UNARY_NEGATION: &str = "op_UnaryNegation";
pub const OP_LOGICAL_NOT: &str = "op_LogicalNot";
pub const OP_ONES_COMPLEMENT: &str = "op_OnesComplement";
pub const OP_INCREMENT: &str = "op_Increment";
pub const OP_DECREMENT: &str = "op_Decrement";
pub const OP_IMPLICIT: &str = "op_Implicit";
pub const OP_EXPLICIT: &str = "op_Explicit";

/// The built-in types' types in `System` (`i32` is `System.Int32`).
pub const SBYTE: &str = "SByte";
pub const INT16: &str = "Int16";
pub const INT32: &str = "Int32";
pub const INT64: &str = "Int64";
pub const BYTE: &str = "Byte";
pub const UINT16: &str = "UInt16";
pub const UINT32: &str = "UInt32";
pub const UINT64: &str = "UInt64";
pub const SINGLE: &str = "Single";
pub const DOUBLE: &str = "Double";
pub const DECIMAL: &str = "Decimal";
pub const CHAR: &str = "Char";
pub const BOOLEAN: &str = "Boolean";
pub const STRING: &str = "String";
pub const OBJECT: &str = "Object";
pub const VOID: &str = "Void";
