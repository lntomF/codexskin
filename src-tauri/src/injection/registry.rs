use crate::cdp::CdpClient;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::oneshot;

pub struct RegisteredTarget {
    pub target_id: String,
    pub target_url: String,
    pub client: Arc<CdpClient>,
    pub new_document_script_id: String,
    pub registration_id: u64,
    pub reload_watcher_stop: oneshot::Sender<()>,
}

#[derive(Default)]
pub struct InjectionRegistry {
    pub targets: HashMap<String, RegisteredTarget>,
}
