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
}

/// An indexer's accessor's method name: `get_Item`, `set_Item`.
pub fn accessor_name(word: &str) -> String {
    format!("{word}{ACCESSOR_SEP}{INDEXER}")
}

impl Indexer {
    /// Its `get` as a method: what `x[i]` calls.
    pub fn getter(&self) -> Option<Method> {
        let body = self.accessor(get::WORD.text)?;
        Some(self.method(
            get::WORD.text,
            Some(self.ty.clone()),
            self.params.clone(),
            body,
        ))
    }

    /// Its `set` as a method, the value its last parameter: what `x[i] = v` calls.
    pub fn setter(&self) -> Option<Method> {
        let body = self.accessor(set::WORD.text)?;
        let mut params = self.params.clone();
        let at = Name {
            text: set::WORD.text.to_string(),
            span: self.span,
        };
        params.push(value_param(&self.ty, &at));
        Some(self.method(set::WORD.text, None, params, body))
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
