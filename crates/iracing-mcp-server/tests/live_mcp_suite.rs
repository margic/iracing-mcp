use std::{ffi::OsStr, os::windows::ffi::OsStrExt, slice, sync::Arc};

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use iracing_mcp_server::{adapter, mcp, transport};
use serde_json::{json, Value};
use serde_yaml::Value as YamlValue;
use tower::ServiceExt;
use winapi::{
    shared::minwindef::FALSE,
    um::{
        errhandlingapi::GetLastError,
        handleapi::CloseHandle,
        memoryapi::{MapViewOfFile, OpenFileMappingW, UnmapViewOfFile, FILE_MAP_READ},
    },
};

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

#[tokio::test]
#[ignore = "requires live iRacing replay/spectator mode"]
async fn live_mcp_replay_playback_verifies_and_restores() {
    let app = build_live_app();
    let before = call_tool(app.clone(), "replay_get_state", json!({})).await;
    assert!(!before["isOnTrack"].as_bool().unwrap_or(true));
    assert!(!before["isInGarage"].as_bool().unwrap_or(true));

    let paused = call_tool(
        app.clone(),
        "replay_set_playback",
        json!({ "speed": 0, "slowMotion": false }),
    )
    .await;
    assert_eq!(paused["verified"], Value::Bool(true));
    assert_eq!(paused["observed"]["replayPlaySpeed"], Value::from(0));

    let resumed = call_tool(
        app,
        "replay_set_playback",
        json!({ "speed": 1, "slowMotion": false }),
    )
    .await;
    assert_eq!(resumed["verified"], Value::Bool(true));
    assert_eq!(resumed["observed"]["replayPlaySpeed"], Value::from(1));
}

#[tokio::test]
#[ignore = "requires live iRacing replay/spectator mode"]
async fn live_mcp_camera_focus_switches_target_and_camera() {
    let app = build_live_app();
    let before = call_tool(app.clone(), "replay_get_state", json!({})).await;
    assert!(!before["isOnTrack"].as_bool().unwrap_or(true));
    assert!(!before["isInGarage"].as_bool().unwrap_or(true));

    let current_car_idx = before["camCarIdx"].as_i64().expect("camCarIdx as i64") as i32;
    let current_group = before["camGroupNumber"].as_i64().expect("camGroupNumber as i64") as i32;
    let current_camera = before["camCameraNumber"].as_i64().expect("camCameraNumber as i64") as i32;

    let session_yaml = read_session_yaml().expect("session yaml available");
    let target_car_idx = choose_alternate_car_idx(&session_yaml, current_car_idx)
        .expect("an alternate focus car should exist");
    let (target_group, target_camera) = choose_alternate_camera(&session_yaml, current_group, current_camera)
        .expect("an alternate camera should exist");

    let focused_car = call_tool(
        app.clone(),
        "camera_focus",
        json!({
            "carIdx": target_car_idx,
            "groupNumber": current_group,
            "cameraNumber": current_camera
        }),
    )
    .await;
    assert_eq!(focused_car["verified"], Value::Bool(true));
    assert_eq!(focused_car["observed"]["camCarIdx"], Value::from(target_car_idx));

    let switched_camera = call_tool(
        app.clone(),
        "camera_focus",
        json!({
            "carIdx": target_car_idx,
            "groupNumber": target_group,
            "cameraNumber": target_camera
        }),
    )
    .await;
    assert_eq!(switched_camera["verified"], Value::Bool(true));
    assert_eq!(switched_camera["observed"]["camGroupNumber"], Value::from(target_group));
    assert_eq!(switched_camera["observed"]["camCameraNumber"], Value::from(target_camera));

    let restored = call_tool(
        app,
        "camera_focus",
        json!({
            "carIdx": current_car_idx,
            "groupNumber": current_group,
            "cameraNumber": current_camera
        }),
    )
    .await;
    assert_eq!(restored["verified"], Value::Bool(true));
}

#[tokio::test]
#[ignore = "requires live iRacing replay/spectator mode"]
async fn live_mcp_replay_seek_session_time_verifies_and_restores() {
    let app = build_live_app();
    let before = call_tool(app.clone(), "replay_get_state", json!({})).await;
    assert!(!before["isOnTrack"].as_bool().unwrap_or(true));
    assert!(!before["isInGarage"].as_bool().unwrap_or(true));

    let session_num = before["replaySessionNum"].as_i64().expect("replaySessionNum as i64") as i32;
    let original_time_ms = (before["replaySessionTime"].as_f64().expect("replaySessionTime as f64") * 1000.0)
        .round() as i32;
    let target_time_ms = if original_time_ms > 2_000 {
        original_time_ms - 2_000
    } else {
        original_time_ms + 2_000
    };

    let seek = call_tool(
        app.clone(),
        "replay_seek_session_time",
        json!({
            "sessionNum": session_num,
            "sessionTimeMs": target_time_ms,
            "toleranceMs": 2000
        }),
    )
    .await;
    assert_eq!(seek["verified"], Value::Bool(true));

    let restored = call_tool(
        app,
        "replay_seek_session_time",
        json!({
            "sessionNum": session_num,
            "sessionTimeMs": original_time_ms,
            "toleranceMs": 2000
        }),
    )
    .await;
    assert_eq!(restored["verified"], Value::Bool(true));
}

