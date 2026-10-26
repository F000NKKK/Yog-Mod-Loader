//! A lambda given where an `Expression<TDelegate>` is wanted: written
//! twice, as C# compiles it — the tree of its nodes (made by
//! `System.Linq.Expressions.Expression`'s factories) and the lambda itself,
//! which the tree's `Compile()` gives.
//! `Expression.quote(delegate, names, types, $ps -> tree, (D) lambda)`.

use yog_script_analyze::ast::{Arg, BinOp, Expr, ExprKind, LambdaBody, LambdaParam, UnOp};
use yog_script_analyze::{Quoted, Type};
use yog_script_ids::jvm::reflection::TYPE;
use yog_script_ids::jvm::trees::{METHOD_OF, PARAMETERS, QUOTE, STATIC_MEMBER};
use yog_script_ids::members::{
    ADD, AND, AND_ALSO, ARRAY_INDEX, CALL, COALESCE, CONDITION, CONSTANT, CONVERT, DIVIDE, EQUAL,
    EXCLUSIVE_OR, GREATER_THAN, GREATER_THAN_OR_EQUAL, LEFT_SHIFT, LESS_THAN, LESS_THAN_OR_EQUAL,
    MODULO, MULTIPLY, NEGATE, NOT, NOT_EQUAL, OR, OR_ELSE, PROPERTY_OR_FIELD, RIGHT_SHIFT,
    SUBTRACT,
};
use yog_script_ids::system::EXPRESSION;

use crate::Gen;

