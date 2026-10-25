//! An operator declared (`public static Vec operator +(Vec a, Vec b)`,
//! `public static implicit operator Energy(i32 v)`): a static method named
//! as the CLR names it (`op_Addition`, `op_Implicit`), so the checker,
//! inheritance and the JVM treat it as any method.

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
                BinOp::Add => "op_Addition",
                BinOp::Sub => "op_Subtraction",
                BinOp::Mul => "op_Multiply",
                BinOp::Div => "op_Division",
                BinOp::Rem => "op_Modulus",
                BinOp::BitAnd => "op_BitwiseAnd",
                BinOp::BitOr => "op_BitwiseOr",
                BinOp::BitXor => "op_ExclusiveOr",
                BinOp::Shl => "op_LeftShift",
                BinOp::Shr => "op_RightShift",
                BinOp::Eq => "op_Equality",
                BinOp::NotEq => "op_Inequality",
                BinOp::Lt => "op_LessThan",
                BinOp::LtEq => "op_LessThanOrEqual",
                BinOp::Gt => "op_GreaterThan",
                BinOp::GtEq => "op_GreaterThanOrEqual",
                BinOp::Coalesce | BinOp::Or | BinOp::And | BinOp::In => return None,
            },
            OperatorKind::Unary(op) => match op {
                UnOp::Neg => "op_UnaryNegation",
                UnOp::Not => "op_LogicalNot",
                UnOp::BitNot => "op_OnesComplement",
                UnOp::Increment => "op_Increment",
                UnOp::Decrement => "op_Decrement",
                UnOp::PostIncrement | UnOp::PostDecrement | UnOp::Try | UnOp::Await => {
                    return None;
                }
            },
            OperatorKind::Implicit => "op_Implicit",
            OperatorKind::Explicit => "op_Explicit",
        })
    }
}