fn build_live_app() -> axum::Router {
    let state = mcp::ServerState::new(Arc::new(adapter::SdkAdapter));
    transport::http::build_router(state)
}

async fn call_tool(app: axum::Router, name: &str, arguments: Value) -> Value {
    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": name,
            "arguments": arguments
        }
    });

    let response = app
        .oneshot(
            Request::builder()
                .uri("/mcp")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(request.to_string()))
                .expect("valid request"),
        )
        .await
        .expect("router response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("read body");
    let json: Value = serde_json::from_slice(&body).expect("json body");
    let payload = json
        .get("result")
        .and_then(|v| v.get("structuredContent"))
        .cloned()
        .unwrap_or_else(|| {
            panic!(
                "tool {name} returned unexpected response envelope: {json}"
            )
        });

    let ok = payload.get("ok").and_then(Value::as_bool).unwrap_or(false);
    if !ok {
        let error = payload.get("error").cloned().unwrap_or(Value::Null);
        panic!(
            "tool {name} returned MCP error: {error}; full payload: {payload}"
        );
    }

    payload.get("data").cloned().unwrap_or_else(|| {
        panic!(
            "tool {name} returned ok=true but missing data field: {payload}"
        )
    })
}

fn choose_alternate_car_idx(session_yaml: &str, current_car_idx: i32) -> Option<i32> {
    let root = parse_session_root(session_yaml).ok()?;
    let drivers = root
        .get("DriverInfo")?
        .get("Drivers")?
        .as_sequence()?;

    drivers.iter().find_map(|driver| {
        let is_spectator = driver.get("IsSpectator").and_then(YamlValue::as_i64).unwrap_or(1);
        let car_idx = driver.get("CarIdx").and_then(YamlValue::as_i64)? as i32;
        if is_spectator == 0 && car_idx != current_car_idx {
            Some(car_idx)
        } else {
            None
        }
    })
}

fn choose_alternate_camera(
    session_yaml: &str,
    current_group: i32,
    current_camera: i32,
) -> Option<(i32, i32)> {
    let root = parse_session_root(session_yaml).ok()?;
    let groups = root
        .get("CameraInfo")?
        .get("Groups")?
        .as_sequence()?;

    for group in groups {
        let group_num = group.get("GroupNum").and_then(YamlValue::as_i64)? as i32;
        let cameras = group.get("Cameras")?.as_sequence()?;

        if group_num == current_group {
            if let Some(camera_num) = cameras.iter().find_map(|camera| {
                let camera_num = camera.get("CameraNum").and_then(YamlValue::as_i64)? as i32;
                if camera_num != current_camera {
                    Some(camera_num)
                } else {
                    None
                }
            }) {
                return Some((group_num, camera_num));
            }
        }
    }

    for group in groups {
        let group_num = group.get("GroupNum").and_then(YamlValue::as_i64)? as i32;
        let cameras = group.get("Cameras")?.as_sequence()?;
        if let Some(camera_num) = cameras
            .first()
            .and_then(|camera| camera.get("CameraNum"))
            .and_then(YamlValue::as_i64)
        {
            if group_num != current_group || camera_num as i32 != current_camera {
                return Some((group_num, camera_num as i32));
            }
        }
    }

    None
}

fn parse_session_root(session_yaml: &str) -> Result<YamlValue, serde_yaml::Error> {
    serde_yaml::from_str(session_yaml)
}

fn wide_string(value: &str) -> Vec<u16> {
    OsStr::new(value)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

fn read_session_yaml() -> Result<String, std::io::Error> {
    let path = wide_string(IRSDK_MEMMAPFILENAME);

    unsafe {
        let mapping = OpenFileMappingW(FILE_MAP_READ, FALSE, path.as_ptr());
        if mapping.is_null() {
            return Err(std::io::Error::from_raw_os_error(GetLastError() as i32));
        }

        let view = MapViewOfFile(mapping, FILE_MAP_READ, 0, 0, 0);
        if view.is_null() {
            let error = std::io::Error::from_raw_os_error(GetLastError() as i32);
            CloseHandle(mapping);
            return Err(error);
        }

        let header = &*(view as *const IrsdkHeaderPrefix);
        let start = (view as usize + header.session_info_offset as usize) as *const u8;
        let bytes = slice::from_raw_parts(start, header.session_info_len as usize);
        let result = String::from_utf8_lossy(bytes)
            .trim_end_matches('\0')
            .to_string();

        UnmapViewOfFile(view);
        CloseHandle(mapping);

        Ok(result)
    }
}
