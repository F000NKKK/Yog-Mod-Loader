//! Type-aware formatting of a located variable into a bounded [`Value`]
//! tree, with Rust-specific pretty forms for `&str`, slices, `String`,
//! `Vec<T>`, `Box<T>`, enums with variant parts, and the std wrappers a
//! user doesn't want to click through (`Cell`, `RefCell`, `OnceLock`,
//! `Rc`/`Arc`, `NonNull`, `PhantomData`...).

use std::cell::Cell;

use gimli::{AttributeValue, DebuggingInformationEntry, UnitOffset};

mod nav;

pub(crate) use nav::Slot;

use crate::locals::{Place, TargetMemory, UNAVAILABLE, UnitInfo, Value, unit_ref};
use crate::{R, SymbolTable};

mod std_types;

mod dwarf;

/// Nesting below a top-level variable that still gets expanded children.
const MAX_DEPTH: usize = 3;
/// Elements shown for arrays, slices and `Vec`s.
const MAX_ELEMS: u64 = 32;
/// Bytes of string data read for `&str` / `String`.
const MAX_STR: u64 = 256;
/// Total nodes produced for one variable, so a wide struct of wide arrays
/// stays cheap.
const NODE_BUDGET: usize = 2048;
/// Length of the one-line summary of an aggregate's children.
const SUMMARY_LEN: usize = 120;
/// Shown for a null pointer.
pub(crate) const NULL: &str = "null";
/// Shown for an empty `OnceCell` / `OnceLock`.
const UNINIT: &str = "<uninit>";
/// `Option` variant names.
const SOME: &str = "Some";
const NONE: &str = "None";
/// Qualified-name prefix of `PhantomData` / `PhantomPinned`.
const PHANTOM_PREFIX: &str = "core::marker::Phantom";
const PTR_SIZE: u64 = crate::locals::WORD_SIZE as u64;
/// Typedef/qualifier layers [`Formatter::peel`] strips.
const MAX_PEEL: usize = 16;
/// Children of one DIE looked at.
const MAX_CHILDREN: usize = 4096;
/// Enclosing namespaces/types in a qualified name.
const MAX_QUALIFIERS: usize = 32;
/// Nesting of a type name, and of the leading-member walks that find an
/// integer or pointer inside a wrapper.
const MAX_TYPE_DEPTH: usize = 8;
/// Shown where a value or name is cut off.
pub(crate) const ELLIPSIS: &str = "…";

/// Member names rustc gives the parts of a fat pointer (`&str`, `&[T]`).
const FAT_DATA: &str = "data_ptr";
const FAT_LEN: &str = "length";
/// Prefix rustc gives tuple fields (`__0`, `__1`...).
const TUPLE_FIELD: &str = "__";

const STRING: &str = "alloc::string::String";
const VEC: &str = "alloc::vec::Vec";
const BOX: &str = "alloc::boxed::Box";
const PIN: &str = "core::pin::Pin";
/// std wrappers shown as the one value they hold.
const TRANSPARENT_WRAPPERS: [&str; 8] = [
    "core::cell::UnsafeCell",
    "core::cell::SyncUnsafeCell",
    "core::cell::Cell",
    "core::cell::RefCell",
    "core::mem::maybe_uninit::MaybeUninit",
    "core::mem::manually_drop::ManuallyDrop",
    "core::mem::maybe_dangling::MaybeDangling",
    PIN,
];
const NON_NULL: &str = "core::ptr::non_null::NonNull";
const UNIQUE: &str = "core::ptr::unique::Unique";
const PHANTOM_DATA: &str = "core::marker::PhantomData";
const PHANTOM_PINNED: &str = "core::marker::PhantomPinned";
const ONCE_CELL: &str = "core::cell::once::OnceCell";
const ONCE_LOCK: &str = "std::sync::once_lock::OnceLock";
const RC: &str = "alloc::rc::Rc";
const ARC: &str = "alloc::sync::Arc";

type Off = UnitOffset<usize>;
type Die = DebuggingInformationEntry<R>;

pub(crate) struct Formatter<'a> {
    table: &'a SymbolTable,
    info: &'a UnitInfo,
    mem: &'a dyn TargetMemory,
    budget: Cell<usize>,
}

