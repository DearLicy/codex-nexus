//! Provider routing and safe artifact handling for the desktop application.
//!
//! The crate intentionally has no Tauri dependency.  It can be used from
//! Tauri commands, a CLI, or a test process without requiring a WebView.

pub mod artifacts;
pub mod image_pool;
pub mod models;
pub mod providers;
pub mod routing;
pub mod secrets;
pub mod store;

pub use artifacts::{ArtifactError, ArtifactScanner, ScanConfig};
pub use image_pool::{ImageAssignment, ImagePool, ImagePoolMember};
pub use models::{
    AccountPool, ArtifactRecord, ArtifactStatus, ImageJob, JobStatus, ProviderAccount, RoutePolicy,
    SelectionStrategy,
};
pub use providers::{ProviderAdapter, ProviderCapabilities, WireApi};
pub use routing::{RetryError, RouteDecision, RouteError, Router};
