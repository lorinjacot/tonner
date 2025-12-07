#[cfg(feature = "web")]
pub use web::{EngineContext, EngineProvider};

#[cfg(feature = "desktop")]
pub use native::{EngineContext, EngineProvider};

#[cfg(feature = "web")]
mod web;

#[cfg(feature = "desktop")]
mod native;