/// A run of elements: their type, where the first is, the step to the next, how many.
struct Items {
    elem: Off,
    base: Place,
    stride: u64,
    count: u64,
}

struct Member {
    name: String,
    ty: Option<Off>,
    offset: u64,
}

struct Variant {
    discr_value: Option<u64>,
    member: Member,
}

enum VariantPick {
    Found(Variant),
    /// A discriminant wider than 64 bits.
    Unsupported,
    /// No variant matches this raw discriminant.
    Unknown(u64),
}

impl<'a> Formatter<'a> {
    pub(crate) fn new(
        table: &'a SymbolTable,
        info: &'a UnitInfo,
        mem: &'a dyn TargetMemory,
    ) -> Self {
        Self::with_budget(table, info, mem, NODE_BUDGET)
    }

    /// A formatter with its own, smaller node budget — for one row of a
    /// lazily expanded view, where only the row's one-line summary is kept.
    pub(crate) fn with_budget(
        table: &'a SymbolTable,
        info: &'a UnitInfo,
        mem: &'a dyn TargetMemory,
        budget: usize,
    ) -> Self {
        Formatter {
            table,
            info,
            mem,
            budget: Cell::new(budget),
        }
    }

    // ---- DIE helpers -------------------------------------------------

    pub(crate) fn format(
        &self,
        name: String,
        ty: Option<Off>,
        place: &Place,
        depth: usize,
    ) -> Value {
        let type_name = short_type_name(&self.type_name(ty));
        let mut out = Value {
            name,
            type_name,
            value: String::new(),
            children: Vec::new(),
        };
        let left = self.budget.get();
        if left == 0 {
            out.value = ELLIPSIS.into();
            return out;
        }
        self.budget.set(left - 1);

        let Some(ty) = ty else {
            out.value = "()".into();
            return out;
        };
        let Some((off, entry)) = self.peel(ty) else {
            out.value = UNAVAILABLE.into();
            return out;
        };
        let result = match entry.tag() {
            gimli::DW_TAG_base_type => self.format_base(&entry, place).map(|v| (v, Vec::new())),
            gimli::DW_TAG_pointer_type
            | gimli::DW_TAG_reference_type
            | gimli::DW_TAG_rvalue_reference_type => {
                let ptr = self.read_uint(place, PTR_SIZE);
                ptr.map(|p| self.format_pointer(&out.name, p, self.ty_of(&entry), depth))
            }
            gimli::DW_TAG_structure_type | gimli::DW_TAG_class_type | gimli::DW_TAG_union_type => {
                self.format_struct(off, &entry, place, depth)
            }
            gimli::DW_TAG_enumeration_type => self.format_c_enum(off, &entry, place),
            gimli::DW_TAG_array_type => self.format_array(&entry, place, depth),
            gimli::DW_TAG_subroutine_type => Some(("fn".into(), Vec::new())),
            _ => Some(("<unsupported type>".into(), Vec::new())),
        };
        match result {
            Some((value, children)) => {
                out.value = value;
                out.children = children;
            }
            None => out.value = UNAVAILABLE.into(),
        }
        out
    }

    fn format_base(&self, entry: &Die, place: &Place) -> Option<String> {
        let size = Self::udata(entry, gimli::DW_AT_byte_size).unwrap_or(0);
        if size == 0 {
            return Some("()".into());
        }
        let bytes = self.read(place, size)?;
        let encoding = match entry.attr_value(gimli::DW_AT_encoding) {
            Some(AttributeValue::Encoding(e)) => e,
            _ => gimli::DW_ATE_unsigned,
        };
        let mut wide = [0u8; 16];
        let n = bytes.len().min(16);
        wide[..n].copy_from_slice(&bytes[..n]);
        let unsigned = u128::from_le_bytes(wide);
        let signed = || {
            let shift = 128 - 8 * n as u32;
            ((unsigned << shift) as i128) >> shift
        };
        Some(match encoding {
            gimli::DW_ATE_boolean => match unsigned {
                0 => "false".into(),
                1 => "true".into(),
                other => format!("{other} (invalid bool)"),
            },
            gimli::DW_ATE_UTF if size == 4 || size == 1 => match char::from_u32(unsigned as u32) {
                Some(c) => format!("{c:?}"),
                None => format!("{unsigned:#x} (invalid char)"),
            },
            gimli::DW_ATE_float if size == 4 => format!("{:?}", f32::from_bits(unsigned as u32)),
            gimli::DW_ATE_float if size == 8 => format!("{:?}", f64::from_bits(unsigned as u64)),
            gimli::DW_ATE_signed | gimli::DW_ATE_signed_char if n <= 16 => signed().to_string(),
            gimli::DW_ATE_unsigned | gimli::DW_ATE_unsigned_char if n <= 16 => unsigned.to_string(),
            _ => hex_bytes(&bytes),
        })
    }

