//! Attributes (`ys.md` §3, Attributes and macros): `[Name(args)]` names a
//! class inheriting `System.Attribute`, made with its constructor's
//! arguments — constants, as C#'s.

use yog_ids::script_problem;

use super::Checker;
use super::model::Type;
use crate::ast::{self, Attribute, TypeKind};
use crate::source::token::Span;
use crate::system::SERVICE;

/// The class every attribute class inherits.
const ATTRIBUTE: &str = "Attribute";

impl Checker<'_, '_, '_> {
    pub(super) fn attributes(&mut self, attributes: &[Attribute]) {
        for a in attributes {
            self.attribute(a);
        }
    }

    fn attribute(&mut self, a: &Attribute) {
        let (Some(first), Some(last)) = (a.path.first(), a.path.last()) else {
            return;
        };
        let at = first.span.to(last.span);
        let written = ast::Type {
            kind: TypeKind::Path {
                path: a.path.clone(),
                args: Vec::new(),
            },
            span: at,
        };
        let t = self.type_of(&written);
        let Type::Own(class, _) = &t else {
            if t.known() {
                self.not_attribute(&last.text, at);
            }
            return;
        };
        if !self.own.is_a(class, ATTRIBUTE) {
            return self.not_attribute(class, at);
        }
        let params = self
            .own
            .named(class)
            .map(|(_, o)| o.ctor.clone())
            .unwrap_or_default();
        // `[Derive(Clone)]` names macros, not values: a class without a
        // constructor takes what its macro reads.
        if params.is_empty() {
            return;
        }
        if a.args.len() > params.len() {
            self.report.error(
                script_problem::WRONG_ARGS,
                at,
                format!("`{class}` takes {} arguments", params.len()),
            );
        }
        for (arg, p) in a.args.iter().zip(&params) {
            let wanted = self.type_of(&p.ty);
            self.fits(&arg.value, &wanted);
        }
    }

    /// An `extern` method is a call on the bus: it says where.
    pub(super) fn extern_has_service(&mut self, d: &crate::ast::Decl) {
        let crate::ast::DeclKind::Method(m) = &d.kind else {
            return;
        };
        let service = d
            .attributes
            .iter()
            .any(|a| a.path.last().is_some_and(|n| n.text == SERVICE));
        if d.has(crate::ast::Modifier::Extern) != service {
            self.report.error(
                script_problem::ATTRIBUTE,
                m.name.span,
                format!("an `extern` method is a call on the bus, and a `[{SERVICE}]` one is `extern`: `[{SERVICE}(\"mod:address\")] static extern …`"),
            );
        }
    }

    fn not_attribute(&mut self, name: &str, at: Span) {
        self.report.error(
            script_problem::ATTRIBUTE,
            at,
            format!("`{name}` isn't an attribute: an attribute is a class inheriting `System.{ATTRIBUTE}`"),
        );
    }
}
