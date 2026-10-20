//! Reading a value against a schema: its problems, with their paths.
use super::{LineCol, MapRef, Path, Ref, Res, Schema, Seg, Text, ValueRef, fmt};

// ── Reading ─────────────────────────────────────────────────────────────────

/// A problem found reading a value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub path: Path,
    pub message: String,
    /// About the key at `path` rather than its value (an unknown field).
    pub at_key: bool,
}

/// Where a reference was found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found<T> {
    pub value: T,
    pub path: Path,
}

/// The state of reading a value: where it is, the problems so far, and the
/// references met (for the build to check against the content there is).
#[derive(Default)]
pub struct Reader<'r> {
    /// Where it is, as steps: a field's name stays `&'static` until a
    /// problem or a reference needs the whole [`Path`].
    pub(super) path: Vec<Step>,
    /// Don't keep the references, tags and resources met (who only wants the value).
    pub(super) value_only: bool,
    /// What the tool makes of references and texts (none: kept as written).
    pub(super) resolver: Option<&'r mut dyn Resolve>,
    pub problems: Vec<Problem>,
    pub refs: Vec<Found<Ref>>,
    pub tags: Vec<Found<Ref>>,
    pub resources: Vec<Found<Res>>,
    /// Keys a struct being read doesn't report as unknown (an enum's tag).
    pub(super) allowed: Vec<&'static str>,
}

/// A step of [`Reader`]'s path.
pub(super) enum Step {
    Seg(Seg),
    Field(&'static str),
}

/// Source-only keys every object may have: `$…`, `_…`, `comment`.
pub fn is_metadata_key(key: &str) -> bool {
    key.starts_with('$') || key.starts_with('_') || key == "comment"
}

/// What a tool makes of the references and texts it reads: `yog build`
/// checks each reference against the content there is and turns texts
/// into translation keys; the IDE shows them.
pub trait Resolve {
    /// A reference: the id to keep, or why it's wrong.
    fn reference(&mut self, r: &Ref, _path: &Path) -> Result<String, String> {
        Ok(r.id.clone())
    }

    /// A text: what to keep (its words, a translation key, ...), or why it's wrong.
    fn text(&mut self, t: &Text, _path: &Path) -> Result<String, String> {
        Ok(match t {
            Text::Literal(s) => s.clone(),
            Text::Res(r) => format!("@{}.{}", r.set, r.key),
            Text::NameOf(r) => format!("@{}:{}", r.kind, r.id),
            Text::Lang(k) => format!("@lang:{k}"),
        })
    }

    /// The words of a `.yres` text, if known (to check what's in them).
    fn words(&mut self, _r: &Res) -> Option<String> {
        None
    }
}

impl<'r> Reader<'r> {
    pub fn new() -> Self {
        Self::default()
    }

    /// A reader handing references and texts to `resolver`.
    pub fn with_resolver(resolver: &'r mut dyn Resolve) -> Self {
        Self {
            resolver: Some(resolver),
            ..Self::default()
        }
    }

    /// `r` resolved (a problem if it's wrong); its id without a resolver.
    pub fn resolve_ref(&mut self, r: &Ref) -> Option<String> {
        let result = match &mut self.resolver {
            Some(res) => res.reference(r, &Self::built(&self.path)),
            None => Ok(r.id.clone()),
        };
        result.map_err(|e| self.problem(e)).ok()
    }

    /// `t` resolved (a problem if it's wrong).
    pub fn resolve_text(&mut self, t: &Text) -> Option<String> {
        struct Keep;
        impl Resolve for Keep {}
        let result = match &mut self.resolver {
            Some(res) => res.text(t, &Self::built(&self.path)),
            None => Keep.text(t, &Path::root()),
        };
        result.map_err(|e| self.problem(e)).ok()
    }

    /// [`Reader::resolve_text`], taking `t`: without a resolver its words
    /// move into the result instead of being copied.
    pub fn take_text(&mut self, t: Text) -> Option<String> {
        match (t, self.resolver.is_some()) {
            (Text::Literal(s), false) => Some(s),
            (t, _) => self.resolve_text(&t),
        }
    }

    /// [`Reader::resolve_ref`], taking `r`: without a resolver the id moves.
    pub fn take_ref(&mut self, r: Ref) -> Option<String> {
        if self.resolver.is_none() {
            return Some(r.id);
        }
        self.resolve_ref(&r)
    }

    /// Whether references are kept for the caller ([`read_value`] doesn't).
    pub fn keeps_found(&self) -> bool {
        !self.value_only
    }

    /// Whether references and texts go through a resolver (else kept as written).
    pub fn resolves(&self) -> bool {
        self.resolver.is_some()
    }

    /// The words of `.yres` text `r`, if the resolver knows them.
    pub fn words(&mut self, r: &Res) -> Option<String> {
        self.resolver.as_mut()?.words(r)
    }

    /// Where the reader is.
    pub fn path(&self) -> Path {
        Self::built(&self.path)
    }

