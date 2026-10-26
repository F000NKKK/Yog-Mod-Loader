//! A lambda given where an `Expression<TDelegate>` is wanted: written
//! twice, as C# compiles it — the tree of its nodes (made by
//! `System.Linq.Expressions.Expression`'s factories) and the lambda itself,
//! which the tree's `Compile()` gives.
//! `Expression.quote(delegate, names, types, $ps0 -> tree, (D) lambda)`; a
//! lambda inside it is a node run by the tree (`Expression.nested`).

mod nodes;

use yog_script_analyze::ast::{BinOp, Expr, LambdaBody, LambdaParam};
use yog_script_analyze::{Quoted, Type};
use yog_script_ids::jvm::reflection::TYPE;
use yog_script_ids::jvm::trees::{NESTED, PARAMETERS, QUOTE};
use yog_script_ids::members::{
    ADD, AND, AND_ALSO, COALESCE, DIVIDE, EQUAL, EXCLUSIVE_OR, GREATER_THAN, GREATER_THAN_OR_EQUAL,
    LEFT_SHIFT, LESS_THAN, LESS_THAN_OR_EQUAL, MODULO, MULTIPLY, NOT_EQUAL, OR, OR_ELSE,
    RIGHT_SHIFT, SUBTRACT,
};
use yog_script_ids::system::EXPRESSION;

use crate::Gen;

/// The lambdas a node is inside, innermost last: each one's parameters' names.
type Scopes = Vec<Vec<String>>;

impl Gen<'_> {
    /// Quoted lambda `e` (`quoted` says of what delegate).
    pub(crate) fn quoted_lambda(
        &mut self,
        e: &Expr,
        (params, body): (&[LambdaParam], &LambdaBody),
        quoted: &Quoted,
    ) -> String {
        let names = self.lambda_names(params, quoted.params.is_empty());
        let LambdaBody::Expr(value) = body else {
            self.not_yet("an expression tree of a block", e.span);
            return String::new();
        };
        let scopes: Scopes = vec![names.clone()];
        let Some(tree) = self.tree(value, &scopes) else {
            return String::new();
        };
        let compiled = self.lambda_of(&names, body, quoted.ret == Type::Void);
        format!(
            "{}.{QUOTE}({}, {}, {}0 -> {tree}, ({}) {compiled})",
            self.expressions(),
            self.value_of(&quoted.delegate).unwrap_or_default(),
            self.parameters_of(&names, &quoted.params),
            PARAMETERS,
            self.java(&quoted.delegate, false)
        )
    }

    /// A lambda inside a tree: a node run by the tree, the parameters of
    /// the lambdas around it (`scopes`) seen from it.
    fn nested_lambda(
        &mut self,
        e: &Expr,
        (params, body): (&[LambdaParam], &LambdaBody),
        scopes: &Scopes,
    ) -> Option<String> {
        let delegate = self.types.of(e).cloned();
        let Some(Type::Fn(types, _)) = delegate.clone() else {
            return self.not_tree(e);
        };
        let delegate = delegate?;
        let LambdaBody::Expr(value) = body else {
            return self.not_tree(e);
        };
        let names = self.lambda_names(params, types.is_empty());
        let mut inner = scopes.clone();
        inner.push(names.clone());
        let tree = self.tree(value, &inner)?;
        let class = erased(&self.java(&delegate, false));
        Some(format!(
            "{}.{NESTED}({class}.class, {}, {}, {}{} -> {tree})",
            self.expressions(),
            self.value_of(&delegate)?,
            self.parameters_of(&names, &types),
            PARAMETERS,
            scopes.len()
        ))
    }

    /// `new String[] { "x" }, new Type[] { … }`: a lambda's parameters.
    fn parameters_of(&self, names: &[String], types: &[Type]) -> String {
        let quoted: Vec<String> = names.iter().map(|n| self.code.str(n)).collect();
        let types: Vec<String> = types
            .iter()
            .map(|t| self.value_of(t).unwrap_or_default())
            .collect();
        format!(
            "new String[] {{ {} }}, new {TYPE}[] {{ {} }}",
            quoted.join(", "),
            types.join(", ")
        )
    }

    /// `System.Linq.Expressions.Expression`'s class.
    fn expressions(&self) -> String {
        self.index.place(EXPRESSION, 0)
    }

    fn not_tree(&mut self, e: &Expr) -> Option<String> {
        self.not_yet("this in an expression tree", e.span);
        None
    }
}

/// Java type `java` without its arguments (`Func2<A, B>` is `Func2`).
fn erased(java: &str) -> String {
    java.split('<').next().unwrap_or(java).to_string()
}

/// The factory, and node type, of binary operator `op` (`a + b` is `Add`).
fn binary_factory(op: BinOp) -> Option<&'static str> {
    Some(match op {
        BinOp::Add => ADD,
        BinOp::Sub => SUBTRACT,
        BinOp::Mul => MULTIPLY,
        BinOp::Div => DIVIDE,
        BinOp::Rem => MODULO,
        BinOp::BitAnd => AND,
        BinOp::BitOr => OR,
        BinOp::BitXor => EXCLUSIVE_OR,
        BinOp::Shl => LEFT_SHIFT,
        BinOp::Shr => RIGHT_SHIFT,
        BinOp::And => AND_ALSO,
        BinOp::Or => OR_ELSE,
        BinOp::Eq => EQUAL,
        BinOp::NotEq => NOT_EQUAL,
        BinOp::Lt => LESS_THAN,
        BinOp::LtEq => LESS_THAN_OR_EQUAL,
        BinOp::Gt => GREATER_THAN,
        BinOp::GtEq => GREATER_THAN_OR_EQUAL,
        BinOp::Coalesce => COALESCE,
        BinOp::In => return None,
    })
}
