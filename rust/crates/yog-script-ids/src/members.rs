//! The System library's members a tool runs by hand (the build's
//! interpreter, the compilers' bridges), by their names as declared
//! (`system/**/*.ys`).

// `object`'s: what every value has.
pub const EQUALS: &str = "Equals";
pub const GET_HASH_CODE: &str = "GetHashCode";
pub const TO_STRING: &str = "ToString";
pub const GET_TYPE: &str = "GetType";

// An async sequence's walk (`await foreach`): made, stepped, read, let go.
pub const GET_ASYNC_ENUMERATOR: &str = "GetAsyncEnumerator";
pub const MOVE_NEXT_ASYNC: &str = "MoveNextAsync";
pub const CURRENT: &str = "Current";
pub const DISPOSE_ASYNC: &str = "DisposeAsync";

// A `Lock<T>`'s: what `lock` enters it by, its value.
pub const ENTER: &str = "Enter";
pub const VALUE: &str = "Value";

// `IDisposable`'s: what a drop calls.
pub const DISPOSE: &str = "Dispose";

// Collections (`List<T>`).
pub const COUNT: &str = "Count";
pub const ADD: &str = "Add";
/// What a collection initializer's items are written on: the object being
/// made (`new List<i32> { 1, 2 }` is `$made.Add(1); $made.Add(2);`). No
/// source can name it.
pub const MADE: &str = "$made";
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

// `System.Linq.Enumerable`'s, a query's clauses become (`from … select`).
pub const WHERE: &str = "Where";
pub const SELECT: &str = "Select";
pub const SELECT_MANY: &str = "SelectMany";
pub const ORDER_BY: &str = "OrderBy";
pub const ORDER_BY_DESCENDING: &str = "OrderByDescending";
pub const THEN_BY: &str = "ThenBy";
pub const THEN_BY_DESCENDING: &str = "ThenByDescending";
pub const GROUP_BY: &str = "GroupBy";
pub const JOIN: &str = "Join";
pub const GROUP_JOIN: &str = "GroupJoin";
pub const CAST: &str = "Cast";

// `System.Type`'s: a generic type made of its arguments, an array of a type.
pub const MAKE_GENERIC_TYPE: &str = "MakeGenericType";
pub const MAKE_GENERIC_METHOD: &str = "MakeGenericMethod";
pub const MAKE_ARRAY_TYPE: &str = "MakeArrayType";

// `System.Linq.Expressions.Expression`'s factories a quoted lambda's tree is made by.
pub const CONSTANT: &str = "Constant";
pub const PROPERTY_OR_FIELD: &str = "PropertyOrField";
pub const CALL: &str = "Call";
pub const CONDITION: &str = "Condition";
pub const CONVERT: &str = "Convert";
pub const ARRAY_INDEX: &str = "ArrayIndex";
pub const NOT: &str = "Not";
pub const NEGATE: &str = "Negate";
pub const COALESCE: &str = "Coalesce";
pub const SUBTRACT: &str = "Subtract";
pub const MULTIPLY: &str = "Multiply";
pub const DIVIDE: &str = "Divide";
pub const MODULO: &str = "Modulo";
pub const AND: &str = "And";
pub const OR: &str = "Or";
pub const EXCLUSIVE_OR: &str = "ExclusiveOr";
pub const LEFT_SHIFT: &str = "LeftShift";
pub const RIGHT_SHIFT: &str = "RightShift";
pub const AND_ALSO: &str = "AndAlso";
pub const OR_ELSE: &str = "OrElse";
pub const EQUAL: &str = "Equal";
pub const NOT_EQUAL: &str = "NotEqual";
pub const LESS_THAN: &str = "LessThan";
pub const LESS_THAN_OR_EQUAL: &str = "LessThanOrEqual";
pub const GREATER_THAN: &str = "GreaterThan";
pub const GREATER_THAN_OR_EQUAL: &str = "GreaterThanOrEqual";
pub const NEW: &str = "New";
pub const MAKE_BINARY: &str = "MakeBinary";
pub const MAKE_UNARY: &str = "MakeUnary";

/// `Task`'s members (`System.Threading.Tasks`).
pub const FROM_RESULT: &str = "FromResult";
pub const YIELD_MEMBER: &str = "Yield";
pub const DELAY: &str = "Delay";
pub const WHEN_ALL: &str = "WhenAll";
pub const WHEN_ANY: &str = "WhenAny";
pub const COMPLETED_TASK: &str = "CompletedTask";
pub const RESULT_MEMBER: &str = "Result";
pub const IS_COMPLETED: &str = "IsCompleted";
pub const WAIT: &str = "Wait";
pub const GET_RESULT: &str = "GetResult";
pub const GET_AWAITER: &str = "GetAwaiter";
pub const CONFIGURE_AWAIT: &str = "ConfigureAwait";
