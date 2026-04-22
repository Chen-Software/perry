//! Structured errors for HIR lowering.
//!
//! Lowering returns `anyhow::Result` throughout, but when the failure has a
//! known source location we wrap it in a `LowerError` so downstream tooling
//! (notably `perry check`) can downcast and produce a diagnostic with a
//! proper span instead of a locationless message.

use swc_common::Span;

/// A lowering failure, optionally with a SWC span pointing at the offending
/// AST node.
#[derive(Debug, Clone)]
pub struct LowerError {
    pub message: String,
    pub span: Option<Span>,
}

impl LowerError {
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span: Some(span),
        }
    }

    pub fn without_span(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            span: None,
        }
    }
}

impl std::fmt::Display for LowerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for LowerError {}

/// Return early from a lowering function with a span-tagged error.
///
/// Usage: `lower_bail!(some_ast_node.span, "unsupported thing: {}", detail);`
#[macro_export]
macro_rules! lower_bail {
    ($span:expr, $($arg:tt)*) => {
        return ::std::result::Result::Err(::anyhow::Error::new(
            $crate::error::LowerError::new(format!($($arg)*), $span),
        ))
    };
}
