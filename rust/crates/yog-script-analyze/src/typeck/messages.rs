//! Calls across sides (`ys.md` §4.1, `ys-grammar.md` §3.6), each a
//! message the compiler writes: `player.Client.F(args)` runs the file's
//! `client` method `F` on that player's client; a `server` method called
//! from `client` code runs on the server, its `Sender` (taken first, not
//! given) who sent it. A message gives nothing back; a `client` method
//! called from elsewhere without a client is a side's error.

use yog_ids::script_problem;

use super::Checker;
use super::members::Callee;
use super::model::Type;
use super::types::Message;
use crate::ast::{Arg, Expr, ExprKind, Method, Name};
use crate::index::Side;

impl Checker<'_, '_, '_> {
    /// Call `e` of `callee` with `args` when it crosses sides: checked as
    /// the message it is, noted; `None` when it doesn't.
    pub(super) fn message_call(&mut self, e: &Expr, callee: &Expr, args: &[Arg]) -> Option<Type> {
        match &callee.kind {
            ExprKind::Member { expr, name, .. } => match &expr.kind {
                ExprKind::Member {
                    expr: target,
                    name: side,
                    ..
                } if side.text == yog_codegen::pascal(crate::words::client::WORD.text) => {
                    Some(self.client_call(e, target, name, args))
                }
                _ => None,
            },
            ExprKind::Ident(name) if self.side == Side::Client && self.local(name).is_none() => {
                (self.own.sides.get(name) == Some(&Side::Server))
                    .then(|| self.server_call(e, name, callee, args))
            }
            ExprKind::Ident(name) if self.side != Side::Client && self.local(name).is_none() => {
                if self.own.sides.get(name) == Some(&Side::Client) {
                    self.report.error(
                        script_problem::WRONG_SIDE,
                        callee.span,
                        format!("`{name}` runs on a client: call it on a player's (`player.Client.{name}(…)`)"),
                    );
                }
                None
            }
            _ => None,
        }
    }

    /// `target.Client.name(args)`: the file's `client` method on a player's client.
    fn client_call(&mut self, e: &Expr, target: &Expr, name: &Name, args: &[Arg]) -> Type {
        let t = self.expr(target);
        if !matches!(t.non_null(), Type::Yog(h) if h == crate::yog::player()) && t.known() {
            self.report.error(
                script_problem::TYPE_MISMATCH,
                target.span,
                format!(
                    "a client is a player's: `player.Client.{}(…)`, not a `{t}`'s",
                    name.text
                ),
            );
        }
        if self.own.sides.get(&name.text) != Some(&Side::Client) {
            self.report.error(
                script_problem::WRONG_SIDE,
                name.span,
                format!("`{}` isn't a `client` method of this file", name.text),
            );
        }
        self.message_args(e, (&name.text, name.span), args, false)
    }

    /// `name(args)` from `client` code: the file's `server` method run on
    /// the server, its `Sender` the client.
    fn server_call(&mut self, e: &Expr, name: &str, callee: &Expr, args: &[Arg]) -> Type {
        self.message_args(e, (name, callee.span), args, true)
    }

    /// Method `name`'s message's arguments checked (a `server` one's
    /// `Sender` not given); the call noted as a message.
    fn message_args(
        &mut self,
        e: &Expr,
        (name, at): (&str, crate::source::token::Span),
        args: &[Arg],
        to_server: bool,
    ) -> Type {
        let methods: Vec<(Method, super::context::Context)> = self
            .callable(name)
            .into_iter()
            .map(|(mut m, c)| {
                if to_server && self.takes_sender(&m) {
                    m.params.remove(0);
                }
                m.ret = None;
                (m, c)
            })
            .collect();
        let given: Vec<Type> = args.iter().map(|a| self.expr(&a.value)).collect();
        let args: Vec<&Arg> = args.iter().collect();
        self.called_with_own(Callee::Own(methods), (&given, &args), at);
        self.types.set_message(
            e,
            Message {
                method: name.to_string(),
                to_server,
            },
        );
        Type::Void
    }

    /// Whether `m` takes who sent it first (`Sender`).
    fn takes_sender(&mut self, m: &Method) -> bool {
        m.params.first().is_some_and(|p| {
            self.type_of(&p.ty) == Type::own(yog_script_ids::system::SENDER.to_string())
        })
    }
}