/// No object: a static method's call.
const NULL: &str = "null";

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
        let Some(tree) = self.tree(value, &names) else {
            return String::new();
        };
        let types: Vec<String> = quoted
            .params
            .iter()
            .map(|t| self.value_of(t).unwrap_or_default())
            .collect();
        let quoted_names: Vec<String> = names.iter().map(|n| self.code.str(n)).collect();
        let compiled = self.lambda_of(&names, body, quoted.ret == Type::Void);
        format!(
            "{}.{QUOTE}({}, new String[] {{ {} }}, new {TYPE}[] {{ {} }}, {PARAMETERS} -> {tree}, ({}) {compiled})",
            self.expressions(),
            self.value_of(&quoted.delegate).unwrap_or_default(),
            quoted_names.join(", "),
            types.join(", "),
            self.java(&quoted.delegate, false)
        )
    }

    /// `System.Linq.Expressions.Expression`'s class.
    fn expressions(&self) -> String {
        self.index.place(EXPRESSION, 0)
    }

    /// `e` as a tree's node, the lambda's parameters (`params`) its
    /// `$ps`; what can't be one yet reported.
    fn tree(&mut self, e: &Expr, params: &[String]) -> Option<String> {
        let ex = self.expressions();
        let node = match &e.kind {
            ExprKind::Ident(name) if let Some(i) = params.iter().position(|p| p == name) => {
                format!("{PARAMETERS}[{i}]")
            }
            ExprKind::Int(_)
            | ExprKind::Float(_)
            | ExprKind::Char(_)
            | ExprKind::Bool(_)
            | ExprKind::Null
            | ExprKind::Ident(_)
            | ExprKind::This => self.constant(e)?,
            ExprKind::Str(segs)
                if segs
                    .iter()
                    .all(|s| matches!(s, yog_script_analyze::ast::StrSeg::Text(_))) =>
            {
                self.constant(e)?
            }
            ExprKind::Member {
                expr,
                name,
                safe: false,
            } => self.member_node(expr, &name.text, params)?,
            ExprKind::Call { callee, args } => self.call_node(e, callee, args, params)?,
            ExprKind::Binary { op, left, right } if self.types.operator(e).is_none() => {
                let factory = binary_factory(*op)?;
                let (l, r) = (self.tree(left, params)?, self.tree(right, params)?);
                format!("{ex}.{factory}({l}, {r})")
            }
            ExprKind::Unary { op, expr } if self.types.operator(e).is_none() => {
                let factory = match op {
                    UnOp::Not | UnOp::BitNot => NOT,
                    UnOp::Neg => NEGATE,
                    _ => return self.not_tree(e),
                };
                format!("{ex}.{factory}({})", self.tree(expr, params)?)
            }
            ExprKind::Conditional {
                cond,
                then,
                otherwise,
            } => format!(
                "{ex}.{CONDITION}({}, {}, {})",
                self.tree(cond, params)?,
                self.tree(then, params)?,
                self.tree(otherwise, params)?
            ),
            ExprKind::Cast { ty, expr } => {
                let to = self.type_value(ty, &|_| None)?;
                format!("{ex}.{CONVERT}({}, {to})", self.tree(expr, params)?)
            }
            ExprKind::Index { expr, args }
                if args.len() == 1
                    && matches!(
                        self.types.of(expr).map(Type::non_null),
                        Some(Type::Array(..))
                    ) =>
            {
                format!(
                    "{ex}.{ARRAY_INDEX}({}, {})",
                    self.tree(expr, params)?,
                    self.tree(&args[0], params)?
                )
            }
            _ => return self.not_tree(e),
        };
        Some(node)
    }

    fn not_tree(&mut self, e: &Expr) -> Option<String> {
        self.not_yet("this in an expression tree", e.span);
        None
    }

    /// A value the tree holds as it is: a literal, a captured local, `this`.
    fn constant(&mut self, e: &Expr) -> Option<String> {
        let ex = self.expressions();
        let v = self.expr(e);
        match self.types.of(e).and_then(|t| self.value_of(t)) {
            Some(t) => Some(format!("{ex}.{CONSTANT}((Object) {v}, {t})")),
            None => Some(format!("{ex}.{CONSTANT}((Object) {v})")),
        }
    }

    /// `x.Name`: a property's or a field's read; a static one's of its type.
    fn member_node(&mut self, on: &Expr, name: &str, params: &[String]) -> Option<String> {
        let ex = self.expressions();
        let n = self.code.str(name);
        if let Some(Type::Static(of, args)) = self.types.of(on) {
            let owner = self.value_of(&Type::Own(of.clone(), args.clone()))?;
            return Some(format!("{ex}.{STATIC_MEMBER}({owner}, {n})"));
        }
        Some(format!(
            "{ex}.{PROPERTY_OR_FIELD}({}, {n})",
            self.tree(on, params)?
        ))
    }

    /// A call: the method the compiler chose, found among its type's.
    fn call_node(
        &mut self,
        e: &Expr,
        callee: &Expr,
        args: &[Arg],
        params: &[String],
    ) -> Option<String> {
        let ex = self.expressions();
        let Some(used) = self.types.method(e).cloned() else {
            return self.not_tree(e);
        };
        let Some(owner) = used.owner.clone() else {
            return self.not_tree(e);
        };
        let owner_type = crate::reflection::of_class(&self.index.place(&owner, used.owner_arity));
        let method = format!(
            "{ex}.{METHOD_OF}({owner_type}, {}, {})",
            self.code.str(&used.name),
            used.overload
        );
        let mut nodes = Vec::new();
        let receiver = match &callee.kind {
            ExprKind::Member { expr, .. }
                if !matches!(self.types.of(expr), Some(Type::Static(..))) =>
            {
                Some(self.tree(expr, params)?)
            }
            _ => None,
        };
        // An extension method's receiver is its first argument; a static's none.
        let instance = match receiver {
            Some(r) if used.extension => {
                nodes.push(r);
                NULL.to_string()
            }
            Some(r) => r,
            None => NULL.to_string(),
        };
        for a in args {
            nodes.push(self.tree(&a.value, params)?);
        }
        Some(format!(
            "{ex}.{CALL}({instance}, {method}, new {ex}[] {{ {} }})",
            nodes.join(", ")
        ))
    }
}

/// The factory of binary operator `op` (`a + b` is `Expression.Add`).
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
