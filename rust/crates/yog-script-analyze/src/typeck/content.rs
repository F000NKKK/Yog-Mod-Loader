//! A content declaration's properties (`block Charger { Hardness = 2.0f; }`)
//! against its kind's definition (`yog-registry`'s, the Rust API's): each
//! a field of it by its PascalCase name, its value of the field's type. The
//! id is the declaration's own; a kind Yog Script doesn't declare (a
//! mod's own) isn't checked.

use yog_data::schema::Shape;
use yog_ids::script_problem;

use super::Checker;
use super::model::Type;
use crate::ast::{Decl, DeclKind, Prop};

/// How wide an `f32` is, in bits: a field of that width is one.
const F32_BITS: u32 = u8::BITS * std::mem::size_of::<f32>() as u32;

/// The field a definition's id is, set by the declaration itself.
const ID_FIELD: &str = "id";

impl Checker<'_, '_, '_> {
    /// The properties of content of kind `word` in `body`, checked.
    pub(super) fn content_props(&mut self, word: &str, body: &[Decl]) {
        let Some(kind) = yog_registry::content_kind(word) else {
            return;
        };
        let Shape::Struct(def) = (kind.shape)() else {
            return;
        };
        for d in body {
            let DeclKind::Prop(p) = &d.kind else {
                continue;
            };
            let field = yog_codegen::snake(&p.name.text);
            match def
                .fields
                .iter()
                .find(|f| f.name == field && f.name != ID_FIELD)
            {
                Some(f) => {
                    let t = of_shape(&(f.shape)());
                    self.fits(&p.value, &t);
                }
                None => self.unknown_prop(word, p),
            }
        }
    }

    fn unknown_prop(&mut self, word: &str, p: &Prop) {
        self.expr(&p.value);
        self.report.error(
            script_problem::UNKNOWN_MEMBER,
            p.name.span,
            format!("a `{word}` has no `{}`", p.name.text),
        );
    }
}

/// A definition's field's shape as a type a value fits (`Unknown`: any,
/// what a property can't be checked as yet).
fn of_shape(s: &Shape) -> Type {
    match s {
        Shape::Bool => Type::Bool,
        Shape::Int { min, max } if *min >= i64::from(i32::MIN) && *max <= i64::from(i32::MAX) => {
            Type::I32
        }
        Shape::Int { .. } => Type::I64,
        Shape::Float { bits } if *bits == F32_BITS => Type::F32,
        Shape::Float { .. } => Type::F64,
        Shape::Str | Shape::Atom | Shape::Res => Type::String,
        Shape::Text => Type::Text,
        Shape::Ref([kind]) => Type::Content((*kind).to_string()),
        Shape::Optional(inner) => match of_shape(&inner()) {
            Type::Unknown => Type::Unknown,
            t => Type::Nullable(Box::new(t)),
        },
        Shape::List(item) => match of_shape(&item()) {
            Type::Unknown => Type::Unknown,
            t => Type::Array(Box::new(t), 1),
        },
        _ => Type::Unknown,
    }
}
