mod client;
mod local_endpoint;
pub(crate) mod reconnect;
mod targets;

pub use client::{CdpClient, CdpEvent};
pub use targets::{discover_page_targets, PageTarget};
