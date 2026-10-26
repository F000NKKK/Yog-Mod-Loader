//! The constructors a type's body writes, run as C# runs them: the one
//! taking the arguments, after what it chains to (`: base(…)`,
//! `: this(…)`) or its base's taking nothing, `this` the object made.

use yog_script_analyze::ast::{Chain, Ctor};
use yog_script_analyze::token::Span;

use super::{Interp, Run};
use crate::value::Value;

impl<'p> Interp<'p> {
    /// Type `of`'s constructor taking `args` run on `object` (none written: nothing).
    pub(crate) fn construct(
        &mut self,
        of: &'p str,
        object: &Value,
        args: Vec<Value>,
        at: Span,
    ) -> Run<()> {
        let Some(def) = self.program.types.get(of) else {
            return Ok(());
        };
        let Some(ctor) = def
            .ctors
            .iter()
            .copied()
            .find(|c| c.params.len() == args.len())
        else {
            // None written: its base's taking nothing still runs.
            return match def.base {
                Some(base) if def.ctors.is_empty() => self.construct(base, object, Vec::new(), at),
                _ => Ok(()),
            };
        };
        self.run_ctor(of, ctor, object, args, at)
    }

    fn run_ctor(
        &mut self,
        of: &'p str,
        ctor: &'p Ctor,
        object: &Value,
        args: Vec<Value>,
        at: Span,
    ) -> Run<()> {
        let outer = (
            std::mem::replace(&mut self.scopes, vec![std::collections::HashMap::new()]),
            self.this.replace(object.clone()),
            self.class.replace(of),
        );
        let ran = self.ctor_body(of, ctor, object, args, at);
        (self.scopes, self.this, self.class) = outer;
        ran
    }

    fn ctor_body(
        &mut self,
        of: &'p str,
        ctor: &'p Ctor,
        object: &Value,
        args: Vec<Value>,
        at: Span,
    ) -> Run<()> {
        for (p, v) in ctor.params.iter().zip(args) {
            self.bind(&p.name.text, v);
        }
        let base = self.program.types.get(of).and_then(|d| d.base);
        match &ctor.chain {
            Some((chain, given)) => {
                let given = given
                    .iter()
                    .map(|a| self.eval(&a.value).map(|v| v.copied()))
                    .collect::<Run<Vec<_>>>()?;
                let first = match chain {
                    Chain::This => Some(of),
                    Chain::Base => base,
                };
                if let Some(first) = first {
                    self.construct(first, object, given, at)?;
                }
            }
            None => {
                if let Some(base) = base {
                    self.construct(base, object, Vec::new(), at)?;
                }
            }
        }
        match self.block(&ctor.body) {
            Err(super::Stop::Return(_)) | Ok(()) => Ok(()),
            Err(e) => Err(e),
        }
    }
}
