//! Statement codegen — Phase 1.
//!
//! Only `Stmt::Expr(Expr)` is supported, and only when the expression is
//! exactly `console.log(<number>)` (see `expr.rs`). Everything else raises a
//! clear error so a user running `--backend llvm` on a more complex program
//! gets a one-line explanation of what's not yet supported.

use anyhow::{anyhow, Result};
use perry_hir::Stmt;

use crate::block::LlBlock;
use crate::expr;

/// Lower a single top-level init statement into `block`.
pub(crate) fn lower_init_stmt(block: &mut LlBlock, stmt: &Stmt) -> Result<()> {
    match stmt {
        Stmt::Expr(e) => {
            let double_str = expr::match_console_log_number(e)?;
            expr::emit_console_log_number(block, &double_str);
            Ok(())
        }
        other => Err(anyhow!(
            "perry-codegen-llvm Phase 1: only Stmt::Expr is supported at top level; got {}",
            stmt_variant_name(other)
        )),
    }
}

fn stmt_variant_name(s: &Stmt) -> &'static str {
    match s {
        Stmt::Expr(_) => "Expr",
        _ => "<other>",
    }
}
