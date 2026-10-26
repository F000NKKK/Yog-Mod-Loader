//! An indexer (`T this[i32 i] { get; set; }`): what `x[i]` reads and
//! writes. Its accessors are methods named as the CLR names them —
//! `get_Item(i)`, `set_Item(i, value)` — so inheritance and the JVM treat
//! them as any method.

use super::{Accessor, Method, MethodBody, Name, Param, Type, value_param};
use crate::source::token::Span;
use crate::words::{get, set};
use yog_script_ids::clr::INDEXER;

/// What joins an accessor's word to the indexer's name (`get_Item`).
use yog_script_ids::clr::ACCESSOR_SEP;

#[derive(Debug, Clone, PartialEq)]
pub struct Indexer {
    pub ty: Type,
    pub params: Vec<Param>,
    pub accessors: Vec<Accessor>,
    pub span: Span,
    /// Its `get` and its `set` as methods, made once: what's checked and
    /// compiled is these (one tree, so what's found of it is found once).
    get: Option<Box<Method>>,
    set: Option<Box<Method>>,
}

/// An indexer's accessor's method name: `get_Item`, `set_Item`.
pub fn accessor_name(word: &str) -> String {
    format!("{word}{ACCESSOR_SEP}{INDEXER}")
}

impl Indexer {
    /// Indexer `ty this[params] { accessors }` (at `span`), its accessors
    /// made methods.
    pub fn new(ty: Type, params: Vec<Param>, accessors: Vec<Accessor>, span: Span) -> Self {
        let mut ix = Indexer {
            ty,
            params,
            accessors,
            span,
            get: None,
            set: None,
        };
        ix.get = ix.accessor(get::WORD.text).map(|body| {
            Box::new(ix.method(get::WORD.text, Some(ix.ty.clone()), ix.params.clone(), body))
        });
        ix.set = ix.accessor(set::WORD.text).map(|body| {
            let mut params = ix.params.clone();
            let at = Name {
                text: set::WORD.text.to_string(),
                span: ix.span,
            };
            params.push(value_param(&ix.ty, &at));
            Box::new(ix.method(set::WORD.text, None, params, body))
        });
        ix
    }

    /// Its `get` as a method: what `x[i]` calls.
    pub fn getter(&self) -> Option<&Method> {
        self.get.as_deref()
    }

    /// Its `set` as a method, the value its last parameter: what `x[i] = v` calls.
    pub fn setter(&self) -> Option<&Method> {
        self.set.as_deref()
    }

    /// Accessor `word`'s body (none when it has none), if it has that accessor.
    fn accessor(&self, word: &str) -> Option<Option<MethodBody>> {
        self.accessors
            .iter()
            .find(|a| a.name.text == word)
            .map(|a| a.body.clone())
    }

    fn method(
        &self,
        word: &str,
        ret: Option<Type>,
        params: Vec<Param>,
        body: Option<MethodBody>,
    ) -> Method {
        Method {
            ret,
            name: Name {
                text: accessor_name(word),
                span: self.span,
            },
            explicit: None,
            generics: Vec::new(),
            params,
            constraints: Vec::new(),
            requires: None,
            body,
        }
    }
}
