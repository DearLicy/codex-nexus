//! Provider routing and safe artifact handling for the desktop application.
//!
//! The crate intentionally has no desktop UI dependency. It can be used from
//! the native Slint host, a CLI, or a test process without requiring a WebView.

pub mod artifacts;
pub mod discovery;
pub mod image_pool;
pub mod models;
pub mod providers;
pub mod routing;
pub mod secrets;
pub mod store;

pub use artifacts::{ArtifactError, ArtifactScanner, ScanConfig};
pub use discovery::{CodexSessionRecord, DiscoveryError, QuarantineResult, WorkspaceFileRecord};
pub use image_pool::{ImageAssignment, ImagePool, ImagePoolMember};
pub use models::{
    AccountPool, ArtifactRecord, ArtifactStatus, ImageJob, JobStatus, ProviderAccount, RoutePolicy,
    SelectionStrategy,
};
pub use providers::{ProviderAdapter, ProviderCapabilities, WireApi};
pub use routing::{RetryError, RouteDecision, RouteError, Router};