    fn built(steps: &[Step]) -> Path {
        let mut p = Path::root();
        for s in steps {
            p.push(match s {
                Step::Seg(s) => s.clone(),
                Step::Field(k) => Seg::Key((*k).to_string()),
            });
        }
        p
    }

    /// Reports a problem with the value being read.
    pub fn problem(&mut self, message: impl Into<String>) {
        self.problems.push(Problem {
            path: self.path(),
            message: message.into(),
            at_key: false,
        });
    }

    /// "expected X, found Y".
    pub fn expected(&mut self, what: &str, found: &ValueRef) {
        self.problem(format!("expected {what}, found {}", found_text(found)));
    }

    /// Runs `f` one step deeper.
    pub fn at<T>(&mut self, seg: Seg, f: impl FnOnce(&mut Self) -> T) -> T {
        self.path.push(Step::Seg(seg));
        let r = f(self);
        self.path.pop();
        r
    }

    /// Runs `f` in field `key` (no allocation for the step).
    fn in_field<T>(&mut self, key: &'static str, f: impl FnOnce(&mut Self) -> T) -> T {
        self.path.push(Step::Field(key));
        let r = f(self);
        self.path.pop();
        r
    }

    pub fn note_ref(&mut self, r: Ref) {
        if self.value_only {
            return;
        }
        let path = self.path();
        self.refs.push(Found { value: r, path });
    }

    pub fn note_tag(&mut self, r: Ref) {
        if self.value_only {
            return;
        }
        let path = self.path();
        self.tags.push(Found { value: r, path });
    }

    pub fn note_res(&mut self, r: Res) {
        if self.value_only {
            return;
        }
        let path = self.path();
        self.resources.push(Found { value: r, path });
    }

    /// `v` as an object, or a problem.
    pub fn map<'v>(&mut self, v: &ValueRef<'v>) -> Option<MapRef<'v>> {
        match v {
            ValueRef::Map(m) => Some(*m),
            other => {
                self.expected("an object", other);
                None
            }
        }
    }

    /// Reports keys of `map` that aren't `known` (nor metadata, nor allowed by an enclosing enum).
    pub fn unknown_keys(&mut self, map: MapRef, known: &[&str]) {
        for key in map.keys() {
            if known.contains(&key) || is_metadata_key(key) || self.allowed.contains(&key) {
                continue;
            }
            let hint = closest(key, known)
                .map(|k| format!(" — did you mean `{k}`?"))
                .unwrap_or_else(|| {
                    if known.is_empty() {
                        String::new()
                    } else {
                        format!(
                            " (expected {})",
                            known
                                .iter()
                                .map(|k| format!("`{k}`"))
                                .collect::<Vec<_>>()
                                .join(", ")
                        )
                    }
                });
            self.problems.push(Problem {
                path: self.path().key(key),
                message: format!("unknown field `{key}`{hint}"),
                at_key: true,
            });
        }
    }

    /// Reports a name that isn't one of `known` (an enum's variant).
    pub fn unknown_name(&mut self, name: &str, known: &[&str]) {
        let hint = match closest(name, known) {
            Some(k) => format!(" — did you mean `{k}`?"),
            None => format!(
                " (expected {})",
                known
                    .iter()
                    .map(|k| format!("`{k}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        };
        self.problem(format!("unknown `{name}`{hint}"));
    }

    /// Field `key` of `map`: read, or its absent value, or a problem.
    pub fn field<T: Schema>(&mut self, map: MapRef, key: &'static str) -> Option<T> {
        match map.get(key) {
            Some(v) => self.at(Seg::Key(key.to_string()), |cx| T::read(v, cx)),
            None => T::absent().or_else(|| {
                self.problem(format!("missing `{key}`"));
                None
            }),
        }
    }

    /// Field `key` of `map`, `default()` when it isn't there.
    pub fn field_or<T: Schema>(
        &mut self,
        map: MapRef,
        key: &'static str,
        default: impl FnOnce() -> T,
    ) -> Option<T> {
        match map.get(key) {
            Some(v) => self.at(Seg::Key(key.to_string()), |cx| T::read(v, cx)),
            None => Some(default()),
        }
    }

    /// Field `key` of `map` through codec `W`.
    pub fn field_with<W: crate::with::With<F>, F>(
        &mut self,
        map: MapRef,
        key: &'static str,
    ) -> Option<F> {
        match map.get(key) {
            Some(v) => self.in_field(key, |cx| W::read(v, cx)),
            None => W::absent().or_else(|| {
                self.problem(format!("missing `{key}`"));
                None
            }),
        }
    }

    /// Field `key` of `map` through codec `W`, `default()` when it isn't there.
    pub fn field_or_with<W: crate::with::With<F>, F>(
        &mut self,
        map: MapRef,
        key: &'static str,
        default: impl FnOnce() -> F,
    ) -> Option<F> {
        match map.get(key) {
            Some(v) => self.in_field(key, |cx| W::read(v, cx)),
            None => Some(default()),
        }
    }

    /// Reads `v` as `T` with `key` allowed in it — the tag of the enum variant `T` is.
    pub fn with_allowed<T>(&mut self, key: &'static str, f: impl FnOnce(&mut Self) -> T) -> T {
        self.allowed.push(key);
        let r = f(self);
        self.allowed.pop();
        r
    }

    /// Reads with every one of `keys` allowed (the fields of a struct read
    /// from the same object — see [`read_both`]).
    pub fn with_allowed_all<T>(
        &mut self,
        keys: &[&'static str],
        f: impl FnOnce(&mut Self) -> T,
    ) -> T {
        let before = self.allowed.len();
        self.allowed.extend_from_slice(keys);
        let r = f(self);
        self.allowed.truncate(before);
        r
    }

    /// Reads a struct's own keys with no allowance inherited from outside.
    pub fn own_keys<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> T {
        let saved = std::mem::take(&mut self.allowed);
        let r = f(self);
        self.allowed = saved;
        r
    }
}

fn found_text(v: &ValueRef) -> String {
    match v {
        ValueRef::Str(s) if s.chars().count() <= 40 => format!("{s:?}"),
        ValueRef::Int(n) => n.to_string(),
        ValueRef::Float(f) => f.to_string(),
        ValueRef::Bool(b) => b.to_string(),
        other => other.kind_name().to_string(),
    }
}

/// The `known` name closest to `key`, if it's close enough to be a typo
/// (`txt` → `text`, `img` → `image`, `colr` → `color`).
fn closest<'k>(key: &str, known: &[&'k str]) -> Option<&'k str> {
    let lower = key.to_ascii_lowercase();
    known
        .iter()
        .map(|k| (edit_distance(&lower, k), *k))
        .filter(|(d, k)| *d <= if k.len() <= 3 { 1 } else { k.len() / 2 })
        .min_by_key(|(d, _)| *d)
        .map(|(_, k)| k)
}

fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut cur = vec![i + 1; b.len() + 1];
        for (j, cb) in b.iter().enumerate() {
            cur[j + 1] = (prev[j] + usize::from(ca != *cb))
                .min(prev[j + 1] + 1)
                .min(cur[j] + 1);
        }
        prev = cur;
    }
    prev[b.len()]
}

