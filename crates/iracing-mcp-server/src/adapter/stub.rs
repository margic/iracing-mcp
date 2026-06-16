use async_trait::async_trait;

use super::{IracingAdapter, SessionOverview};

#[derive(Debug, Default)]
pub struct StubAdapter;

#[async_trait]
impl IracingAdapter for StubAdapter {
    async fn get_session_overview(&self) -> SessionOverview {
        SessionOverview {
            connected: true,
            is_replay: true,
            is_in_car: false,
            session_name: "Practice".to_string(),
            track_name: "Stub Track".to_string(),
        }
    }
}
