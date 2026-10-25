//! `delegate` — a type of methods, as in C#.

use crate::ast::DeclKind;
use crate::parser::{PResult, Parser};
use crate::words::{Role, Word, void};

pub static WORD: Word = Word {
    text: "delegate",
    doc: "A type of methods, as in C#: `public delegate bool Predicate<in T>(T obj);`. A lambda or a \
          method of its shape is one (`Predicate<i32> even = n => n % 2 == 0;`), called as a method \
          (`even(4)`, `even.Invoke(4)`); an `event` is of a delegate type. `System` has `Action<…>`, \
          `Func<…, TResult>`, `Predicate<T>`, `Comparison<T>`, `Converter<TIn, TOut>`, `EventHandler<T>`.",
    reserved: true,
    roles: &[Role::Decl { starts: None, parse }],
};

/// `delegate R Name<…>(params);`: a signature, never a body.
fn parse(p: &mut Parser) -> PResult<DeclKind> {
    p.bump();
    let ret = if p.at_word(&void::WORD) {
        p.bump();
        None
    } else {
        Some(p.ty()?)
    };
    let name = p.name("a delegate's name")?;
    let m = p.method_rest(ret, name)?;
    if m.body.is_some() {
        return Err(p.expected("`;`: a delegate is a signature, without a body"));
    }
    Ok(DeclKind::Delegate(m))
}
