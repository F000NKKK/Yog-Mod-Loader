//! Classes (and structs, and the mod) with their constructors — the
//! fields' first values set in each, after the base's — enums, a method's
//! body, and an `extern` method's call on the bus.

use yog_script_analyze::ast::{
    Chain, Ctor, Decl, DeclKind, Method, MethodBody, Modifier, Param, TypeHeader,
};

use super::Gen;
use crate::decls::{Place, generic_list};
use crate::index::Kind;

use crate::modifier;
use yog_script_ids::java::{
    ABSTRACT, CATCH, CLASS, CLASS_LITERAL, EXTENDS, FINAL, IMPLEMENTS, NEW, OVERRIDE, PRIVATE,
    PUBLIC, RECORD, RETURN, STATIC, SUPER, THIS, THROW, TRY, VOID,
};
use yog_script_ids::java::{UNCHECKED, WILDCARD};
use yog_script_ids::jvm::reflection::{TYPE, TYPED, TYPES_FIELD};
use yog_script_ids::jvm::{BUS_CALL, CLONEABLE, COPY, REQUEST};

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
        // A generic one keeps the type arguments it's made with (`GetType()`).
        let generic = header.generics.iter().any(|g| !g.lifetime);
        let head = self.class_head(d, (name, header), (struct_, generic), also);
        self.code.open(head);
        self.describe(d);
        self.constants(body);
        self.statics_holder(&(ys.to_string(), arity), header, body);
        // Its primary constructor's parameters, reached by its members (C# 12's).
        for p in captured(header, body) {
            self.code.line(format!(
                "{PRIVATE} {FINAL} {} {};",
                self.written(&p.ty, false),
                Self::ident(&p.name.text)
            ));
        }
        self.backing_fields(body);
        for f in fields(body) {
            self.code.line(format!(
                "{PUBLIC} {} {};",
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
            self.code.open(format!("{PUBLIC} {own} {COPY}()"));
            self.code.line(format!("{TRY} {{ {RETURN} ({own}) clone(); }} {CATCH} (CloneNotSupportedException e) {{ {THROW} {NEW} AssertionError(e); }}"));
            self.code.close("");
        }
        self.code.close("");
    }

    /// `public static final class Name<T> extends Base implements …`.
    fn class_head(
        &self,
        d: &Decl,
        (name, header): (&str, &TypeHeader),
        (struct_, generic): (bool, bool),
        also: Option<&str>,
    ) -> String {
        let (extends, interfaces) = self.bases(header);
        let mut implements = interfaces;
        implements.extend(also.map(str::to_string));
        if struct_ {
            implements.push(CLONEABLE.into());
        }
        if generic {
            implements.push(TYPED.into());
        }
        let implements = if implements.is_empty() {
            String::new()
        } else {
            format!(" {IMPLEMENTS} {}", implements.join(", "))
        };
        let extends = extends
            .map(|b| format!(" {EXTENDS} {b}"))
            .unwrap_or_default();
        let lead = if d.has(Modifier::Abstract) {
            modifier(ABSTRACT)
        } else if d.has(Modifier::Sealed) || struct_ {
            modifier(FINAL)
        } else {
            String::new()
        };
        let stat = if self.nested || self.inner_type {
            modifier(STATIC)
        } else {
            String::new()
        };
        format!(
            "{PUBLIC} {stat}{lead}{CLASS} {name}{}{extends}{implements}",
            generic_list(&header.generics)
        )
    }

    /// What keeps a generic object's type arguments (`Typed`).
    fn typed_members(&mut self) {
        self.code.line(format!("{PRIVATE} {TYPE}[] {TYPES_FIELD};"));
        self.code.line(format!(
            "{OVERRIDE} {PUBLIC} {TYPE}[] {TYPES_FIELD}() {{ {RETURN} {TYPES_FIELD}; }}"
        ));
        self.code.line(format!(
            "{OVERRIDE} {PUBLIC} {VOID} {TYPES_FIELD}({TYPE}[] arguments) {{ {THIS}.{TYPES_FIELD} = arguments; }}"
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
    pub(crate) fn constants(&mut self, body: &[Decl]) {
        let per_type = self
            .owner
            .as_ref()
            .is_some_and(|o| self.index.takes_types(o));
        for d in body {
            if let DeclKind::Field(f) = &d.kind
                && (d.has(Modifier::Const) || d.has(Modifier::Static))
                && !(per_type && !d.has(Modifier::Const))
            {
                let fixed = d.has(Modifier::Const) || d.has(Modifier::Readonly);
                let (t, n) = (self.written(&f.ty, false), Self::ident(&f.name.text));
                let v = match &f.value {
                    Some(v) => format!(" = {}", self.expr(v)),
                    None => String::new(),
                };
                let fin = if fixed {
                    modifier(FINAL)
                } else {
                    String::new()
                };
                self.code
                    .line(format!("{PUBLIC} {STATIC} {fin}{t} {n}{v};"));
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
    /// parameters) and each written one. An own generic type's take its
    /// type arguments first (`Type[] $types`): kept on it at once, its `T`
    /// is known in them as anywhere (C#'s).
    fn constructors(&mut self, name: &str, header: &TypeHeader, body: &[Decl]) {
        let ctors: Vec<&Ctor> = body
            .iter()
            .filter_map(|d| match &d.kind {
                DeclKind::Ctor(c) => Some(c),
                _ => None,
            })
            .collect();
        let taking = self.made_with_types(header);
        // `T` in them: the arguments handed.
        let mapping: Vec<(String, String)> = header
            .generics
            .iter()
            .filter(|g| !g.lifetime)
            .enumerate()
            .filter(|_| taking)
            .map(|(i, g)| (g.name.text.clone(), format!("{TYPES_FIELD}[{i}]")))
            .collect();
        self.with_type_params(mapping, |g| {
            let primary = header.ctor.as_deref().unwrap_or_default();
            if ctors.is_empty() || !primary.is_empty() {
                let params = g.ctor_params(primary, taking);
                g.code.open(format!("{PUBLIC} {name}({params})"));
                let base = g.base_types(header);
                g.code.line(format!("{SUPER}({base});"));
                g.keep_types(taking);
                for p in captured(header, body) {
                    let n = Self::ident(&p.name.text);
                    g.code.line(format!("{THIS}.{n} = {n};"));
                }
                g.first_values(body);
                g.code.close("");
            }
            for c in ctors {
                g.constructor(name, (header, body), c, taking);
            }
        });
    }

    /// Written constructor `c` of a type (`taking` its type arguments).
    fn constructor(
        &mut self,
        name: &str,
        (header, body): (&TypeHeader, &[Decl]),
        c: &Ctor,
        taking: bool,
    ) {
        let params = self.ctor_params(&c.params, taking);
        self.code.open(format!("{PUBLIC} {name}({params})"));
        match &c.chain {
            Some((Chain::This, args)) => {
                let args = self.args(args);
                let args = if taking {
                    joined(TYPES_FIELD, &args)
                } else {
                    args
                };
                self.code.line(format!("{THIS}({args});"));
            }
            other => {
                let args = match other {
                    Some((_, args)) => self.args(args),
                    None => String::new(),
                };
                let base = self.base_types(header);
                let args = if base.is_empty() {
                    args
                } else {
                    joined(&base, &args)
                };
                self.code.line(format!("{SUPER}({args});"));
                self.keep_types(taking);
                self.first_values(body);
            }
        }
        let outer = std::mem::replace(&mut self.cells, crate::refs::cells(&c.params));
        self.with_boxed(&c.body.stmts, |g| g.block_stmts(&c.body));
        self.cells = outer;
        self.code.close("");
    }

    /// Whether the type of `header` (the one being written) is an own
    /// generic one: made with its type arguments handed.
    fn made_with_types(&self, header: &TypeHeader) -> bool {
        self.owner
            .as_ref()
            .is_some_and(|key| self.index.takes_types(key))
            && header.generics.iter().any(|g| !g.lifetime)
    }

    /// A constructor's parameters, its type arguments' first when `taking`.
    fn ctor_params(&mut self, params: &[Param], taking: bool) -> String {
        let params = self.params(params);
        if taking {
            joined(&format!("{TYPE}[] {TYPES_FIELD}"), &params)
        } else {
            params
        }
    }

    /// The type arguments handed on to the base's constructor, when it's an
    /// own generic one (its own written in terms of this one's `T`).
    fn base_types(&mut self, header: &TypeHeader) -> String {
        let Some(base) = header.bases.first() else {
            return String::new();
        };
        let yog_script_analyze::ast::TypeKind::Path { path, args } = &base.kind else {
            return String::new();
        };
        let Some(last) = path.last() else {
            return String::new();
        };
        if !self.index.takes_types(&(last.text.clone(), args.len())) {
            return String::new();
        }
        let values: Vec<String> = args
            .iter()
            .map(|a| {
                self.type_value(a, &|_| None)
                    .unwrap_or_else(|| yog_script_ids::java::NULL.into())
            })
            .collect();
        format!("{NEW} {TYPE}[] {{ {} }}", values.join(", "))
    }

    /// `this.$types = $types;`: the arguments kept as soon as the base is made.
    fn keep_types(&mut self, taking: bool) {
        if taking {
            self.code
                .line(format!("{THIS}.{TYPES_FIELD} = {TYPES_FIELD};"));
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
                // One keeping its own value: that set first.
                DeclKind::Property {
                    name,
                    value,
                    accessors,
                    ..
                } if crate::backing::keeps_value(accessors) => {
                    if let Some(v) = value {
                        let v = self.handed_expr(v);
                        let field = crate::backing::backing_name(&name.text);
                        self.code.line(format!("{THIS}.{field} = {v};"));
                    }
                    continue;
                }
                _ => continue,
            };
            if let Some(v) = value {
                let v = self.handed_expr(v);
                self.code
                    .line(format!("{THIS}.{} = {v};", Self::ident(name)));
            }
        }
    }

    /// A stored property an interface it implements reads by a getter
    /// (`ICollection<T>.Count`): that getter, beside its field.
    fn inherited_getters(&mut self, name: &crate::java_names::TypeRef, body: &[Decl]) {
        for (ty, field) in fields(body) {
            if self.index.inherited_call(name, field) {
                let (t, f) = (self.written(ty, false), Self::ident(field));
                self.code.open(format!("{PUBLIC} {t} {f}()"));
                self.code.line(format!("{RETURN} {THIS}.{f};"));
                self.code.close("");
            }
        }
    }

    /// A method's body after `head`: a block, or an expression returned.
    pub(crate) fn body(&mut self, head: &str, body: Option<&MethodBody>, void: bool) {
        self.code.open(head);
        match body {
            Some(MethodBody::Block(b)) => self.with_boxed(&b.stmts, |g| g.block_stmts(b)),
            Some(MethodBody::Expr(e)) => {
                let v = self.handed_expr(e);
                if void {
                    self.code.line(format!("{v};"))
                } else {
                    self.code.line(format!("{RETURN} {v};"))
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
            .line(format!("{PUBLIC} {RECORD} {request}({fields}) {{}}"));
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
            "{BUS_CALL}({address}, {NEW} {request}({}), {reply}{CLASS_LITERAL})",
            args.join(", ")
        );
        if m.ret.is_some() {
            self.code.line(format!("{RETURN} {call};"))
        } else {
            self.code.line(format!("{call};"))
        };
        self.code.close("");
    }

    /// The address a `[Service("…")]` gives, as a Java string.
    fn service_address(&self, d: &Decl) -> Option<String> {
        crate::index::attribute_text(d, yog_script_ids::system::SERVICE).map(|t| self.code.str(&t))
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

/// `first` and then `rest` (a list of arguments or parameters), as Java writes them.
fn joined(first: &str, rest: &str) -> String {
    if rest.is_empty() {
        first.to_string()
    } else {
        format!("{first}, {rest}")
    }
}

/// The primary constructor's parameters its type keeps for its members: all
/// but those a field or property of the name takes over.
fn captured<'h>(header: &'h TypeHeader, body: &[Decl]) -> Vec<&'h Param> {
    let stored: Vec<&str> = fields(body).into_iter().map(|(_, n)| n).collect();
    header
        .ctor
        .iter()
        .flatten()
        .filter(|p| !stored.contains(&p.name.text.as_str()))
        .collect()
}
