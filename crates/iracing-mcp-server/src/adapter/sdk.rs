use async_trait::async_trait;
use iracing::telemetry::Value;
use iracing_broadcast::{BroadcastMessage, Client as BroadcastClient};
use serde_yaml::Value as YamlValue;

#[cfg(windows)]
use std::{ffi::OsStr, os::windows::ffi::OsStrExt, ptr::null_mut, slice};

#[cfg(windows)]
use winapi::{
    shared::minwindef::FALSE,
    um::{
        errhandlingapi::GetLastError,
        handleapi::CloseHandle,
        memoryapi::{MapViewOfFile, OpenFileMappingW, UnmapViewOfFile, FILE_MAP_READ},
    },
};

use super::{AdapterError, IracingAdapter, ReplayState, SessionData, SessionOverview};

const IRSDK_MEMMAPFILENAME: &str = "Local\\IRSDKMemMapFileName";

#[repr(C)]
struct IrsdkHeaderPrefix {
    ver: i32,
    status: i32,
    tick_rate: i32,
    session_info_update: i32,
    session_info_len: i32,
    session_info_offset: i32,
}

#[derive(Debug, Default)]
pub struct SdkAdapter;

impl SdkAdapter {
    fn session_data_sync(&self) -> Result<SessionData, AdapterError> {
        let connection = iracing::Connection::new()
            .map_err(|error| AdapterError::NotConnected(error.to_string()))?;
        let telemetry = connection
            .telemetry()
            .map_err(|error| AdapterError::NotConnected(error.to_string()))?;
        let current_session_num = read_i32(&telemetry, "SessionNum")?;
        let session_yaml = read_session_yaml()?;

        parse_session_data(&session_yaml, current_session_num)
    }

    fn replay_state_sync(&self) -> Result<ReplayState, AdapterError> {
        let connection = iracing::Connection::new()
            .map_err(|error| AdapterError::NotConnected(error.to_string()))?;
        let sample = connection
            .telemetry()
            .map_err(|error| AdapterError::NotConnected(error.to_string()))?;

        Ok(ReplayState {
            connected: true,
            is_on_track: read_bool(&sample, "IsOnTrack")?,
            is_in_garage: read_bool(&sample, "IsInGarage")?,
            is_replay_playing: read_bool(&sample, "IsReplayPlaying")?,
            replay_play_speed: read_i32(&sample, "ReplayPlaySpeed")?,
            replay_play_slow_motion: read_bool(&sample, "ReplayPlaySlowMotion")?,
            replay_frame_num: read_i32(&sample, "ReplayFrameNum")?,
            replay_frame_num_end: read_i32(&sample, "ReplayFrameNumEnd")?,
            replay_session_num: read_i32(&sample, "ReplaySessionNum")?,
            replay_session_time: read_f64(&sample, "ReplaySessionTime")?,
            cam_car_idx: read_i32(&sample, "CamCarIdx")?,
            cam_group_number: read_i32(&sample, "CamGroupNumber")?,
            cam_camera_number: read_i32(&sample, "CamCameraNumber")?,
        })
    }

    fn set_replay_playback_sync(&self, speed: i32, slow_motion: bool) -> Result<(), AdapterError> {
        if !(0..=255).contains(&speed) {
            return Err(AdapterError::UnsupportedReplaySpeed(speed));
        }

        iracing::Connection::new().map_err(|error| AdapterError::NotConnected(error.to_string()))?;

        let client = BroadcastClient::new()
            .map_err(|error| AdapterError::Broadcast(error.to_string()))?;
        client
            .send_message(BroadcastMessage::ReplaySetPlaySpeed(speed as u8, slow_motion))
            .map_err(|error| AdapterError::Broadcast(error.to_string()))
    }
}

#[async_trait]
impl IracingAdapter for SdkAdapter {
    async fn get_session_overview(&self) -> SessionOverview {
        let session_data = self.session_data_sync().ok();
        let replay_state = self.replay_state_sync().ok();

        SessionOverview {
            connected: replay_state.is_some(),
            is_replay: replay_state
                .as_ref()
                .map(|state| {
                    state.is_replay_playing
                        || state.replay_frame_num > 0
                        || state.replay_session_time > 0.0
                })
                .unwrap_or(false),
            is_in_car: replay_state
                .as_ref()
                .map(|state| state.is_on_track || state.is_in_garage)
                .unwrap_or(false),
            session_name: session_data
                .as_ref()
                .map(|session| session.current_session_type.clone())
                .unwrap_or_else(|| "Disconnected".to_string()),
            track_name: session_data
                .as_ref()
                .map(|session| session.track_display_name.clone())
                .unwrap_or_else(|| "Disconnected".to_string()),
        }
    }

    async fn get_session_data(&self) -> Result<SessionData, AdapterError> {
        self.session_data_sync()
    }

    async fn get_replay_state(&self) -> Result<ReplayState, AdapterError> {
        self.replay_state_sync()
    }

    async fn set_replay_playback(
        &self,
        speed: i32,
        slow_motion: bool,
    ) -> Result<(), AdapterError> {
        self.set_replay_playback_sync(speed, slow_motion)
    }
}

fn read_bool(sample: &iracing::telemetry::Sample, name: &'static str) -> Result<bool, AdapterError> {
    match sample
        .get(name)
        .map_err(|_| AdapterError::MissingTelemetryVar(name))?
    {
        Value::BOOL(value) => Ok(value),
        _ => Err(AdapterError::InvalidTelemetryType(name)),
    }
}

fn read_i32(sample: &iracing::telemetry::Sample, name: &'static str) -> Result<i32, AdapterError> {
    match sample
        .get(name)
        .map_err(|_| AdapterError::MissingTelemetryVar(name))?
    {
        Value::INT(value) => Ok(value),
        _ => Err(AdapterError::InvalidTelemetryType(name)),
    }
}