    fn format_pointer(
        &self,
        name: &str,
        ptr: u64,
        pointee: Option<Off>,
        depth: usize,
    ) -> (String, Vec<Value>) {
        if ptr == 0 {
            return (NULL.into(), Vec::new());
        }
        let hex = format!("{ptr:#x}");
        let Some(pointee) = pointee else {
            return (hex, Vec::new());
        };
        if depth >= MAX_DEPTH {
            return (hex, Vec::new());
        }
        if !self.is_derefable(pointee) {
            return (hex, Vec::new());
        }
        let child = self.format(
            format!("*{name}"),
            Some(pointee),
            &Place::Mem(ptr),
            depth + 1,
        );
        if child.children.is_empty() && child.value != UNAVAILABLE {
            (format!("{hex} {{{}}}", child.value), vec![child])
        } else {
            (hex, vec![child])
        }
    }

    fn format_c_enum(&self, off: Off, entry: &Die, place: &Place) -> Option<(String, Vec<Value>)> {
        let size = Self::udata(entry, gimli::DW_AT_byte_size).unwrap_or(4);
        let raw = self.read_uint(place, size)?;
        let mask = mask(size);
        for e in self.children(off) {
            if e.tag() != gimli::DW_TAG_enumerator {
                continue;
            }
            let value = match e.attr_value(gimli::DW_AT_const_value) {
                Some(v) => attr_u64(&v),
                None => None,
            };
            if value.map(|v| v & mask) == Some(raw & mask) {
                return Some((self.name_of(&e).unwrap_or_default(), Vec::new()));
            }
        }
        Some((format!("{raw} (unknown variant)"), Vec::new()))
    }

    fn format_array(
        &self,
        entry: &Die,
        place: &Place,
        depth: usize,
    ) -> Option<(String, Vec<Value>)> {
        let (elem, count) = self.array_shape(entry)?;
        let elem = elem?;
        let stride = self.array_stride(entry, elem)?;
        Some(self.format_elements(
            &Items {
                elem,
                base: place.clone(),
                stride,
                count,
            },
            depth,
        ))
    }

    /// An array's elements: children `[0]..[n]` and a `[a, b, c]` summary.
    fn format_elements(&self, items: &Items, depth: usize) -> (String, Vec<Value>) {
        self.format_items(items, depth, String::new())
    }

    /// A dynamically sized sequence's: the same, its length first.
    fn format_sequence(&self, items: &Items, depth: usize) -> (String, Vec<Value>) {
        self.format_items(items, depth, format!("len={} ", items.count))
    }

    fn format_items(&self, items: &Items, depth: usize, prefix: String) -> (String, Vec<Value>) {
        if depth >= MAX_DEPTH {
            return (format!("{prefix}[{ELLIPSIS}]"), Vec::new());
        }
        let shown = items.count.min(MAX_ELEMS);
        let children: Vec<Value> = (0..shown)
            .map(|i| {
                self.format(
                    format!("[{i}]"),
                    Some(items.elem),
                    &items.base.offset(i.wrapping_mul(items.stride)),
                    depth + 1,
                )
            })
            .collect();
        let summary = summarize(
            "[",
            "]",
            children.iter().map(|c| c.value.clone()),
            items.count > shown,
        );
        (format!("{prefix}{summary}"), children)
    }

