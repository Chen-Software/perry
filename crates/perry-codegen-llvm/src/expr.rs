//! Expression codegen — Phase 1.
//!
//! Scope is intentionally tiny: exactly the expressions needed to lower
//! `console.log(42)` end-to-end. Anything else returns an explicit
//! "unsupported" error so we never silently generate garbage.

use anyhow::{anyhow, Result};
use perry_hir::Expr;

use crate::block::LlBlock;
use crate::nanbox::double_literal;

/// Lower a numeric literal (`Integer(i64)` or `Number(f64)`) to a raw LLVM
/// `double` value expression — returned as a string because LLVM constants
/// live in-line in the instruction stream, they don't occupy a register.
pub(crate) fn lower_number_literal(expr: &Expr) -> Result<String> {
    match expr {
        Expr::Integer(i) => Ok(double_literal(*i as f64)),
        Expr::Number(f) => Ok(double_literal(*f)),
        other => Err(anyhow!(
            "perry-codegen-llvm Phase 1: expected number literal, got {}",
            variant_name(other)
        )),
    }
}

/// Identify the `console.log(<number>)` call pattern and return the raw
/// double string for the argument. Any divergence from this exact pattern
/// yields an actionable error — the caller passes this up as the
/// "module failed to compile" message.
pub(crate) fn match_console_log_number(expr: &Expr) -> Result<String> {
    let (callee, args) = match expr {
        Expr::Call { callee, args, .. } => (callee.as_ref(), args),
        _ => return Err(anyhow!(
            "Phase 1 only supports a top-level Call expression; got {}",
            variant_name(expr)
        )),
    };

    // Callee must be PropertyGet { object: GlobalGet(_), property: "log" }.
    // Phase 1 doesn't resolve GlobalId → "console", it just trusts that the
    // property name `"log"` on *any* global is console.log. That's enough to
    // distinguish our one supported pattern; later phases plumb a proper
    // global name table.
    let property = match callee {
        Expr::PropertyGet { object, property } => {
            if !matches!(object.as_ref(), Expr::GlobalGet(_)) {
                return Err(anyhow!(
                    "Phase 1 only supports console.log(<number>); callee.object is not a GlobalGet"
                ));
            }
            property.as_str()
        }
        _ => return Err(anyhow!(
            "Phase 1 only supports console.log(<number>); callee is not a PropertyGet"
        )),
    };

    if property != "log" {
        return Err(anyhow!(
            "Phase 1 only supports console.log(<number>); got console.{}",
            property
        ));
    }

    if args.len() != 1 {
        return Err(anyhow!(
            "Phase 1 only supports a single numeric argument to console.log; got {}",
            args.len()
        ));
    }

    lower_number_literal(&args[0])
}

/// Emit the LLVM instructions for `console.log(<number>)` into `block`.
///
/// Uses `js_console_log_number`, which takes a raw `double` (not NaN-boxed),
/// so we can skip the whole box/unbox dance for Phase 1.
pub(crate) fn emit_console_log_number(block: &mut LlBlock, double_str: &str) {
    block.call_void("js_console_log_number", &[(crate::types::DOUBLE, double_str)]);
}

fn variant_name(e: &Expr) -> &'static str {
    match e {
        Expr::Undefined => "Undefined",
        Expr::Null => "Null",
        Expr::Bool(_) => "Bool",
        Expr::Number(_) => "Number",
        Expr::Integer(_) => "Integer",
        Expr::BigInt(_) => "BigInt",
        Expr::String(_) => "String",
        Expr::I18nString { .. } => "I18nString",
        Expr::LocalGet(_) => "LocalGet",
        Expr::LocalSet(_, _) => "LocalSet",
        Expr::GlobalGet(_) => "GlobalGet",
        Expr::GlobalSet(_, _) => "GlobalSet",
        Expr::Update { .. } => "Update",
        Expr::Binary { .. } => "Binary",
        Expr::Unary { .. } => "Unary",
        Expr::Compare { .. } => "Compare",
        Expr::Logical { .. } => "Logical",
        Expr::Call { .. } => "Call",
        Expr::CallSpread { .. } => "CallSpread",
        Expr::FuncRef(_) => "FuncRef",
        Expr::ExternFuncRef { .. } => "ExternFuncRef",
        Expr::NativeModuleRef(_) => "NativeModuleRef",
        Expr::NativeMethodCall { .. } => "NativeMethodCall",
        Expr::PropertyGet { .. } => "PropertyGet",
        Expr::PropertySet { .. } => "PropertySet",
        Expr::PropertyUpdate { .. } => "PropertyUpdate",
        _ => "<other>",
    }
}
