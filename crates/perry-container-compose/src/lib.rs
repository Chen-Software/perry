//! Provides a Docker Compose-like experience for Apple's native Container
//! framework.  Can be used:
//!
//! 1. As a standalone CLI binary (`perry-compose`)
//! 2. As a library imported from Perry TypeScript applications
//! 3. Via FFI from compiled Perry TypeScript code (requires `ffi` feature)
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use perry_container_compose::orchestrate::Orchestrator;
//!
//! # #[tokio::main]
//! # async fn main() -> perry_container_compose::error::Result<()> {
//! let orchestrator = Orchestrator::new(&[], None, &[])?;
//! orchestrator.up(&[], true, false).await?;
//! # Ok(())
//! # }
//! ```

pub mod backend;
pub mod cli;
pub mod commands;
pub mod entities;
pub mod error;
pub mod orchestrate;

// FFI exports (Perry TypeScript integration)
#[cfg(feature = "ffi")]
pub mod ffi;

// Re-exports
pub use error::{ComposeError, Result};
pub use orchestrate::Orchestrator;