    fn format_struct(
        &self,
        off: Off,
        entry: &Die,
        place: &Place,
        depth: usize,
    ) -> Option<(String, Vec<Value>)> {
        let children = self.children(off);
        if let Some(vp) = children
            .iter()
            .find(|e| e.tag() == gimli::DW_TAG_variant_part)
        {
            return self.format_rust_enum(vp, place, depth);
        }

        let short = self.name_of(entry).unwrap_or_default();
        let members = self.members(off);
        let member = |n: &str| Self::member_named(&members, n);

        // Fat pointers: `&str`, `&[T]`, `*const [T]`...
        if let (Some(data), Some(len)) = (member(FAT_DATA), member(FAT_LEN)) {
            let ptr = self.read_uint(&place.offset(data.offset), PTR_SIZE)?;
            let n = self.read_uint(&place.offset(len.offset), PTR_SIZE)?;
            if short.ends_with("str") {
                return Some((self.read_string(ptr, n), Vec::new()));
            }
            if short.contains('[') {
                let elem = data
                    .ty
                    .and_then(|t| self.peel(t))
                    .and_then(|(_, e)| self.ty_of(&e));
                if let Some(elem) = elem {
                    let stride = self.byte_size(elem)?;
                    return Some(self.format_sequence(
                        &Items {
                            elem,
                            base: Place::Mem(ptr),
                            stride,
                            count: n,
                        },
                        depth,
                    ));
                }
            }
        }

        let qualified = self.qualified_name(off, entry);
        if qualified == STRING
            && let Some((ptr, len)) =
                member("vec").and_then(|vec| self.vec_parts(vec.ty?, &place.offset(vec.offset)))
        {
            return Some((self.read_string(ptr, len), Vec::new()));
        }
        if generic_base(&qualified) == VEC
            && let (Some(elem), Some((ptr, len))) =
                (self.template_param(off, "T"), self.vec_parts(off, place))
        {
            let stride = self.byte_size(elem)?;
            return Some(self.format_sequence(
                &Items {
                    elem,
                    base: Place::Mem(ptr),
                    stride,
                    count: len,
                },
                depth,
            ));
        }
        if generic_base(&qualified) == BOX
            && let Some((ptr, pointee)) = self.find_pointer(off, place, 0)
        {
            return Some(self.format_pointer("", ptr, pointee, depth));
        }
        if let Some(wrapped) = self.format_wrapper(off, &qualified, &members, place, depth) {
            return Some(wrapped);
        }

        Some(self.format_fields(&members, place, depth, &short))
    }

    fn is_zero_sized(&self, ty: Option<Off>) -> bool {
        ty.and_then(|t| self.byte_size(t)) == Some(0)
    }

    fn is_phantom(&self, ty: Option<Off>) -> bool {
        ty.and_then(|t| self.peel(t)).is_some_and(|(off, e)| {
            e.tag() == gimli::DW_TAG_structure_type
                && self.qualified_name(off, &e).starts_with(PHANTOM_PREFIX)
        })
    }

    /// Generic struct/tuple/union: one child per member.
    fn format_fields(
        &self,
        members: &[Member],
        place: &Place,
        depth: usize,
        short_name: &str,
    ) -> (String, Vec<Value>) {
        let members: Vec<&Member> = members.iter().filter(|m| !self.is_phantom(m.ty)).collect();
        if members.is_empty() {
            return (short_name.to_string(), Vec::new());
        }
        let tuple = is_tuple(members.iter().copied());
        if depth >= MAX_DEPTH {
            let elided = if tuple {
                format!("({ELLIPSIS})")
            } else {
                format!("{{{ELLIPSIS}}}")
            };
            return (elided, Vec::new());
        }
        let children: Vec<Value> = members
            .iter()
            .map(|m| {
                self.format(
                    field_label(m, tuple),
                    m.ty,
                    &place.offset(m.offset),
                    depth + 1,
                )
            })
            .collect();
        let summary = if tuple {
            summarize("(", ")", children.iter().map(|c| c.value.clone()), false)
        } else {
            summarize(
                "{",
                "}",
                children.iter().map(|c| format!("{}: {}", c.name, c.value)),
                false,
            )
        };
        (summary, children)
    }

