//! Persistent authored tool work. Browser navigation and passive capture state
//! intentionally have no document kind and never enter this database.

mod model;
mod store;

pub use model::{Document, DocumentId, DocumentKind, Project, ProjectId};
pub use store::{ProjectStore, StoreError};
