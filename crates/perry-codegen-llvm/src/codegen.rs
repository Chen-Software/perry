//! HIR → LLVM IR compilation entry point.
//!
//! Public contract:
//!
//! ```ignore
//! let opts = CompileOptions { target: None, is_entry_module: true };
//! let object_bytes: Vec<u8> = perry_codegen_llvm::compile_module(&hir, opts)?;
//! ```
//!
//! The returned bytes are a regular object file (`.o` on macOS/Linux, `.obj`
//! on Windows) produced by `clang -c`. Perry's existing linking stage in
//! `crates/perry/src/commands/compile.rs` picks them up identically to the
//! Cranelift output — no linker changes needed.
//!
//! Phase 1 scope: emit a `main` function that calls `js_gc_init()` once and
//! then lowers the entry module's `init` statements. Only `console.log(<number>)`
//! is supported at statement level; everything else errors with a clear
//! "unsupported" message from `stmt::lower_init_stmt`.

use anyhow::{anyhow, Context, Result};
use perry_hir::Module as HirModule;

use crate::module::LlModule;
use crate::runtime_decls;
use crate::stmt;
use crate::types::I32;

/// Options mirrored from the Cranelift backend's setter API — only the
/// handful Phase 1 needs. More fields get added as later phases require them.
#[derive(Debug, Clone, Default)]
pub struct CompileOptions {
    /// Target triple override. `None` uses the host default
    /// (`arm64-apple-macosx15.0.0` on Apple Silicon macOS).
    pub target: Option<String>,
    /// Whether this module is the program entry point. When true, codegen
    /// emits a `main` function that calls `js_gc_init` and then the module's
    /// top-level statements.
    pub is_entry_module: bool,
}

/// Compile a Perry HIR module to an object file via LLVM IR.
pub fn compile_module(hir: &HirModule, opts: CompileOptions) -> Result<Vec<u8>> {
    let triple = opts
        .target
        .clone()
        .unwrap_or_else(default_target_triple);

    let mut llmod = LlModule::new(&triple);
    runtime_decls::declare_phase1(&mut llmod);

    // Phase 1 only supports single-file, entry-module programs. Reject
    // anything else now with a useful error rather than silently producing
    // a broken binary.
    if !opts.is_entry_module {
        return Err(anyhow!(
            "perry-codegen-llvm Phase 1 only supports the entry module; \
             non-entry module '{}' is not yet supported",
            hir.name
        ));
    }
    if !hir.imports.is_empty() {
        return Err(anyhow!(
            "perry-codegen-llvm Phase 1 does not support imports; module '{}' has {} imports",
            hir.name,
            hir.imports.len()
        ));
    }
    if !hir.classes.is_empty() || !hir.functions.is_empty() {
        return Err(anyhow!(
            "perry-codegen-llvm Phase 1 only supports top-level statements; \
             module '{}' has {} classes and {} functions",
            hir.name,
            hir.classes.len(),
            hir.functions.len()
        ));
    }
    if hir.init.is_empty() {
        return Err(anyhow!(
            "perry-codegen-llvm Phase 1: module '{}' has no init statements to lower",
            hir.name
        ));
    }

    // Build `int main() { js_gc_init(); <init stmts>; return 0; }`
    let main = llmod.define_function("main", I32, vec![]);
    // Phase 1 has no control flow, so one entry block is sufficient.
    let entry = main.create_block("entry");

    // Runtime bootstrap. Matches what the Cranelift backend does at the top
    // of every entry `main`.
    entry.call_void("js_gc_init", &[]);

    // Lower top-level statements one at a time. Phase 1 only accepts
    // `console.log(<number>)`; anything else propagates an error.
    for (idx, s) in hir.init.iter().enumerate() {
        stmt::lower_init_stmt(entry, s).with_context(|| {
            format!(
                "lowering init statement #{} of module '{}'",
                idx, hir.name
            )
        })?;
    }

    entry.ret(I32, "0");

    // Hand the serialized IR to clang -c and return the object bytes.
    let ll_text = llmod.to_ir();
    log::debug!(
        "perry-codegen-llvm: emitted {} bytes of LLVM IR for '{}'",
        ll_text.len(),
        hir.name
    );
    crate::linker::compile_ll_to_object(&ll_text, opts.target.as_deref())
}

/// Host default triple. Mirrors anvil's hardcoded value for macOS; later
/// phases plumb Perry's existing cross-target table here.
fn default_target_triple() -> String {
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "arm64-apple-macosx15.0.0".to_string()
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        "x86_64-apple-macosx15.0.0".to_string()
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        "x86_64-unknown-linux-gnu".to_string()
    } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
        "aarch64-unknown-linux-gnu".to_string()
    } else {
        "arm64-apple-macosx15.0.0".to_string()
    }
}
