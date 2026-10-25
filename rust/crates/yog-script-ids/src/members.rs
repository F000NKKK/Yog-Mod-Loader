//! The System library's members a tool runs by hand (the build's
//! interpreter, the compilers' bridges), by their names as declared
//! (`system/**/*.ys`).

// `object`'s: what every value has.
pub const EQUALS: &str = "Equals";
pub const GET_HASH_CODE: &str = "GetHashCode";
pub const TO_STRING: &str = "ToString";

// Collections (`List<T>`).
pub const COUNT: &str = "Count";
pub const ADD: &str = "Add";
pub const CONTAINS: &str = "Contains";
pub const INDEX_OF: &str = "IndexOf";
pub const CLEAR: &str = "Clear";
pub const REMOVE_AT: &str = "RemoveAt";

// `Math`.
pub const MAX: &str = "Max";
pub const MIN: &str = "Min";
pub const ABS: &str = "Abs";
pub const CLAMP: &str = "Clamp";
pub const SQRT: &str = "Sqrt";
pub const FLOOR: &str = "Floor";
pub const ROUND: &str = "Round";

// `string`.
pub const LENGTH: &str = "Length";
pub const TO_UPPER: &str = "ToUpper";
pub const TO_LOWER: &str = "ToLower";
pub const TRIM: &str = "Trim";
pub const STARTS_WITH: &str = "StartsWith";
pub const ENDS_WITH: &str = "EndsWith";
pub const SUBSTRING: &str = "Substring";

// `Result<T, E>`.
pub const IS_OK: &str = "IsOk";
pub const IS_ERR: &str = "IsErr";
pub const UNWRAP: &str = "Unwrap";
pub const EXPECT: &str = "Expect";
pub const UNWRAP_OR: &str = "UnwrapOr";
