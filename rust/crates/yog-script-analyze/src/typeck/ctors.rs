//! `new T(args)`: the arguments checked against `T`'s constructors, as a
//! call's against its overloads (a lambda typed by the delegate a
//! constructor takes: `new Lazy<i32>(() => 42)`).

use super::Checker;
use super::members::Signature;
use super::model::Type;
use crate::ast::{Arg, Expr, ExprKind, Param};

impl Checker<'_, '_, '_> {
    /// `new t(args)` (expression `e`): the constructor chosen noted.
    pub(super) fn constructed(&mut self, e: &Expr, t: &Type, args: &[Arg]) {
        let at = e.span;
        let sigs = match t {
            Type::Own(name, targs) => self.constructors(name, targs),
            _ => Vec::new(),
        };
        let given: Vec<Type> = args
            .iter()
            .map(|a| match a.value.kind {
                ExprKind::Lambda { .. } if !sigs.is_empty() => self.quietly(|c| c.expr(&a.value)),
                _ => self.expr(&a.value),
            })
            .collect();
        if !sigs.is_empty() {
            let args: Vec<&Arg> = args.iter().collect();
            if let (_, Some(chosen)) = self.pick(&sigs, &given, &args, at) {
                self.types.set_constructor(e, chosen.index);
            }
        }
    }

    /// Own type `name<targs>`'s constructors: those its body writes, and
    /// its primary one (unless only those are, it taking nothing). None for
    /// a struct (one makes any `new S()`) or a type not known.
    fn constructors(&mut self, name: &str, targs: &[Type]) -> Vec<Signature> {
        let Some(own) = self.own.get(name, targs.len()) else {
            return Vec::new();
        };
        if !own.heap {
            return Vec::new();
        }
        let mut all = own.ctors.clone();
        if all.is_empty() || !own.ctor.is_empty() {
            all.push(own.ctor.clone());
        }
        let context = self.context_of(name, targs);
        all.iter()
            .map(|params| self.in_context(&context, |c| c.ctor_signature(params)))
            .collect()
    }

    fn ctor_signature(&mut self, params: &[Param]) -> Signature {
        Signature {
            params: params.iter().map(|p| self.type_of(&p.ty)).collect(),
            passes: params.iter().map(|p| p.pass).collect(),
            required: params.iter().take_while(|p| p.default.is_none()).count(),
            ret: Type::Void,
            generics: Vec::new(),
            bound: Default::default(),
        }
    }
}
