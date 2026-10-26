//! An expression tree's nodes, each of what the lambda's body writes:
//! constants, its lambdas' parameters, members by name, calls and `new`s by
//! what the compiler chose, operators (a type's own by its method), `?:`,
//! casts, an array's item, a string's parts, a lambda inside it.

use yog_script_analyze::ast::{Arg, Expr, ExprKind, StrSeg, UnOp};
use yog_script_analyze::{OperatorUse, Type};
use yog_script_ids::jvm::reflection::TYPE;
use yog_script_ids::jvm::trees::{
    CONSTRUCTOR_OF, METHOD_OF, OPERATOR_OF, PARAMETERS, STATIC_MEMBER,
};
use yog_script_ids::members::{
    ADD, ARRAY_INDEX, CALL, CONDITION, CONSTANT, CONVERT, MAKE_BINARY, MAKE_UNARY, NEGATE, NEW,
    NOT, PROPERTY_OR_FIELD,
};
use yog_script_ids::system::EXPRESSION_TYPE;

use super::{Scopes, binary_factory};
use crate::Gen;
use crate::reflection::of_class;

/// No object: a static method's call.
const NULL: &str = "null";

impl Gen<'_> {
    /// `e` as a tree's node, inside the lambdas `scopes`; what can't be one
    /// yet reported.
    pub(super) fn tree(&mut self, e: &Expr, scopes: &Scopes) -> Option<String> {
        let ex = self.expressions();
        let node = match &e.kind {
            ExprKind::Ident(name) if let Some(p) = parameter(scopes, name) => p,
            ExprKind::Int(_)
            | ExprKind::Float(_)
            | ExprKind::Char(_)
            | ExprKind::Bool(_)
            | ExprKind::Null
            | ExprKind::Ident(_)
            | ExprKind::This => self.constant(e)?,
            ExprKind::Str(segs) => self.text_node(e, segs, scopes)?,
            ExprKind::Member {
                expr,
                name,
                safe: false,
            } => self.member_node(expr, &name.text, scopes)?,
            ExprKind::Call { callee, args } => self.call_node(e, callee, args, scopes)?,
            ExprKind::Binary { op, left, right } => {
                let factory = binary_factory(*op)?;
                self.binary_node(e, factory, (left, right), scopes)?
            }
            ExprKind::Unary { op, expr } => {
                let factory = match op {
                    UnOp::Not | UnOp::BitNot => NOT,
                    UnOp::Neg => NEGATE,
                    _ => return self.not_tree(e),
                };
                let operand = self.tree(expr, scopes)?;
                self.unary_node(e, (factory, &operand), expr)?
            }
            ExprKind::Conditional {
                cond,
                then,
                otherwise,
            } => format!(
                "{ex}.{CONDITION}({}, {}, {})",
                self.tree(cond, scopes)?,
                self.tree(then, scopes)?,
                self.tree(otherwise, scopes)?
            ),
            ExprKind::Cast { ty, expr } => {
                let operand = self.tree(expr, scopes)?;
                let to = self.type_value(ty, &|_| None)?;
                self.cast_node(e, (&operand, &to), expr)?
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
                    self.tree(expr, scopes)?,
                    self.tree(&args[0], scopes)?
                )
            }
            ExprKind::New {
                ty: Some(_),
                args: Some(args),
                init,
            } if init.is_empty() => self.new_node(e, args, scopes)?,
            ExprKind::Lambda { params, body } => self.nested_lambda(e, (params, body), scopes)?,
            _ => return self.not_tree(e),
        };
        Some(node)
    }

    /// `Expression`'s node types' enum (`ExpressionType`).
    fn node_types(&self) -> String {
        self.index.place(EXPRESSION_TYPE, 0)
    }

    /// `l op r`: the language's, or a type's own operator.
    fn binary_node(
        &mut self,
        e: &Expr,
        factory: &str,
        (left, right): (&Expr, &Expr),
        scopes: &Scopes,
    ) -> Option<String> {
        let ex = self.expressions();
        let (l, r) = (self.tree(left, scopes)?, self.tree(right, scopes)?);
        match self.types.operator(e).cloned() {
            Some(used) => {
                let method = self.operator_node(&used, &[left, right])?;
                Some(format!(
                    "{ex}.{MAKE_BINARY}({}.{factory}, {l}, {r}, false, {method})",
                    self.node_types()
                ))
            }
            None => Some(format!("{ex}.{factory}({l}, {r})")),
        }
    }

    /// `(T)x`: a number's conversion, or a type's own.
    fn cast_node(&mut self, e: &Expr, (operand, to): (&str, &str), of: &Expr) -> Option<String> {
        let ex = self.expressions();
        match self.types.operator(e).cloned() {
            Some(used) => {
                let method = self.operator_node(&used, &[of])?;
                Some(format!(
                    "{ex}.{MAKE_UNARY}({}.{CONVERT}, {operand}, {to}, {method})",
                    self.node_types()
                ))
            }
            None => Some(format!("{ex}.{CONVERT}({operand}, {to})")),
        }
    }

    /// `-x`, `!x`: the language's, or a type's own operator.
    fn unary_node(
        &mut self,
        e: &Expr,
        (factory, operand): (&str, &str),
        of: &Expr,
    ) -> Option<String> {
        let ex = self.expressions();
        match self.types.operator(e).cloned() {
            Some(used) => {
                let method = self.operator_node(&used, &[of])?;
                let t = self.value_of(self.types.of(e)?)?;
                Some(format!(
                    "{ex}.{MAKE_UNARY}({}.{factory}, {operand}, {t}, {method})",
                    self.node_types()
                ))
            }
            None => Some(format!("{ex}.{factory}({operand})")),
        }
    }

    /// A type's operator `used` taking `operands`, found among its methods.
    fn operator_node(&mut self, used: &OperatorUse, operands: &[&Expr]) -> Option<String> {
        let owner = self.value_of(&used.owner)?;
        let types = operands
            .iter()
            .map(|o| self.types.of(o).and_then(|t| self.value_of(t)))
            .collect::<Option<Vec<_>>>()?;
        Some(format!(
            "{}.{OPERATOR_OF}({owner}, {}, new {TYPE}[] {{ {} }})",
            self.expressions(),
            self.code.str(&used.method),
            types.join(", ")
        ))
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

    /// `"a {x} b"`: its parts added, a string's first.
    fn text_node(&mut self, e: &Expr, segs: &[StrSeg], scopes: &Scopes) -> Option<String> {
        let ex = self.expressions();
        let string = self.value_of(&Type::String)?;
        let text =
            |g: &Self, t: &str| format!("{ex}.{CONSTANT}((Object) {}, {string})", g.code.str(t));
        if segs.iter().all(|s| matches!(s, StrSeg::Text(_))) {
            return self.constant(e);
        }
        // A string first, so `+` joins text: its first part, or an empty one.
        let mut out = match segs.first() {
            Some(StrSeg::Text(t)) => text(self, t),
            _ => text(self, ""),
        };
        let rest = match segs.first() {
            Some(StrSeg::Text(_)) => &segs[1..],
            _ => segs,
        };
        for s in rest {
            let part = match s {
                StrSeg::Text(t) => text(self, t),
                StrSeg::Expr(x) => self.tree(x, scopes)?,
            };
            out = format!("{ex}.{ADD}({out}, {part})");
        }
        Some(out)
    }

    /// `x.Name`: a property's or a field's read; a static one's of its type.
    fn member_node(&mut self, on: &Expr, name: &str, scopes: &Scopes) -> Option<String> {
        let ex = self.expressions();
        let n = self.code.str(name);
        if let Some(Type::Static(of, args)) = self.types.of(on) {
            let owner = self.value_of(&Type::Own(of.clone(), args.clone()))?;
            return Some(format!("{ex}.{STATIC_MEMBER}({owner}, {n})"));
        }
        Some(format!(
            "{ex}.{PROPERTY_OR_FIELD}({}, {n})",
            self.tree(on, scopes)?
        ))
    }

    /// A call: the method the compiler chose, found among its type's.
    fn call_node(
        &mut self,
        e: &Expr,
        callee: &Expr,
        args: &[Arg],
        scopes: &Scopes,
    ) -> Option<String> {
        let ex = self.expressions();
        let used = self.types.method(e).cloned()?;
        let Some(owner) = used.owner.clone() else {
            return self.not_tree(e);
        };
        let owner_type = of_class(&self.index.place(&owner, used.owner_arity));
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
                Some(self.tree(expr, scopes)?)
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
            nodes.push(self.tree(&a.value, scopes)?);
        }
        Some(format!(
            "{ex}.{CALL}({instance}, {method}, new {ex}[] {{ {} }})",
            nodes.join(", ")
        ))
    }

    /// `new T(args)`: the constructor the compiler chose, found among its type's.
    fn new_node(&mut self, e: &Expr, args: &[Arg], scopes: &Scopes) -> Option<String> {
        let ex = self.expressions();
        let made = self.value_of(self.types.of(e)?)?;
        let Some(overload) = self.types.constructor(e) else {
            return self.not_tree(e);
        };
        let nodes = args
            .iter()
            .map(|a| self.tree(&a.value, scopes))
            .collect::<Option<Vec<_>>>()?;
        Some(format!(
            "{ex}.{NEW}({ex}.{CONSTRUCTOR_OF}({made}, {overload}), new {ex}[] {{ {} }})",
            nodes.join(", ")
        ))
    }
}

/// Lambda parameter `name`'s node: the innermost lambda's that has it.
fn parameter(scopes: &Scopes, name: &str) -> Option<String> {
    scopes.iter().enumerate().rev().find_map(|(depth, names)| {
        names
            .iter()
            .position(|n| n == name)
            .map(|i| format!("{PARAMETERS}{depth}[{i}]"))
    })
}
