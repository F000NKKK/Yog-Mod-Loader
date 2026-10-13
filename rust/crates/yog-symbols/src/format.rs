//! Type-aware formatting of a located variable into a bounded [`Value`]
//! tree, with Rust-specific pretty forms for `&str`, slices, `String`,
//! `Vec<T>`, `Box<T>`, enums with variant parts, and the std wrappers a
//! user doesn't want to click through (`Cell`, `RefCell`, `OnceLock`,
//! `Rc`/`Arc`, `NonNull`, `PhantomData`...).

use std::cell::Cell;

use gimli::{AttributeValue, DebuggingInformationEntry, Reader as _, UnitOffset};

mod nav;

pub(crate) use nav::Slot;

use crate::locals::{unit_ref, Place, TargetMemory, UnitInfo, Value, UNAVAILABLE};
use crate::{SymbolTable, R};

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

    fn entry(&self, off: Off) -> Option<Die> {
        self.info.unit.entry(off).ok()
    }

    fn name_of(&self, entry: &Die) -> Option<String> {
        let attr = entry.attr_value(gimli::DW_AT_name)?;
        let s = self.table.dwarf.attr_string(&self.info.unit, attr).ok()?;
        Some(s.to_string_lossy().ok()?.into_owned())
    }

    fn ty_of(&self, entry: &Die) -> Option<Off> {
        unit_ref(self.info, entry.attr_value(gimli::DW_AT_type)?)
    }

    fn udata(entry: &Die, at: gimli::DwAt) -> Option<u64> {
        entry.attr_value(at)?.udata_value()
    }

    /// Strips typedefs and cv-qualifiers down to the type that decides
    /// the representation.
    fn peel(&self, mut off: Off) -> Option<(Off, Die)> {
        for _ in 0..MAX_PEEL {
            let entry = self.entry(off)?;
            match entry.tag() {
                gimli::DW_TAG_typedef
                | gimli::DW_TAG_const_type
                | gimli::DW_TAG_volatile_type
                | gimli::DW_TAG_restrict_type
                | gimli::DW_TAG_atomic_type => match self.ty_of(&entry) {
                    Some(next) => off = next,
                    None => return Some((off, entry)),
                },
                _ => return Some((off, entry)),
            }
        }
        None
    }

    fn byte_size(&self, off: Off) -> Option<u64> {
        let (_, entry) = self.peel(off)?;
        if let Some(size) = Self::udata(&entry, gimli::DW_AT_byte_size) {
            return Some(size);
        }
        match entry.tag() {
            gimli::DW_TAG_pointer_type
            | gimli::DW_TAG_reference_type
            | gimli::DW_TAG_rvalue_reference_type => Some(PTR_SIZE),
            gimli::DW_TAG_array_type => {
                let (elem, count) = self.array_shape(&entry)?;
                self.array_stride(&entry, elem?)?.checked_mul(count)
            }
            _ => None,
        }
    }

    fn children(&self, off: Off) -> Vec<Die> {
        let mut out = Vec::new();
        let Ok(mut tree) = self.info.unit.entries_tree(Some(off)) else {
            return out;
        };
        let Ok(root) = tree.root() else {
            return out;
        };
        let mut iter = root.children();
        while let Ok(Some(child)) = iter.next() {
            out.push(child.entry().clone());
            if out.len() > MAX_CHILDREN {
                break;
            }
        }
        out
    }

    fn members(&self, off: Off) -> Vec<Member> {
        self.children(off)
            .iter()
            .filter(|e| e.tag() == gimli::DW_TAG_member)
            .filter(|e| !e.has_attr(gimli::DW_AT_external) && !e.has_attr(gimli::DW_AT_declaration))
            .map(|e| self.member(e))
            .collect()
    }

    fn member(&self, e: &Die) -> Member {
        Member {
            name: self.name_of(e).unwrap_or_default(),
            ty: self.ty_of(e),
            offset: Self::udata(e, gimli::DW_AT_data_member_location).unwrap_or(0),
        }
    }

    /// The member named `name` (a struct's own, not inherited).
    fn member_named<'m>(members: &'m [Member], name: &str) -> Option<&'m Member> {
        members.iter().find(|m| m.name == name)
    }

    /// The only member of a wrapper that holds data: `value` for the cells
    /// and `MaybeUninit` (a union whose other member is the `()` "uninit"
    /// view), otherwise the one non-zero-sized field.
    fn wrapped_member<'m>(&self, members: &'m [Member]) -> Option<&'m Member> {
        Self::member_named(members, "value").or_else(|| {
            let mut data = members.iter().filter(|m| !self.is_zero_sized(m.ty));
            match (data.next(), data.next()) {
                (Some(only), None) => Some(only),
                _ => None,
            }
        })
    }

    /// Whether the `OnceLock` whose members are `members` at `place` has
    /// been initialized; `None` if its state can't be read. Both `Once`
    /// implementations (futex and queue) keep the state in the low two
    /// bits; COMPLETE is 0 (since Rust 1.80, when INCOMPLETE became 3).
    fn once_lock_complete(&self, members: &[Member], place: &Place) -> Option<bool> {
        const STATE_MASK: u64 = 3;
        let once = Self::member_named(members, "once")?;
        let state = self.first_uint(once.ty?, &place.offset(once.offset), 0)?;
        Some(state & STATE_MASK == 0)
    }

    /// The payload member of an `Rc`/`Arc` allocation (`RcInner` /
    /// `ArcInner`).
    fn rc_payload(members: &[Member]) -> Option<&Member> {
        Self::member_named(members, "value").or_else(|| Self::member_named(members, "data"))
    }

    /// Whether a pointer to `pointee` can be followed: not a function or an
    /// opaque/unsized type.
    fn is_derefable(&self, pointee: Off) -> bool {
        self.peel(pointee).is_some_and(|(off, e)| {
            !matches!(
                e.tag(),
                gimli::DW_TAG_subroutine_type | gimli::DW_TAG_unspecified_type
            ) && self.byte_size(off).is_some_and(|s| s > 0)
        })
    }

    /// Distance between elements of the array type `entry` of `elem`s.
    fn array_stride(&self, entry: &Die, elem: Off) -> Option<u64> {
        Self::udata(entry, gimli::DW_AT_byte_stride).or_else(|| self.byte_size(elem))
    }

    fn template_param(&self, off: Off, name: &str) -> Option<Off> {
        self.children(off)
            .iter()
            .filter(|e| e.tag() == gimli::DW_TAG_template_type_parameter)
            .find(|e| self.name_of(e).as_deref() == Some(name))
            .and_then(|e| self.ty_of(e))
    }

    /// Element type and element count of an array type (first dimension).
    fn array_shape(&self, entry: &Die) -> Option<(Option<Off>, u64)> {
        let elem = self.ty_of(entry);
        let sub = self
            .children(entry.offset())
            .into_iter()
            .find(|e| e.tag() == gimli::DW_TAG_subrange_type)?;
        let count = match Self::udata(&sub, gimli::DW_AT_count) {
            Some(c) => c,
            None => {
                let lower = Self::udata(&sub, gimli::DW_AT_lower_bound).unwrap_or(0);
                let upper = sub.attr_value(gimli::DW_AT_upper_bound)?.udata_value()?;
                upper.checked_sub(lower)?.checked_add(1)?
            }
        };
        Some((elem, count))
    }

    /// `entry`'s name with the namespaces and types enclosing it
    /// (`alloc::vec::Vec<u8, alloc::alloc::Global>`), memoized per unit.
    pub(crate) fn qualified_name(&self, off: Off, entry: &Die) -> String {
        let cache = || self.info.qualified_names.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(name) = cache().get(&off.0) {
            return name.clone();
        }
        let name = self.build_qualified_name(off, entry);
        cache().insert(off.0, name.clone());
        name
    }

    fn build_qualified_name(&self, off: Off, entry: &Die) -> String {
        let mut parts = vec![self.name_of(entry).unwrap_or_else(|| "<anon>".into())];
        let parents = self.info.parents();
        let mut cur = off.0;
        for _ in 0..MAX_QUALIFIERS {
            let Some(&parent) = parents.get(&cur) else {
                break;
            };
            let Some(p) = self.entry(UnitOffset(parent)) else {
                break;
            };
            match p.tag() {
                gimli::DW_TAG_namespace
                | gimli::DW_TAG_structure_type
                | gimli::DW_TAG_union_type
                | gimli::DW_TAG_enumeration_type
                | gimli::DW_TAG_class_type => {
                    parts.push(self.name_of(&p).unwrap_or_else(|| "{anon}".into()));
                }
                _ => break,
            }
            cur = parent;
        }
        parts.reverse();
        parts.join("::")
    }

    // ---- type names --------------------------------------------------

    pub(crate) fn type_name(&self, ty: Option<Off>) -> String {
        self.type_name_depth(ty, 0)
    }

    fn type_name_depth(&self, ty: Option<Off>, depth: usize) -> String {
        let Some(off) = ty else {
            return "()".into();
        };
        if depth > MAX_TYPE_DEPTH {
            return ELLIPSIS.into();
        }
        let Some(entry) = self.entry(off) else {
            return "?".into();
        };
        match entry.tag() {
            gimli::DW_TAG_structure_type
            | gimli::DW_TAG_union_type
            | gimli::DW_TAG_enumeration_type
            | gimli::DW_TAG_class_type
            | gimli::DW_TAG_typedef => {
                let name = self.name_of(&entry);
                // Rust's fat pointers (`&str`, `&[T]`, `&dyn Trait`) and
                // tuples are named structs at CU level: keep them bare.
                match name {
                    Some(n) if n.starts_with(['&', '*', '(', '[']) => n,
                    _ => self.qualified_name(off, &entry),
                }
            }
            gimli::DW_TAG_pointer_type
            | gimli::DW_TAG_reference_type
            | gimli::DW_TAG_rvalue_reference_type => self.name_of(&entry).unwrap_or_else(|| {
                let sigil = if entry.tag() == gimli::DW_TAG_pointer_type {
                    "*"
                } else {
                    "&"
                };
                format!(
                    "{sigil}{}",
                    self.type_name_depth(self.ty_of(&entry), depth + 1)
                )
            }),
            gimli::DW_TAG_const_type => format!(
                "const {}",
                self.type_name_depth(self.ty_of(&entry), depth + 1)
            ),
            gimli::DW_TAG_volatile_type
            | gimli::DW_TAG_restrict_type
            | gimli::DW_TAG_atomic_type => self.type_name_depth(self.ty_of(&entry), depth + 1),
            gimli::DW_TAG_array_type => self.name_of(&entry).unwrap_or_else(|| {
                let elem = self.type_name_depth(self.ty_of(&entry), depth + 1);
                match self.array_shape(&entry) {
                    Some((_, n)) => format!("[{elem}; {n}]"),
                    None => format!("[{elem}]"),
                }
            }),
            gimli::DW_TAG_subroutine_type => self.name_of(&entry).unwrap_or_else(|| "fn".into()),
            _ => self.name_of(&entry).unwrap_or_else(|| "?".into()),
        }
    }

    // ---- values ------------------------------------------------------

    fn read(&self, place: &Place, len: u64) -> Option<Vec<u8>> {
        place.read(self.mem, usize::try_from(len).ok()?)
    }

    fn read_uint(&self, place: &Place, size: u64) -> Option<u64> {
        if size == 0 || size > 8 {
            return None;
        }
        let bytes = self.read(place, size)?;
        let mut buf = [0u8; 8];
        buf[..bytes.len()].copy_from_slice(&bytes);
        Some(u64::from_le_bytes(buf))
    }

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
        Some(self.format_elements(elem, place, stride, count, depth, false))
    }

    /// Children `[0]..[n]` plus a `[a, b, c]` summary (prefixed with the
    /// length for dynamically sized sequences).
    fn format_elements(
        &self,
        elem: Off,
        base: &Place,
        stride: u64,
        count: u64,
        depth: usize,
        show_len: bool,
    ) -> (String, Vec<Value>) {
        let prefix = if show_len {
            format!("len={count} ")
        } else {
            String::new()
        };
        if depth >= MAX_DEPTH {
            return (format!("{prefix}[{ELLIPSIS}]"), Vec::new());
        }
        let shown = count.min(MAX_ELEMS);
        let children: Vec<Value> = (0..shown)
            .map(|i| {
                self.format(
                    format!("[{i}]"),
                    Some(elem),
                    &base.offset(i.wrapping_mul(stride)),
                    depth + 1,
                )
            })
            .collect();
        let more = count > shown;
        let summary = summarize("[", "]", children.iter().map(|c| c.value.clone()), more);
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
                    return Some(self.format_elements(
                        elem,
                        &Place::Mem(ptr),
                        stride,
                        n,
                        depth,
                        true,
                    ));
                }
            }
        }

        let qualified = self.qualified_name(off, entry);
        if qualified == STRING {
            if let Some((ptr, len)) =
                member("vec").and_then(|vec| self.vec_parts(vec.ty?, &place.offset(vec.offset)))
            {
                return Some((self.read_string(ptr, len), Vec::new()));
            }
        }
        if generic_base(&qualified) == VEC {
            if let (Some(elem), Some((ptr, len))) =
                (self.template_param(off, "T"), self.vec_parts(off, place))
            {
                let stride = self.byte_size(elem)?;
                return Some(self.format_elements(
                    elem,
                    &Place::Mem(ptr),
                    stride,
                    len,
                    depth,
                    true,
                ));
            }
        }
        if generic_base(&qualified) == BOX {
            if let Some((ptr, pointee)) = self.find_pointer(off, place, 0) {
                return Some(self.format_pointer("", ptr, pointee, depth));
            }
        }
        if let Some(wrapped) = self.format_wrapper(off, &qualified, &members, place, depth) {
            return Some(wrapped);
        }

        Some(self.format_fields(&members, place, depth, &short))
    }

    /// std wrappers shown as what they wrap; `None` to fall back to the
    /// plain field view (unknown type, or a layout this doesn't recognize).
    fn format_wrapper(
        &self,
        off: Off,
        qualified: &str,
        members: &[Member],
        place: &Place,
        depth: usize,
    ) -> Option<(String, Vec<Value>)> {
        let base = generic_base(qualified);
        let member = |n: &str| Self::member_named(members, n);
        match base {
            _ if TRANSPARENT_WRAPPERS.contains(&base) => {
                Some(self.unwrap_member(self.wrapped_member(members)?, place, depth))
            }
            NON_NULL | UNIQUE => {
                let (ptr, pointee) = self.find_pointer(off, place, 0)?;
                Some(self.format_pointer("", ptr, pointee, depth))
            }
            PHANTOM_DATA | PHANTOM_PINNED => {
                Some((base.rsplit("::").next()?.to_string(), Vec::new()))
            }
            ONCE_CELL => {
                let (value, children) = self.unwrap_member(member("inner")?, place, depth);
                Some(unwrap_option(value, children))
            }
            ONCE_LOCK => {
                let value = member("value")?;
                match self.once_lock_complete(members, place) {
                    Some(false) => Some((UNINIT.into(), Vec::new())),
                    // Initialized, or a state this can't read: show the slot.
                    _ => Some(self.unwrap_member(value, place, depth)),
                }
            }
            RC | ARC => {
                let (ptr, inner_ty) = self.find_pointer(off, place, 0)?;
                if ptr == 0 {
                    return Some((NULL.into(), Vec::new()));
                }
                let (inner_off, _) = self.peel(inner_ty?)?;
                let inner = self.members(inner_off);
                let strong = Self::member_named(&inner, "strong")?;
                let data = Self::rc_payload(&inner)?;
                let count = self.first_uint(strong.ty?, &Place::Mem(ptr + strong.offset), 0);
                let count = count.map_or_else(|| "?".into(), |n| n.to_string());
                let (value, children) = self.unwrap_member(data, &Place::Mem(ptr), depth);
                Some((format!("{value} (strong={count})"), children))
            }
            _ => None,
        }
    }

    /// `member` of the struct at `place`, formatted at the struct's own
    /// depth (the wrapper doesn't count as a nesting level).
    fn unwrap_member(&self, member: &Member, place: &Place, depth: usize) -> (String, Vec<Value>) {
        let inner = self.format(
            member.name.clone(),
            member.ty,
            &place.offset(member.offset),
            depth,
        );
        (inner.value, inner.children)
    }

    /// The first integer reachable through leading struct members (an
    /// atomic's or `Cell`'s payload).
    fn first_uint(&self, ty: Off, place: &Place, depth: usize) -> Option<u64> {
        if depth > MAX_TYPE_DEPTH {
            return None;
        }
        let (off, entry) = self.peel(ty)?;
        match entry.tag() {
            gimli::DW_TAG_base_type => {
                self.read_uint(place, Self::udata(&entry, gimli::DW_AT_byte_size)?)
            }
            gimli::DW_TAG_structure_type | gimli::DW_TAG_union_type => {
                let first = self.members(off).into_iter().next()?;
                self.first_uint(first.ty?, &place.offset(first.offset), depth + 1)
            }
            _ => None,
        }
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
