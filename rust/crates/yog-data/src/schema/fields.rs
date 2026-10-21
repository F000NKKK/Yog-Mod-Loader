//! A struct's fields as files write them, by their Rust names: code that
//! names a field (where a problem is) says it through [`field!`](crate::field),
//! checked when it's compiled and renamed with the field.

/// A struct with named fields (`#[derive(Schema)]`): each field's Rust name
/// and its name in files.
pub trait Fields {
    const FIELDS: &'static [(&'static str, &'static str)];
}

/// `ident`'s name in files among `fields` — a compile error when it's used
/// in a constant (as [`field!`](crate::field) does) and there's no such field.
#[must_use]
pub const fn field_name(
    fields: &'static [(&'static str, &'static str)],
    ident: &str,
) -> &'static str {
    let mut i = 0;
    while i < fields.len() {
        if same(fields[i].0, ident) {
            return fields[i].1;
        }
        i += 1;
    }
    panic!("no such field")
}

const fn same(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

/// Field `ident` of struct `T` as files write it: `field!(BlockProps, tool_level)`
/// → `"tool_level"`. A field `T` hasn't got doesn't compile.
#[macro_export]
macro_rules! field {
    ($t:ty, $f:ident) => {{
        const NAME: &str = $crate::schema::field_name(
            <$t as $crate::schema::Fields>::FIELDS,
            ::core::stringify!($f),
        );
        NAME
    }};
}

#[cfg(test)]
mod tests {
    use crate::Schema;

    #[derive(Schema)]
    struct Probe {
        plain: u32,
        #[data(rename = "written")]
        renamed: u32,
    }

    #[test]
    fn fields_by_their_rust_names() {
        assert_eq!(crate::field!(Probe, plain), "plain");
        assert_eq!(crate::field!(Probe, renamed), "written");
    }
}
