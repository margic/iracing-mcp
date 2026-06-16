use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub mod stub;

pub use stub::StubAdapter;

pub type AdapterRef = Arc<dyn IracingAdapter>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionOverview {
    pub connected: bool,
    pub is_replay: bool,
    pub is_in_car: bool,
    pub session_name: String,
    pub track_name: String,
}

#[async_trait]
pub trait IracingAdapter: Send + Sync {
    async fn get_session_overview(&self) -> SessionOverview;
}
