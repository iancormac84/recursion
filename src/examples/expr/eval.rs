use crate::examples::expr::Expr;

use crate::examples::expr::naive::{generate_layer, ExprAST};
use crate::map_layer::MapLayer;
#[cfg(any(test, feature = "experimental"))]
use crate::stack_machine::experimental::{expand_and_collapse_short_circuit, ShortCircuit};
use crate::stack_machine::{
    expand_and_collapse, expand_and_collapse_result, expand_and_collapse_v,
};
#[cfg(test)]
use crate::{
    examples::expr::naive::arb_expr,
    examples::expr::{BlocAllocExpr, DFSStackExpr},
    recursive::{Collapse, Expand},
};
#[cfg(test)]
use proptest::prelude::*;

#[derive(Debug, Clone)]
pub struct ValidInt(i64);

#[derive(Debug, Clone)]
pub enum CompiledExpr<A> {
    Add(A, A),
    Sub(A, A),
    Mul(A, A),
    LiteralInt(ValidInt),
}

impl<A, B> MapLayer<B> for CompiledExpr<A> {
    type To = CompiledExpr<B>;
    type Unwrapped = A;

    #[inline(always)]
    fn map_layer<F: FnMut(Self::Unwrapped) -> B>(self, mut f: F) -> Self::To {
        match self {
            CompiledExpr::Add(a, b) => CompiledExpr::Add(f(a), f(b)),
            CompiledExpr::Sub(a, b) => CompiledExpr::Sub(f(a), f(b)),
            CompiledExpr::Mul(a, b) => CompiledExpr::Mul(f(a), f(b)),
            CompiledExpr::LiteralInt(x) => CompiledExpr::LiteralInt(x),
        }
    }
}

type CompileError = &'static str;

pub fn eval_lazy_with_fused_compile(expr: &ExprAST) -> Result<i64, CompileError> {
    expand_and_collapse_result(
        expr,
        |seed| compile(generate_layer(seed)),
        |compiled| Ok(eval_compiled(compiled)),
    )
}

// only looks at literal case - add/sub/mul ops are always valid
pub fn compile<A>(expr: Expr<A>) -> Result<CompiledExpr<A>, CompileError> {
    match expr {
        Expr::Add(a, b) => Ok(CompiledExpr::Add(a, b)),
        Expr::Sub(a, b) => Ok(CompiledExpr::Sub(a, b)),
        Expr::Mul(a, b) => Ok(CompiledExpr::Mul(a, b)), // TODO: look into futumorphism to return multiple layers here
        Expr::LiteralInt(x) => {
            // arbitrary check
            if x > 99 {
                return Err("invalid literal");
            }

            Ok(CompiledExpr::LiteralInt(ValidInt(x)))
        }
    }
}

pub fn eval_compiled(expr: CompiledExpr<i64>) -> i64 {
    match expr {
        CompiledExpr::Add(a, b) => a + b,
        CompiledExpr::Sub(a, b) => a - b,
        CompiledExpr::Mul(a, b) => a * b,
        CompiledExpr::LiteralInt(ValidInt(x)) => x,
    }
}

#[inline(always)]
pub fn eval_layer(node: Expr<i64>) -> i64 {
    match node {
        Expr::Add(a, b) => a + b,
        Expr::Sub(a, b) => a - b,
        Expr::Mul(a, b) => a * b,
        Expr::LiteralInt(x) => x,
    }
}

pub fn naive_eval(expr: &ExprAST) -> i64 {
    match expr {
        ExprAST::Add(a, b) => naive_eval(a) + naive_eval(b),
        ExprAST::Sub(a, b) => naive_eval(a) - naive_eval(b),
        ExprAST::Mul(a, b) => naive_eval(a) * naive_eval(b),
        ExprAST::LiteralInt(x) => *x,
    }
}

pub fn eval_lazy_2(expr: &ExprAST) -> i64 {
    let (res, v) = expand_and_collapse_v(expr, generate_layer, eval_layer);
    for v in v.iter() {
        println!("{:?}", v);
    }
    panic!();
    res
}

pub fn eval_lazy(expr: &ExprAST) -> i64 {
    expand_and_collapse(expr, generate_layer, eval_layer)
}

pub fn eval_lazy_et(expr: &ExprAST) -> i64 {
    expand_and_collapse_short_circuit(
        expr,
        |e| {
            let layer = generate_layer(e);

            let sc = if let Expr::Mul(_, _) = layer {
                Some(ShortCircuit {
                    short_circuit_on: 0,
                    return_on_short_circuit: 0,
                })
            } else {
                None
            };

            layer.map_layer(|seed| (seed, sc))
        },
        eval_layer,
    )
}


    #[test]
    fn expr_eval_simple() {
        let expr = ExprAST::Add(Box::new(ExprAST::LiteralInt(1)), Box::new(ExprAST::LiteralInt(2)));
        let _lazy_stack_eval_2 = eval_lazy_2(&expr);
    }

// generate a bunch of expression trees and evaluate them
#[cfg(test)]
proptest! {
    #[test]
    fn expr_eval(expr in arb_expr()) {
        // NOTE: this helped me find one serious bug in new cata impl, where it was doing vec pop instead of vec head_pop so switched to VecDequeue. Found minimal example, Add (0, Sub(0, 1)).
        let simple = naive_eval(&expr);
        let dfs_stack_eval = DFSStackExpr::expand_layers(&expr, generate_layer).collapse_layers(eval_layer);
        let bloc_alloc_eval = BlocAllocExpr::expand_layers(&expr, generate_layer).collapse_layers(eval_layer);
        let lazy_stack_eval = eval_lazy(&expr);
        // let lazy_stack_eval_2 = eval_lazy_2(&expr);
        let lazy_eval_new = expr.collapse_layers(eval_layer);
        let lazy_eval_et = eval_lazy_et(&expr);
        // let lazy_stack_eval_compiled = eval_lazy_with_fused_compile(expr).unwrap();


        assert_eq!(simple, dfs_stack_eval);
        assert_eq!(simple, bloc_alloc_eval);
        assert_eq!(simple, lazy_stack_eval);
        // assert_eq!(simple, lazy_stack_eval_2);
        assert_eq!(simple, lazy_eval_new);
        assert_eq!(simple, lazy_eval_et);
        // will fail because literals > 99 are invalid in compiled ctx
        // assert_eq!(simple, lazy_stack_eval_compiled);
    }
}
