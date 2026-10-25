//! An operator declared (`public static Vec operator +(Vec a, Vec b)`,
//! `public static implicit operator Energy(i32 v)`): a static method named
//! as the CLR names it (`op_Addition`, `op_Implicit`), so the checker,
//! inheritance and the JVM treat it as any method.

use yog_script_ids::clr;

use super::{BinOp, Method, UnOp};

/// Which operator a declaration is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperatorKind {
    Binary(BinOp),
    Unary(UnOp),
    /// A conversion that happens by itself where its target is wanted.
    Implicit,
    /// A conversion written as a cast (`(Energy)n`).
    Explicit,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Operator {
    pub kind: OperatorKind,
    /// Its method, named `kind.method_name()`.
    pub method: Method,
}

impl OperatorKind {
    /// Its method's name, the CLR's; none for an operator that can't be
    /// declared (`&&`, `||`, `??`, `try`, `await`, `in`).
    pub fn method_name(self) -> Option<&'static str> {
        Some(match self {
            OperatorKind::Binary(op) => match op {
                BinOp::Add => clr::OP_ADDITION,
                BinOp::Sub => clr::OP_SUBTRACTION,
                BinOp::Mul => clr::OP_MULTIPLY,
                BinOp::Div => clr::OP_DIVISION,
                BinOp::Rem => clr::OP_MODULUS,
                BinOp::BitAnd => clr::OP_BITWISE_AND,
                BinOp::BitOr => clr::OP_BITWISE_OR,
                BinOp::BitXor => clr::OP_EXCLUSIVE_OR,
                BinOp::Shl => clr::OP_LEFT_SHIFT,
                BinOp::Shr => clr::OP_RIGHT_SHIFT,
                BinOp::Eq => clr::OP_EQUALITY,
                BinOp::NotEq => clr::OP_INEQUALITY,
                BinOp::Lt => clr::OP_LESS_THAN,
                BinOp::LtEq => clr::OP_LESS_THAN_OR_EQUAL,
                BinOp::Gt => clr::OP_GREATER_THAN,
                BinOp::GtEq => clr::OP_GREATER_THAN_OR_EQUAL,
                BinOp::Coalesce | BinOp::Or | BinOp::And | BinOp::In => return None,
            },
            OperatorKind::Unary(op) => match op {
                UnOp::Neg => clr::OP_UNARY_NEGATION,
                UnOp::Not => clr::OP_LOGICAL_NOT,
                UnOp::BitNot => clr::OP_ONES_COMPLEMENT,
                UnOp::Increment => clr::OP_INCREMENT,
                UnOp::Decrement => clr::OP_DECREMENT,
                UnOp::PostIncrement | UnOp::PostDecrement | UnOp::Try | UnOp::Await => {
                    return None;
                }
            },
            OperatorKind::Implicit => clr::OP_IMPLICIT,
            OperatorKind::Explicit => clr::OP_EXPLICIT,
        })
    }
}