/// Problems found reading a document, with where each is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Errors(pub Vec<Located>);

/// A problem and its place in the source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Located {
    pub at: Option<LineCol>,
    pub path: Path,
    pub message: String,
}

impl fmt::Display for Located {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(at) = self.at {
            write!(f, "{at}: ")?;
        }
        if !self.path.is_root() {
            write!(f, "{}: ", self.path)?;
        }
        f.write_str(&self.message)
    }
}

impl fmt::Display for Errors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, e) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str("\n")?;
            }
            write!(f, "{e}")?;
        }
        Ok(())
    }
}

impl std::error::Error for Errors {}

/// What reading a document gives: the value and the references in it.
#[derive(Debug)]
pub struct Read<T> {
    pub value: T,
    pub refs: Vec<Found<Ref>>,
    pub tags: Vec<Found<Ref>>,
    pub resources: Vec<Found<Res>>,
}

/// The numbers a value holds — what `#[data(range = …)]` checks.
pub trait Numbers {
    fn numbers(&self, out: &mut Vec<f64>);
}

macro_rules! numbers {
    ($($t:ty),*) => {$(
        impl Numbers for $t {
            fn numbers(&self, out: &mut Vec<f64>) {
                out.push(*self as f64);
            }
        }
    )*};
}
numbers!(i8, i16, i32, i64, u8, u16, u32, u64, usize, f32, f64);

impl<T: Numbers> Numbers for Option<T> {
    fn numbers(&self, out: &mut Vec<f64>) {
        if let Some(x) = self {
            x.numbers(out);
        }
    }
}

impl<T: Numbers> Numbers for Vec<T> {
    fn numbers(&self, out: &mut Vec<f64>) {
        self.iter().for_each(|x| x.numbers(out));
    }
}

impl<T: Numbers, const N: usize> Numbers for [T; N] {
    fn numbers(&self, out: &mut Vec<f64>) {
        self.iter().for_each(|x| x.numbers(out));
    }
}

impl Reader<'_> {
    /// `value` (field `key`) checked against `lo..=hi`; `None` (a problem) if it's out.
    pub fn in_range<T: Numbers>(
        &mut self,
        key: &str,
        value: Option<T>,
        lo: f64,
        hi: f64,
    ) -> Option<T> {
        let v = value?;
        let mut ns = Vec::new();
        v.numbers(&mut ns);
        match ns.into_iter().find(|n| !(lo..=hi).contains(n)) {
            None => Some(v),
            Some(n) => {
                let show = |x: f64| {
                    if x.fract() == 0.0 {
                        format!("{}", x as i64)
                    } else {
                        x.to_string()
                    }
                };
                let bad = show(n);
                self.at(Seg::Key(key.to_string()), |cx| {
                    cx.problem(format!(
                        "{bad} is out of range ({}..={})",
                        show(lo),
                        show(hi)
                    ))
                });
                None
            }
        }
    }
}