    /// Rust enums: a `DW_TAG_variant_part` whose `DW_AT_discr` member holds
    /// the (possibly niche-encoded) discriminant, and one `DW_TAG_variant`
    /// per variant (the dataful variant of a niche layout has no
    /// `DW_AT_discr_value`).
    fn format_rust_enum(
        &self,
        vp: &Die,
        place: &Place,
        depth: usize,
    ) -> Option<(String, Vec<Value>)> {
        let variant = match self.active_variant(vp, place)? {
            VariantPick::Found(v) => v,
            VariantPick::Unsupported => {
                return Some(("<enum: unsupported discriminant>".into(), Vec::new()));
            }
            VariantPick::Unknown(raw) => {
                return Some((format!("<unknown variant {raw}>"), Vec::new()));
            }
        };

        let name = variant.member.name.clone();
        let Some(vty) = variant.member.ty else {
            return Some((name, Vec::new()));
        };
        let fields = self.members(vty);
        if fields.is_empty() {
            return Some((name, Vec::new()));
        }
        let base = place.offset(variant.member.offset);
        let (summary, children) = self.format_fields(&fields, &base, depth, &name);
        let value = if is_tuple(&fields) {
            format!("{name}{summary}")
        } else {
            format!("{name} {summary}")
        };
        Some((value, children))
    }

    /// The variant of the Rust enum whose `DW_TAG_variant_part` is `vp`
    /// that the value at `place` holds; `None` if the discriminant can't
    /// be read.
    fn active_variant(&self, vp: &Die, place: &Place) -> Option<VariantPick> {
        let parts = self.children(vp.offset());
        let discr = match vp
            .attr_value(gimli::DW_AT_discr)
            .and_then(|v| unit_ref(self.info, v))
        {
            Some(d) => {
                let m = self.member(&self.entry(d)?);
                let size = m.ty.and_then(|t| self.byte_size(t)).unwrap_or(0);
                if size > 8 {
                    // 128-bit discriminants: not decoded.
                    return Some(VariantPick::Unsupported);
                }
                Some((self.read_uint(&place.offset(m.offset), size)?, size))
            }
            None => None,
        };

        let mut variants: Vec<Variant> = parts
            .iter()
            .filter(|e| e.tag() == gimli::DW_TAG_variant)
            .filter_map(|v| {
                let member = self
                    .children(v.offset())
                    .into_iter()
                    .find(|e| e.tag() == gimli::DW_TAG_member)?;
                Some(Variant {
                    discr_value: v
                        .attr_value(gimli::DW_AT_discr_value)
                        .and_then(|a| attr_u64(&a)),
                    member: self.member(&member),
                })
            })
            .collect();

        let chosen = match discr {
            Some((value, size)) => {
                let m = mask(size);
                variants
                    .iter()
                    .position(|v| v.discr_value.is_some_and(|d| d & m == value & m))
                    .or_else(|| variants.iter().position(|v| v.discr_value.is_none()))
            }
            None => (!variants.is_empty()).then_some(0),
        };
        Some(match chosen {
            Some(i) => VariantPick::Found(variants.swap_remove(i)),
            None => VariantPick::Unknown(discr.map(|d| d.0).unwrap_or_default()),
        })
    }

    /// `(data pointer, len)` of an `alloc::vec::Vec<T>` at `place`.
    fn vec_parts(&self, vec_ty: Off, place: &Place) -> Option<(u64, u64)> {
        let (off, _) = self.peel(vec_ty)?;
        let members = self.members(off);
        let buf = Self::member_named(&members, "buf")?;
        let len = Self::member_named(&members, "len")?;
        let (ptr, _) = self.find_pointer(buf.ty?, &place.offset(buf.offset), 0)?;
        let len = self.read_uint(&place.offset(len.offset), PTR_SIZE)?;
        Some((ptr, len))
    }

    /// First raw pointer reachable through leading struct members
    /// (`RawVec { inner: RawVecInner { ptr: Unique { pointer: NonNull {
    /// pointer: *const T }}}}` and friends), with its pointee type.
    fn find_pointer(&self, ty: Off, place: &Place, depth: usize) -> Option<(u64, Option<Off>)> {
        if depth > MAX_TYPE_DEPTH {
            return None;
        }
        let (off, entry) = self.peel(ty)?;
        match entry.tag() {
            gimli::DW_TAG_pointer_type | gimli::DW_TAG_reference_type => {
                Some((self.read_uint(place, PTR_SIZE)?, self.ty_of(&entry)))
            }
            gimli::DW_TAG_structure_type => self
                .members(off)
                .into_iter()
                .find_map(|m| self.find_pointer(m.ty?, &place.offset(m.offset), depth + 1)),
            _ => None,
        }
    }

