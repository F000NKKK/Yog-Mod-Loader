//! Classes (and structs, and the mod) with their constructors — the
//! fields' first values set in each, after the base's — enums, a method's
//! body, and an `extern` method's call on the bus.

use yog_script_analyze::ast::{
    Chain, Ctor, Decl, DeclKind, Generic, Method, MethodBody, Modifier, Param, TypeHeader, Variant,
};

use super::Gen;
use crate::decls::{Place, generic_list};
use crate::index::Kind;
use crate::types::written;

/// What a struct's objects are copied with.
const COPY: &str = "copy";
/// The bus's call for Java code (`java-common`'s `dev.yog.api.YogBus`).
const BUS_CALL: &str = "dev.yog.api.YogBus.call";

impl Gen<'_> {
    pub(crate) fn class(&mut self, d: &Decl, name: &str, header: &TypeHeader, body: &[Decl]) {
        let struct_ = self.index.kind(name) == Some(Kind::Struct);
        let (extends, interfaces) = self.bases(header);
        let mut implements = interfaces;
        if struct_ {
            implements.push("Cloneable".into());
        }
        let implements = if implements.is_empty() {
            String::new()
        } else {
            format!(" implements {}", implements.join(", "))
        };
        let extends = extends.map(|b| format!(" extends {b}")).unwrap_or_default();
        let lead = if d.has(Modifier::Abstract) {
            "abstract "
        } else if d.has(Modifier::Sealed) || struct_ {
            "final "
        } else {
            ""
        };
        self.code.open(format!(
            "public static {lead}class {name}{}{extends}{implements}",
            generic_list(&header.generics)
        ));
        self.constants(body);
        for f in fields(body) {
            self.code.line(format!(
                "public {} {};",
                written(f.0, false),
                Self::ident(f.1)
            ));
        }
        self.constructors(name, header, body);
        self.members(body, Place::Class);
        if struct_ {
            self.code.open(format!("public {name} {COPY}()"));
            self.code.line(format!("try {{ return ({name}) clone(); }} catch (CloneNotSupportedException e) {{ throw new AssertionError(e); }}"));
            self.code.close("");
        }
        self.code.close("");
    }

    /// `const` fields: Java's `static final`, set where they're declared.
    fn constants(&mut self, body: &[Decl]) {
        for d in body {
            if let DeclKind::Field(f) = &d.kind
                && d.has(Modifier::Const)
                && let Some(v) = &f.value
            {
                let (t, n, v) = (
                    written(&f.ty, false),
                    Self::ident(&f.name.text),
                    self.expr(v),
                );
                self.code
                    .line(format!("public static final {t} {n} = {v};"));
            }
        }
    }

    /// The class's base (`extends`) and interfaces (`implements`).
    fn bases(&self, header: &TypeHeader) -> (Option<String>, Vec<String>) {
        let mut extends = None;
        let mut interfaces = Vec::new();
        for b in &header.bases {
            let is_interface = match &b.kind {
                yog_script_analyze::ast::TypeKind::Path { path, .. } => path
                    .last()
                    .is_some_and(|n| self.index.kind(&n.text) == Some(Kind::Interface)),
                _ => false,
            };
            if is_interface || extends.is_some() {
                interfaces.push(written(b, true));
            } else {
                extends = Some(written(b, true));
            }
        }
        (extends, interfaces)
    }

    /// The primary constructor (when there's no other, or it has
    /// parameters) and each written one.
    fn constructors(&mut self, name: &str, header: &TypeHeader, body: &[Decl]) {
        let ctors: Vec<&Ctor> = body
            .iter()
            .filter_map(|d| match &d.kind {
                DeclKind::Ctor(c) => Some(c),
                _ => None,
            })
            .collect();
        let primary = header.ctor.as_deref().unwrap_or_default();
        if ctors.is_empty() || !primary.is_empty() {
            let params = self.params(primary);
            self.code.open(format!("public {name}({params})"));
            self.code.line("super();");
            self.first_values(body);
            self.code.close("");
        }
        for c in ctors {
            let params = self.params(&c.params);
            self.code.open(format!("public {name}({params})"));
            match &c.chain {
                Some((Chain::This, args)) => {
                    let args = self.args(args);
                    self.code.line(format!("this({args});"));
                }
                other => {
                    let args = match other {
                        Some((_, args)) => self.args(args),
                        None => String::new(),
                    };
                    self.code.line(format!("super({args});"));
                    self.first_values(body);
                }
            }
            self.block_stmts(&c.body);
            self.code.close("");
        }
    }

    /// `this.f = …;` for each field with a first value, in order.
    fn first_values(&mut self, body: &[Decl]) {
        for d in body {
            let (name, value) = match &d.kind {
                DeclKind::Field(_) if d.has(Modifier::Const) => continue,
                DeclKind::Field(f) => (&f.name.text, f.value.as_ref()),
                DeclKind::Property {
                    name,
                    value,
                    accessors,
                    ..
                } if accessors.iter().all(|a| a.body.is_none()) => (&name.text, value.as_ref()),
                _ => continue,
            };
            if let Some(v) = value {
                let v = self.handed_expr(v);
                self.code.line(format!("this.{} = {v};", Self::ident(name)));
            }
        }
    }

    pub(crate) fn enum_decl(
        &mut self,
        name: &str,
        generics: &[Generic],
        variants: &[Variant],
        body: &[Decl],
    ) {
        if self.index.kind(name) == Some(Kind::Enum) {
            let cases: Vec<String> = variants.iter().map(|v| Self::ident(&v.name.text)).collect();
            self.code.open(format!("public enum {name}"));
            self.code.line(format!("{};", cases.join(", ")));
            self.members(body, Place::Fixed);
            self.code.close("");
            return;
        }
        let g = generic_list(generics);
        let cases: Vec<String> = variants
            .iter()
            .map(|v| format!("{name}.{}", Self::ident(&v.name.text)))
            .collect();
        self.code.open(format!(
            "public sealed interface {name}{g} permits {}",
            cases.join(", ")
        ));
        for v in variants {
            let fields = self.params(&v.fields);
            self.code.line(format!(
                "record {}{g}({fields}) implements {name}{g} {{}}",
                Self::ident(&v.name.text)
            ));
        }
        self.members(body, Place::Interface);
        self.code.close("");
    }

    /// A method's body after `head`: a block, or an expression returned.
    pub(crate) fn body(&mut self, head: &str, body: Option<&MethodBody>, void: bool) {
        self.code.open(head);
        match body {
            Some(MethodBody::Block(b)) => self.block_stmts(b),
            Some(MethodBody::Expr(e)) => {
                let v = self.handed_expr(e);
                if void {
                    self.code.line(format!("{v};"))
                } else {
                    self.code.line(format!("return {v};"))
                };
            }
            None => {}
        }
        self.code.close("");
    }

    /// An `extern` `[Service]` method: its request (a record of its
    /// parameters) called on the bus at the service's address.
    pub(crate) fn service(&mut self, d: &Decl, m: &Method, head: &str) {
        let Some(address) = self.service_address(d) else {
            return self.not_yet(
                "an `extern` method without its `[Service]` address",
                m.name.span,
            );
        };
        let request = format!("{}{REQUEST}", m.name.text);
        let fields = self.params(&m.params);
        self.code
            .line(format!("public record {request}({fields}) {{}}"));
        let reply = m
            .ret
            .as_ref()
            .map_or_else(|| "Void".to_string(), |t| written(t, true));
        let args: Vec<String> = m
            .params
            .iter()
            .map(|p: &Param| Self::ident(&p.name.text))
            .collect();
        self.code.open(head);
        let call = format!(
            "{BUS_CALL}({address}, new {request}({}), {reply}.class)",
            args.join(", ")
        );
        if m.ret.is_some() {
            self.code.line(format!("return {call};"))
        } else {
            self.code.line(format!("{call};"))
        };
        self.code.close("");
    }

    /// The address a `[Service("…")]` gives, as a Java string.
    fn service_address(&self, d: &Decl) -> Option<String> {
        let a = d.attributes.iter().find(|a| {
            a.path
                .last()
                .is_some_and(|n| n.text == yog_script_analyze::system::SERVICE)
        })?;
        match &a.args.first()?.value.kind {
            yog_script_analyze::ast::ExprKind::Str(segs) => match segs.as_slice() {
                [yog_script_analyze::ast::StrSeg::Text(t)] => Some(self.code.str(t)),
                _ => None,
            },
            _ => None,
        }
    }
}

/// What a request record is called: the method's name and this.
const REQUEST: &str = "Request";

/// A class's stored fields: its fields and auto-properties.
fn fields(body: &[Decl]) -> Vec<(&yog_script_analyze::ast::Type, &str)> {
    body.iter()
        .filter_map(|d| match &d.kind {
            DeclKind::Field(f) if !d.has(Modifier::Const) => Some((&f.ty, f.name.text.as_str())),
            DeclKind::Property {
                ty,
                name,
                accessors,
                ..
            } if accessors.iter().all(|a| a.body.is_none()) => Some((ty, name.text.as_str())),
            _ => None,
        })
        .collect()
}
