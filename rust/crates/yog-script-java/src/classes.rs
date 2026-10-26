//! Classes (and structs, and the mod) with their constructors — the
//! fields' first values set in each, after the base's — enums, a method's
//! body, and an `extern` method's call on the bus.

use yog_script_analyze::ast::{
    Chain, Ctor, Decl, DeclKind, Method, MethodBody, Modifier, Param, TypeHeader,
};

use super::Gen;
use crate::decls::{Place, generic_list};
use crate::index::Kind;

use yog_script_ids::jvm::reflection::{TYPE, TYPED, TYPES_FIELD};
use yog_script_ids::jvm::{BUS_CALL, CLONEABLE, COPY, REQUEST};

/// A type argument not known (`Box<?>`).
const WILDCARD: &str = "?";
/// Java's word that a cast its generics can't check is known safe.
const UNCHECKED: &str = "@SuppressWarnings(\"unchecked\")";

impl Gen<'_> {
    /// A `mod`: a class the runtime finds and makes (`YsMod`).
    pub(crate) fn mod_class(&mut self, d: &Decl, name: &str, body: &[Decl]) {
        self.class_with(
            d,
            (name, &TypeHeader::default()),
            body,
            Some(crate::MOD_INTERFACE),
        );
    }

    pub(crate) fn class(&mut self, d: &Decl, name: &str, header: &TypeHeader, body: &[Decl]) {
        self.class_with(d, (name, header), body, None);
    }

    fn class_with(
        &mut self,
        d: &Decl,
        (name, header): (&str, &TypeHeader),
        body: &[Decl],
        also: Option<&str>,
    ) {
        let arity = header.generics.iter().filter(|g| !g.lifetime).count();
        let struct_ = self.index.kind(&(name.to_string(), arity)) == Some(Kind::Struct);
        // Its name in Java: the arity's too, when its name has several
        // (`ValueTuple2` for `ValueTuple<T1, T2>`).
        let ys = name;
        let java = self.simple_name(name, header);
        let name = java.as_str();
        let (extends, interfaces) = self.bases(header);
        let mut implements = interfaces;
        implements.extend(also.map(str::to_string));
        if struct_ {
            implements.push(CLONEABLE.into());
        }
        // A generic one keeps the type arguments it's made with (`GetType()`).
        let generic = header.generics.iter().any(|g| !g.lifetime);
        if generic {
            implements.push(TYPED.into());
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
        let stat = if self.nested { "static " } else { "" };
        self.code.open(format!(
            "public {stat}{lead}class {name}{}{extends}{implements}",
            generic_list(&header.generics)
        ));
        self.describe(d);
        self.constants(body);
        for f in fields(body) {
            self.code.line(format!(
                "public {} {};",
                self.written(f.0, false),
                Self::ident(f.1)
            ));
        }
        if generic {
            self.typed_members();
        }
        self.constructors(name, header, body);
        self.members(body, Place::Class);
        self.inherited_getters(&(ys.to_string(), arity), body);
        let stored: Vec<&str> = fields(body).into_iter().map(|(_, f)| f).collect();
        // `instanceof` takes a generic one with its arguments unknown: `Box<?>`.
        let arity = header.generics.iter().filter(|g| !g.lifetime).count();
        let tested = if arity == 0 {
            name.to_string()
        } else {
            format!("{name}<{}>", vec![WILDCARD; arity].join(", "))
        };
        self.identity(&tested, body, struct_.then_some(stored.as_slice()));
        if struct_ {
            // Of its own type, arguments and all (a generic one's cast is safe: a clone).
            let own = format!("{name}{}", generic_list(&header.generics));
            if !header.generics.is_empty() {
                self.code.line(UNCHECKED);
            }
            self.code.open(format!("public {own} {COPY}()"));
            self.code.line(format!("try {{ return ({own}) clone(); }} catch (CloneNotSupportedException e) {{ throw new AssertionError(e); }}"));
            self.code.close("");
        }
        self.code.close("");
    }

    /// What keeps a generic object's type arguments (`Typed`).
    fn typed_members(&mut self) {
        self.code.line(format!("private {TYPE}[] {TYPES_FIELD};"));
        self.code.line(format!(
            "@Override public {TYPE}[] {TYPES_FIELD}() {{ return {TYPES_FIELD}; }}"
        ));
        self.code.line(format!(
            "@Override public void {TYPES_FIELD}({TYPE}[] arguments) {{ this.{TYPES_FIELD} = arguments; }}"
        ));
    }

    /// Type `name`'s own name in Java (`header`'s arity told apart, as
    /// `Index::place` does): its place's last name.
    pub(crate) fn simple_name(&self, name: &str, header: &TypeHeader) -> String {
        let arity = header.generics.iter().filter(|g| !g.lifetime).count();
        let place = self.index.place(name, arity);
        place.rsplit('.').next().unwrap_or(&place).to_string()
    }

    /// `const` and `static` fields, set where they're declared: a constant
    /// or a `static readonly` one Java's `static final`.
    fn constants(&mut self, body: &[Decl]) {
        for d in body {
            if let DeclKind::Field(f) = &d.kind
                && (d.has(Modifier::Const) || d.has(Modifier::Static))
                && let Some(v) = &f.value
            {
                let fixed = d.has(Modifier::Const) || d.has(Modifier::Readonly);
                let (t, n, v) = (
                    self.written(&f.ty, false),
                    Self::ident(&f.name.text),
                    self.expr(v),
                );
                let fin = if fixed { "final " } else { "" };
                self.code.line(format!("public static {fin}{t} {n} = {v};"));
            }
        }
    }

    /// The class's base (`extends`) and interfaces (`implements`).
    fn bases(&self, header: &TypeHeader) -> (Option<String>, Vec<String>) {
        let mut extends = None;
        let mut interfaces = Vec::new();
        for b in &header.bases {
            let is_interface = match &b.kind {
                yog_script_analyze::ast::TypeKind::Path { path, args } => {
                    path.last().is_some_and(|n| {
                        self.index.kind(&(n.text.clone(), args.len())) == Some(Kind::Interface)
                    })
                }
                _ => false,
            };
            if is_interface || extends.is_some() {
                interfaces.push(self.written(b, true));
            } else {
                extends = Some(self.written(b, true));
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
                DeclKind::Field(_) if d.has(Modifier::Const) || d.has(Modifier::Static) => continue,
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

    /// A stored property an interface it implements reads by a getter
    /// (`ICollection<T>.Count`): that getter, beside its field.
    fn inherited_getters(&mut self, name: &crate::java_names::TypeRef, body: &[Decl]) {
        for (ty, field) in fields(body) {
            if self.index.inherited_call(name, field) {
                let (t, f) = (self.written(ty, false), Self::ident(field));
                self.code.open(format!("public {t} {f}()"));
                self.code.line(format!("return this.{f};"));
                self.code.close("");
            }
        }
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
            .map_or_else(|| "Void".to_string(), |t| self.written(t, true));
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
                .is_some_and(|n| n.text == yog_script_ids::system::SERVICE)
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

/// A class's stored fields: its fields and auto-properties.
fn fields(body: &[Decl]) -> Vec<(&yog_script_analyze::ast::Type, &str)> {
    body.iter()
        .filter_map(|d| match &d.kind {
            DeclKind::Field(f) if !d.has(Modifier::Const) && !d.has(Modifier::Static) => {
                Some((&f.ty, f.name.text.as_str()))
            }
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
