//! Each expression's type, as the checker found it: what a compiler needs
//! to write the code (a local's declaration, a value boxed, a switch's
//! result) and an editor to show.

use std::collections::HashMap;

use super::model::Type;
use crate::ast::Expr;

/// The types of a checked file's expressions. An expression is known by
/// its node in the file's tree — which, once checked, isn't changed — so
/// two that share a place (a macro's output, all at its call) stay apart.
#[derive(Debug, Default)]
pub struct Types {
    types: HashMap<usize, Type>,
    /// The operators a type declares that expressions are (`a + b`, `-a`,
    /// `(Energy)n`), by expression.
    operators: HashMap<usize, OperatorUse>,
    /// The implicit conversions values go through where they're handed
    /// on, by expression.
    conversions: HashMap<usize, OperatorUse>,
    /// The types values are recast to where they go by variance only (the
    /// JVM's generics have none), by expression.
    recasts: HashMap<usize, Type>,
    /// The lambdas given where an expression tree is wanted
    /// (`Expression<Func<i32, bool>>`), by the lambda.
    quoted: HashMap<usize, Quoted>,
    /// The type of each name a pattern binds (`let r`, `Monster m`), by the name.
    bindings: HashMap<usize, Type>,
    /// The constructor each `new T(…)` chose: which of the type's, its body's
    /// in order and then its primary one, by the `new`.
    constructors: HashMap<usize, usize>,
    /// The method each call chose, by the call.
    methods: HashMap<usize, MethodUse>,
    /// The operators `op=` applies (`v += w` on a `Vec`), by the place it
    /// writes.
    assigned: HashMap<usize, OperatorUse>,
}

/// A lambda given as an expression tree: its delegate's type, and the
/// parameters and result the delegate gives it.
#[derive(Debug, Clone, PartialEq)]
pub struct Quoted {
    pub delegate: Type,
    pub params: Vec<Type>,
    pub ret: Type,
}

/// An operator or a conversion a type declares, as used: the type, its
/// method (`op_Addition`, `op_Implicit`).
#[derive(Debug, Clone, PartialEq)]
pub struct OperatorUse {
    pub owner: Type,
    pub method: String,
}

/// A method as a call chose it: its type (none: the file's), its name,
/// which of the type's methods of that name (in declaration order), its
/// parameters and result as declared (its type parameters unbound).
#[derive(Debug, Clone, PartialEq)]
pub struct MethodUse {
    pub owner: Option<String>,
    /// Its type's arity (`ValueTuple<T1, T2>`'s 2): one of its name.
    pub owner_arity: usize,
    pub name: String,
    pub overload: usize,
    pub params: Vec<Type>,
    pub ret: Type,
    /// Its parameters as the call takes them (its type parameters bound).
    pub taken: Vec<Type>,
    /// Its own type parameters' types in this call, in order.
    pub type_args: Vec<Type>,
    /// An extension method (`xs.Where(…)`): the receiver its first argument.
    pub extension: bool,
}

impl Types {
    /// The type `e`'s value is recast to, going where it's wanted by
    /// variance (`IEnumerable<string>` as an `IEnumerable<object>`).
    /// Lambda `e` as an expression tree, when it's given as one.
    pub fn quoted(&self, e: &Expr) -> Option<&Quoted> {
        self.quoted.get(&key(e))
    }

    pub(super) fn set_quoted(&mut self, e: &Expr, quoted: Quoted) {
        self.quoted.insert(key(e), quoted);
    }

    pub fn recast(&self, e: &Expr) -> Option<&Type> {
        self.recasts.get(&key(e))
    }

    pub(super) fn set_recast(&mut self, e: &Expr, to: Type) {
        self.recasts.insert(key(e), to);
    }

    /// The method call `e` chose, when it's an own one.
    /// The type of name `n` a pattern binds.
    pub fn binding(&self, n: &crate::ast::Name) -> Option<&Type> {
        self.bindings.get(&(std::ptr::from_ref(n) as usize))
    }

    pub(super) fn set_binding(&mut self, n: &crate::ast::Name, t: Type) {
        self.bindings.insert(std::ptr::from_ref(n) as usize, t);
    }

    /// Which of its type's constructors `new` expression `e` chose.
    pub fn constructor(&self, e: &Expr) -> Option<usize> {
        self.constructors.get(&key(e)).copied()
    }

    pub(super) fn set_constructor(&mut self, e: &Expr, index: usize) {
        self.constructors.insert(key(e), index);
    }

    pub fn method(&self, e: &Expr) -> Option<&MethodUse> {
        self.methods.get(&key(e))
    }

    pub(super) fn set_method(&mut self, e: &Expr, used: MethodUse) {
        self.methods.insert(key(e), used);
    }

    /// `e`'s type, if it was checked.
    pub fn of(&self, e: &Expr) -> Option<&Type> {
        self.types.get(&key(e))
    }

    /// The declared operator `e` is, if it's one.
    pub fn operator(&self, e: &Expr) -> Option<&OperatorUse> {
        self.operators.get(&key(e))
    }

    /// The implicit conversion `e`'s value goes through, if any.
    pub fn conversion(&self, e: &Expr) -> Option<&OperatorUse> {
        self.conversions.get(&key(e))
    }

    /// The declared operator `place op= …` applies, if it's one.
    pub fn assigned(&self, place: &Expr) -> Option<&OperatorUse> {
        self.assigned.get(&key(place))
    }

    pub(super) fn set(&mut self, e: &Expr, t: Type) {
        self.types.insert(key(e), t);
    }

    pub(super) fn set_operator(&mut self, e: &Expr, used: OperatorUse) {
        self.operators.insert(key(e), used);
    }

    pub(super) fn set_assigned(&mut self, place: &Expr, used: OperatorUse) {
        self.assigned.insert(key(place), used);
    }

    pub(super) fn set_conversion(&mut self, e: &Expr, used: OperatorUse) {
        self.conversions.insert(key(e), used);
    }
}

/// A node's address: its identity in the tree.
fn key(e: &Expr) -> usize {
    std::ptr::from_ref(e) as usize
}