    fn read_string(&self, ptr: u64, len: u64) -> String {
        if len == 0 {
            return "\"\"".into();
        }
        let take = len.min(MAX_STR);
        match self.read(&Place::Mem(ptr), take) {
            Some(bytes) => {
                let s = String::from_utf8_lossy(&bytes);
                if len > take {
                    format!("{s:?}{ELLIPSIS} (len={len})")
                } else {
                    format!("{s:?}")
                }
            }
            None => UNAVAILABLE.into(),
        }
    }
}

/// `Some(x)` -> `x`, `None` -> `<uninit>` (`OnceCell`'s slot).
fn unwrap_option(value: String, mut children: Vec<Value>) -> (String, Vec<Value>) {
    if value == NONE {
        return (UNINIT.into(), Vec::new());
    }
    if value
        .strip_prefix(SOME)
        .is_some_and(|rest| rest.starts_with('('))
        && children.len() == 1
    {
        let inner = children.remove(0);
        return (inner.value, inner.children);
    }
    (value, children)
}

/// The type a generic instance's qualified name names, without its
/// parameters (`alloc::rc::Rc<u8>` -> `alloc::rc::Rc`).
fn generic_base(qualified: &str) -> &str {
    qualified.split('<').next().unwrap_or(qualified)
}

/// Whether `members` are a tuple's (or tuple variant's) fields.
fn is_tuple<'m>(members: impl IntoIterator<Item = &'m Member>) -> bool {
    members.into_iter().all(|m| m.name.starts_with(TUPLE_FIELD))
}

/// A member's row label: tuple fields as `0`, `1`..., others by name.
fn field_label(member: &Member, tuple: bool) -> String {
    if tuple {
        member.name.trim_start_matches(TUPLE_FIELD).to_string()
    } else {
        member.name.clone()
    }
}

/// Display form of a type name: the allocator parameter and the paths of
/// the most common std types dropped (`Vec<u8>`, not
/// `alloc::vec::Vec<u8, alloc::alloc::Global>`).
pub(crate) fn short_type_name(name: &str) -> String {
    const PREFIXES: [(&str, &str); 12] = [
        ("alloc::vec::Vec<", "Vec<"),
        ("alloc::string::String", "String"),
        ("alloc::boxed::Box<", "Box<"),
        ("alloc::rc::Rc<", "Rc<"),
        ("alloc::sync::Arc<", "Arc<"),
        ("core::option::Option<", "Option<"),
        ("core::result::Result<", "Result<"),
        ("core::cell::Cell<", "Cell<"),
        ("core::cell::RefCell<", "RefCell<"),
        ("core::cell::once::OnceCell<", "OnceCell<"),
        ("std::sync::once_lock::OnceLock<", "OnceLock<"),
        ("core::ptr::non_null::NonNull<", "NonNull<"),
    ];
    let mut out = name.replace(", alloc::alloc::Global", "");
    for (long, short) in PREFIXES {
        out = out.replace(long, short);
    }
    out
}

fn mask(size: u64) -> u64 {
    if size >= 8 {
        !0
    } else {
        (1u64 << (size * 8)) - 1
    }
}

fn attr_u64(v: &AttributeValue<R>) -> Option<u64> {
    match *v {
        AttributeValue::Sdata(x) => Some(x as u64),
        _ => v.udata_value(),
    }
}

fn hex_bytes(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut s = String::with_capacity(2 + 2 * bytes.len());
    s.push_str("0x");
    for b in bytes.iter().rev() {
        let _ = write!(s, "{b:02x}");
    }
    s
}

fn summarize(open: &str, close: &str, items: impl Iterator<Item = String>, more: bool) -> String {
    let mut s = String::from(open);
    let mut first = true;
    let mut truncated = more;
    for item in items {
        if s.chars().count() > SUMMARY_LEN {
            truncated = true;
            break;
        }
        if !first {
            s.push_str(", ");
        }
        first = false;
        s.push_str(&item);
    }
    if truncated {
        if !first {
            s.push_str(", ");
        }
        s.push_str(ELLIPSIS);
    }
    s.push_str(close);
    s
}
