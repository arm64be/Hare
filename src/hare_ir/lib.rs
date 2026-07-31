//! Owned, engine-independent intermediate representation for Hare.
//!
//! The crate deliberately contains no JavaScriptCore types. The protected JSC
//! adapter can only populate [`OwnedVisitorUnit`] through [`ImportBuilder`];
//! analysis and lowering consume a sealed unit or validated [`CompilationUnit`].

mod r#abstract;
mod api;
mod impls;

pub use r#abstract::{BlockView, FunctionView, OperationView, UnitView};
pub use api::*;
pub use impls::{DumpIdentity, ImportBuilder, lower_structural_unit, render_visitor_dump};
