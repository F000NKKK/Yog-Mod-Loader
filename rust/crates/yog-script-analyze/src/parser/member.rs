//! Members written type first (`ys-grammar.md` §2.2): methods, fields,
//! properties, indexers; constructors; content of any kind (`block Charger { … }`,
//! a mod's own kinds: the kinds are open); `Name = value;` properties.

use super::{PResult, Parser};
use crate::ast::{
    Accessor, Chain, Ctor, DeclKind, Field, Indexer, Method, MethodBody, Name, Operator,
    OperatorKind, Prop, Type,
};
use crate::source::token::{Punct, Tok};
use crate::words::{
    explicit, get, has, implicit, r#in, init, operator, requires, set, this, void, was,
};

impl Parser<'_, '_, '_> {
    /// A member no word starts.
    pub(super) fn member(&mut self) -> PResult<DeclKind> {
        if self.at_word(&void::WORD) {
            self.bump();
            let name = self.name("a method's name")?;
            return self.method_rest(None, name).map(DeclKind::Method);
        }
        if matches!(self.tok(), Tok::Ident(_)) && self.nth_is(1, Punct::LParen) {
            return self.ctor().map(DeclKind::Ctor);
        }
        if self.at_word(&implicit::WORD) || self.at_word(&explicit::WORD) {
            return self.conversion();
        }
        if self.operator_ahead() {
            return self.operator();
        }
        if self.indexer_ahead() {
            let ty = self.ty()?;
            return self.indexer(ty);
        }
        let Some(len) = self.typed_ahead() else {
            return Err(self.expected("a declaration"));
        };
        let content = len == 1
            && (self.nth_word(2, &has::WORD)
                || self.nth_is(2, Punct::Colon)
                || (self.nth_is(2, Punct::LBrace) && !self.accessor_ahead(3)));
        if content {
            return self.content();
        }
        let ty = self.ty()?;
        let name = self.name("a name")?;
        match self.tok() {
            Tok::Punct(Punct::LParen | Punct::Lt) => {
                self.method_rest(Some(ty), name).map(DeclKind::Method)
            }
            Tok::Punct(Punct::LBrace) => self.property(ty, name),
            Tok::Punct(Punct::FatArrow) => {
                self.bump();
                let value = self.expr()?;
                self.end()?;
                let get = Accessor {
                    name: Name {
                        text: get::WORD.text.to_string(),
                        span: value.span,
                    },
                    body: Some(MethodBody::Expr(value)),
                };
                Ok(DeclKind::Property {
                    ty,
                    name,
                    accessors: vec![get],
                    value: None,
                })
            }
            _ => self.field_rest(ty, name).map(DeclKind::Field),
        }
    }

    /// Whether an operator starts here: a type, then `operator`.
    fn operator_ahead(&self) -> bool {
        self.type_len(0)
            .is_some_and(|len| self.nth_word(len, &operator::WORD))
    }

    /// `R operator +(A a, B b) …`: which operator its sign and its
    /// parameters' count say (`-` of one is negation).
    fn operator(&mut self) -> PResult<DeclKind> {
        let ret = self.ty()?;
        self.need_word(&operator::WORD)?;
        let Tok::Punct(p) = self.tok().clone() else {
            return Err(self.expected("an operator's sign"));
        };
        let name = Name {
            text: String::new(),
            span: self.bump().span,
        };
        let mut method = self.method_rest(Some(ret), name)?;
        let kind = match method.params.len() {
            1 => super::expr::prefix_of(p).map(OperatorKind::Unary),
            _ => super::expr::binary_of(p).map(OperatorKind::Binary),
        };
        let Some((kind, named)) = kind.and_then(|k| Some((k, k.method_name()?))) else {
            return Err(self.expected(
                "an operator a type may declare (`+ - * / % == != < > <= >=`, unary `-` `!`)",
            ));
        };
        method.name.text = named.to_string();
        Ok(DeclKind::Operator(Operator { kind, method }))
    }

    /// `implicit operator R(A a) …` / `explicit operator R(A a) …`.
    fn conversion(&mut self) -> PResult<DeclKind> {
        let kind = if self.eat_word(&implicit::WORD) {
            OperatorKind::Implicit
        } else {
            self.need_word(&explicit::WORD)?;
            OperatorKind::Explicit
        };
        self.need_word(&operator::WORD)?;
        let ret = self.ty()?;
        let name = Name {
            text: kind.method_name().unwrap_or_default().to_string(),
            span: ret.span,
        };
        let method = self.method_rest(Some(ret), name)?;
        Ok(DeclKind::Operator(Operator { kind, method }))
    }

    /// Whether an indexer starts here: a type, then `this[`.
    fn indexer_ahead(&self) -> bool {
        self.type_len(0).is_some_and(|len| {
            self.nth_word(len, &this::WORD) && self.nth_is(len + 1, Punct::LBracket)
        })
    }

    /// Whether `get`, `set` or `init` stands `n` ahead: a property's braces.
    fn accessor_ahead(&self, n: usize) -> bool {
        [&get::WORD, &set::WORD, &init::WORD]
            .iter()
            .any(|w| self.nth_word(n, w))
    }

    /// After a method's result and name: generics, parameters, constraints,
    /// guard, body (none for a signature: `;`).
    pub(crate) fn method_rest(&mut self, ret: Option<Type>, name: Name) -> PResult<Method> {
        let generics = self.generics()?;
        let params = self.params()?;
        let constraints = self.constraints()?;
        let requires = if self.eat_word(&requires::WORD) {
            Some(self.no_brace(Self::expr)?)
        } else {
            None
        };
        let body = self.method_body()?;
        Ok(Method {
            ret,
            name,
            generics,
            params,
            constraints,
            requires,
            body,
        })
    }

    /// `{ … }`, `=> expr;`, or `;` (none).
    pub(crate) fn method_body(&mut self) -> PResult<Option<MethodBody>> {
        if self.eat(Punct::FatArrow) {
            let e = self.expr()?;
            self.end()?;
            return Ok(Some(MethodBody::Expr(e)));
        }
        if self.at(Punct::LBrace) {
            return Ok(Some(MethodBody::Block(self.block()?)));
        }
        self.end()?;
        Ok(None)
    }

    /// After a field's type and name: `in range`, `= value`, `was Old: T`, `;`.
    pub(crate) fn field_rest(&mut self, ty: Type, name: Name) -> PResult<Field> {
        let bound = if self.eat_word(&r#in::WORD) {
            Some(self.range_bound()?)
        } else {
            None
        };
        let value = if self.eat(Punct::Eq) {
            Some(self.expr()?)
        } else {
            None
        };
        let was = if self.eat_word(&was::WORD) {
            let old = self.name("the old name")?;
            let ty = if self.eat(Punct::Colon) {
                Some(self.ty()?)
            } else {
                None
            };
            Some((old, ty))
        } else {
            None
        };
        self.end()?;
        Ok(Field {
            ty,
            name,
            bound,
            value,
            was,
        })
    }

    /// `{ get; set { … } }`: a property's or an indexer's accessors.
    fn accessors(&mut self) -> PResult<Vec<Accessor>> {
        self.need(Punct::LBrace)?;
        let mut accessors = Vec::new();
        while !self.eat(Punct::RBrace) {
            if !self.accessor_ahead(0) {
                return Err(self.expected("`get`, `set` or `init`"));
            }
            let name = self.any_name("an accessor")?;
            let body = self.method_body()?;
            accessors.push(Accessor { name, body });
        }
        Ok(accessors)
    }

    /// `this[Type name, …] { get; set; }`, after the indexer's type.
    fn indexer(&mut self, ty: Type) -> PResult<DeclKind> {
        self.need_word(&this::WORD)?;
        let start = ty.span;
        let params = self.index_params()?;
        let accessors = self.accessors()?;
        Ok(DeclKind::Indexer(Indexer {
            span: start.to(self.prev()),
            ty,
            params,
            accessors,
        }))
    }

    /// `{ get; set { … } }` and maybe `= value;`.
    fn property(&mut self, ty: Type, name: Name) -> PResult<DeclKind> {
        let accessors = self.accessors()?;
        let value = if self.eat(Punct::Eq) {
            let v = self.expr()?;
            self.end()?;
            Some(v)
        } else {
            None
        };
        Ok(DeclKind::Property {
            ty,
            name,
            accessors,
            value,
        })
    }

    /// `Name(params) : this(args) { … }`.
    fn ctor(&mut self) -> PResult<Ctor> {
        let name = self.name("a constructor")?;
        let params = self.params()?;
        let chain = if self.eat(Punct::Colon) {
            let chain = if self.eat_word(&crate::words::base::WORD) {
                Chain::Base
            } else {
                self.need_word(&crate::words::this::WORD)?;
                Chain::This
            };
            Some((chain, self.args()?))
        } else {
            None
        };
        let body = self.block()?;
        Ok(Ctor {
            name,
            params,
            chain,
            body,
        })
    }

    /// `Name = value;`.
    pub(crate) fn prop(&mut self) -> PResult<Prop> {
        let name = self.name("a property")?;
        self.need(Punct::Eq)?;
        let value = self.expr()?;
        self.end()?;
        Ok(Prop { name, value })
    }

    /// `kind Name (: Base)? (has …)? { body }`.
    fn content(&mut self) -> PResult<DeclKind> {
        let kind = self.name("a content kind")?;
        let name = self.name("a name")?;
        let header = self.header(false)?;
        let body = self.body()?;
        Ok(DeclKind::Content {
            kind,
            name,
            header,
            body,
        })
    }

    /// What a declaration's `in` bounds it by: a range.
    fn range_bound(&mut self) -> PResult<crate::ast::Expr> {
        self.binary(crate::words::Level::Range)
    }
}