fn read_f64(sample: &iracing::telemetry::Sample, name: &'static str) -> Result<f64, AdapterError> {
    match sample
        .get(name)
        .map_err(|_| AdapterError::MissingTelemetryVar(name))?
    {
        Value::DOUBLE(value) => Ok(value),
        Value::FLOAT(value) => Ok(value as f64),
        _ => Err(AdapterError::InvalidTelemetryType(name)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "requires a live iRacing session on this Windows machine"]
    async fn reads_live_session_data_and_replay_state() {
        let adapter = SdkAdapter;

        let session = adapter
            .get_session_data()
            .await
            .expect("live session data should be readable");
        let replay = adapter
            .get_replay_state()
            .await
            .expect("live replay state should be readable");

        assert!(!session.track_display_name.trim().is_empty());
        assert!(!session.current_session_type.trim().is_empty());
        assert!(session.session_count > 0);
        assert!(session.driver_count > 0);
        assert!(replay.connected);

        println!(
            "track={}; session={}; drivers={}; replay_frame={}; replay_session_num={}; replay_time={:.3}; replay_playing={}; on_track={}; in_garage={}",
            session.track_display_name,
            session.current_session_type,
            session.driver_count,
            replay.replay_frame_num,
            replay.replay_session_num,
            replay.replay_session_time,
            replay.is_replay_playing,
            replay.is_on_track,
            replay.is_in_garage,
        );

        assert!(replay.replay_session_time.is_finite());
    }

    #[tokio::test]
    async fn replay_speed_rejects_negative_values() {
        let adapter = SdkAdapter;
        let error = adapter
            .set_replay_playback(-1, false)
            .await
            .expect_err("negative speed should be rejected");

        assert!(matches!(error, AdapterError::UnsupportedReplaySpeed(-1)));
    }
}

fn parse_session_data(
    session_yaml: &str,
    current_session_num: i32,
) -> Result<SessionData, AdapterError> {
    let root: YamlValue =
        serde_yaml::from_str(session_yaml).map_err(|error| AdapterError::SessionInfo(error.to_string()))?;

    let track_display_name = yaml_str_at(&root, &["WeekendInfo", "TrackDisplayName"])?
        .to_string();
    let sessions = yaml_seq_at(&root, &["SessionInfo", "Sessions"])?;
    let driver_count = yaml_seq_at(&root, &["DriverInfo", "Drivers"])? .len();

    let current_session_type = sessions
        .iter()
        .find(|session| {
            session
                .get("SessionNum")
                .and_then(YamlValue::as_i64)
                == Some(current_session_num as i64)
        })
        .or_else(|| sessions.first())
        .and_then(|session| session.get("SessionType"))
        .and_then(YamlValue::as_str)
        .unwrap_or("Unknown")
        .to_string();

    Ok(SessionData {
        track_display_name,
        current_session_type,
        driver_count,
        session_count: sessions.len(),
    })
}

fn yaml_str_at<'a>(root: &'a YamlValue, path: &[&str]) -> Result<&'a str, AdapterError> {
    let value = yaml_value_at(root, path)?;
    value
        .as_str()
        .ok_or_else(|| AdapterError::SessionInfo(format!("{} is not a string", path.join("."))))
}

fn yaml_seq_at<'a>(root: &'a YamlValue, path: &[&str]) -> Result<&'a Vec<YamlValue>, AdapterError> {
    let value = yaml_value_at(root, path)?;
    value
        .as_sequence()
        .ok_or_else(|| AdapterError::SessionInfo(format!("{} is not a sequence", path.join("."))))
}

fn yaml_value_at<'a>(root: &'a YamlValue, path: &[&str]) -> Result<&'a YamlValue, AdapterError> {
    let mut current = root;

    for segment in path {
        current = current.get(*segment).ok_or_else(|| {
            AdapterError::SessionInfo(format!("missing {}", path.join(".")))
        })?;
    }

    Ok(current)
}

#[cfg(windows)]
fn read_session_yaml() -> Result<String, AdapterError> {
    let path: Vec<u16> = OsStr::new(IRSDK_MEMMAPFILENAME)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let mapping = OpenFileMappingW(FILE_MAP_READ, FALSE, path.as_ptr());
        if mapping.is_null() {
            return Err(AdapterError::NotConnected(std::io::Error::from_raw_os_error(
                GetLastError() as i32,
            )
            .to_string()));
        }

        let view = MapViewOfFile(mapping, FILE_MAP_READ, 0, 0, 0);
        if view.is_null() {
            let error = std::io::Error::from_raw_os_error(GetLastError() as i32).to_string();
            CloseHandle(mapping);
            return Err(AdapterError::NotConnected(error));
        }

        let result = read_session_yaml_from_view(view);

        UnmapViewOfFile(view);
        CloseHandle(mapping);

        result
    }
}

#[cfg(not(windows))]
fn read_session_yaml() -> Result<String, AdapterError> {
    Err(AdapterError::NotConnected(
        "shared-memory session YAML is only available on Windows".to_string(),
    ))
}

#[cfg(windows)]
unsafe fn read_session_yaml_from_view(view: *mut std::ffi::c_void) -> Result<String, AdapterError> {
    if view == null_mut() {
        return Err(AdapterError::NotConnected(
            "shared-memory view pointer was null".to_string(),
        ));
    }

    let header = &*(view as *const IrsdkHeaderPrefix);
    let start = (view as usize + header.session_info_offset as usize) as *const u8;
    let bytes = slice::from_raw_parts(start, header.session_info_len as usize);

    Ok(String::from_utf8_lossy(bytes)
        .trim_end_matches('\0')
        .to_string())
}