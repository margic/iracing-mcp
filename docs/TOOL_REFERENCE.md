# iracing-mcp — Tool Reference

This document defines every MCP tool the server exposes: its purpose, input schema, output data
structure, the iRacing SDK / telemetry sources it uses, and the verification it performs.

> Conventions
> - All schemas are shown as JSON Schema fragments suitable for the MCP `tools/list` response.
> - All timestamps are ISO‑8601 UTC strings.
> - All tools return the **common response envelope** (see §1).
> - Mutating tools additionally return the **verification block** (see §2 and
>   [FEEDBACK_VERIFICATION.md](FEEDBACK_VERIFICATION.md)).

---

## 1. Common response envelope

Every tool result `data` payload is wrapped in this envelope:

```jsonc
{
  "ok": true,                       // boolean — tool completed without a hard error
  "asOfUtc": "2026-06-16T14:32:10Z",// when the data/observation was taken
  "sourceTick": 183402,             // telemetry tick (irsdk varBuf tickCount) if available
  "sessionInfoUpdate": 42,          // session-string update counter (for YAML-derived data)
  "data": { /* tool-specific */ },
  "warnings": [],                   // array of human-readable strings
  "error": null                     // null on success, else an Error object (see §3)
}
```

`sourceTick` corresponds to the SDK `tickCount` that increments on each telemetry write
(`irsdk_varBuf` in [irsdk_defines.h](../iracing/irsdk-1-20/irsdk_1_20/irsdk_defines.h#L390)).
`sessionInfoUpdate` corresponds to `getSessionInfoStrUpdate()` in
[irsdk_client.h](../iracing/irsdk-1-20/irsdk_1_20/irsdk_client.h#L75).

---

## 2. Verification block (mutating tools)

```jsonc
{
  "commandAccepted": true,   // server successfully sent the broadcast message
  "verified": true,          // observed telemetry matched the expected post-condition
  "reason": null,            // string explaining why verified=false, else null
  "before": { /* relevant vars snapshot */ },
  "observed": { /* relevant vars after the action */ },
  "elapsedMs": 120,          // time spent polling for confirmation
  "pollCount": 8             // number of telemetry samples inspected
}
```

`commandAccepted` only means the Win32 broadcast was sent (it is always best-effort; see the
fire-and-forget transport in [irsdk_utils.cpp](../iracing/irsdk-1-20/irsdk_1_20/irsdk_utils.cpp#L365)).
`verified` is the value the agent should trust.

---

## 3. Error object & taxonomy

```jsonc
{ "code": "wrong_mode", "message": "Player is in the car; replay control unavailable.", "retryable": false }
```

| `code` | Meaning | Typical agent action |
| --- | --- | --- |
| `not_connected` | SDK not connected to a running sim | Surface to user; stop |
| `wrong_mode` | Player in car (replay/camera need out-of-car — [irsdk_defines.h](../iracing/irsdk-1-20/irsdk_1_20/irsdk_defines.h#L453)) | Ask user to exit car |
| `target_not_found` | Driver/car/camera not in session | Re-resolve or report |
| `invalid_argument` | Input failed schema/semantic validation | Fix and retry |
| `not_verified` | Command sent but telemetry didn't confirm | Retry / fallback |
| `timeout` | No telemetry update within `timeoutMs` | Retry with longer timeout |

---

## 4. Session interrogation tools (read-only)

### 4.1 `get_session_overview`

Fast, single-call snapshot for agent grounding.

**Input**

```jsonc
{
  "type": "object",
  "properties": {
    "maxAgeMs": { "type": "integer", "minimum": 0, "default": 250,
      "description": "Reuse cached telemetry if newer than this." }
  },
  "additionalProperties": false
}
```

**Output `data`**

```jsonc
{
  "connected": true,
  "inCar": false,                  // from IsOnTrack / IsInGarage
  "isReplayPlaying": true,         // IsReplayPlaying
  "sessionNum": 2,                 // SessionNum
  "sessionState": "Racing",        // SessionState enum (irsdk_SessionState)
  "sessionTime": 872.04,           // SessionTime (s)
  "sessionTimeRemain": 1200.0,     // SessionTimeRemain (s)
  "track": { "name": "Watkins Glen", "config": "Boot", "lengthKm": 5.43 },
  "counts": { "drivers": 24, "carClasses": 2, "cameraGroups": 18 }
}
```

**Sources:** live vars `IsOnTrack`, `IsInGarage`, `IsReplayPlaying`, `SessionNum`, `SessionState`,
`SessionTime`, `SessionTimeRemain`
([telemetry_11_23_15.md](../iracing/telemetry_11_23_15.md#L128)); track + counts from the session
YAML `WeekendInfo` ([telemetry_11_23_15.md](../iracing/telemetry_11_23_15.md#L453)).
`sessionState` is mapped from [`irsdk_SessionState`](../iracing/irsdk-1-20/irsdk_1_20/irsdk_defines.h#L185).

---

### 4.2 `get_weekend_info`

Static event metadata.

**Input:** `{ "type": "object", "properties": {}, "additionalProperties": false }`

**Output `data`**

```jsonc
{
  "track": {
    "name": "%s", "id": 0, "displayName": "%s", "configName": "%s",
    "lengthKm": 0.0, "city": "%s", "country": "%s",
    "altitudeM": 0.0, "latitude": 0.0, "longitude": 0.0,
    "numTurns": 0, "pitSpeedLimitKph": 0.0, "type": "%s"
  },
  "weather": {
    "type": "%s", "skies": "%s", "surfaceTempC": 0.0, "airTempC": 0.0,
    "airPressureHg": 0.0, "windVelMs": 0.0, "windDirRad": 0.0,
    "relativeHumidityPct": 0, "fogLevelPct": 0
  },
  "event": {
    "seriesId": 0, "seasonId": 0, "sessionId": 0, "subSessionId": 0,
    "leagueId": 0, "official": false, "raceWeek": 0,
    "eventType": "%s", "category": "%s", "simMode": "%s", "teamRacing": false
  }
}
```

**Source:** session YAML `WeekendInfo` block
([telemetry_11_23_15.md](../iracing/telemetry_11_23_15.md#L453)).

---

### 4.3 `get_roster`

Driver/team/car registry; the basis for `resolve_driver`.

**Input**

```jsonc
{
  "type": "object",
  "properties": {
    "includeSpectators": { "type": "boolean", "default": false },
    "includePaceCar": { "type": "boolean", "default": false }
  },
  "additionalProperties": false
}
```

**Output `data`**

```jsonc
{
  "entries": [
    {
      "carIdx": 7,
      "userName": "Max Verstappen",
      "abbrevName": "Verstappen, M",
      "initials": "MV",
      "userId": 123456,
      "teamName": "Red Bull",
      "carNumber": "1",          // display string (may be zero-padded)
      "carNumberRaw": 1,
      "carId": 145,
      "carScreenName": "Mercedes W13",
      "carClassId": 84,
      "carClassShortName": "F1",
      "irating": 7421,
      "licString": "A 4.99",
      "isSpectator": false
    }
  ],
  "count": 24
}
```

**Source:** session YAML `DriverInfo.Drivers[]`
([telemetry_11_23_15.md](../iracing/telemetry_11_23_15.md#L606)). `carIdx` is the stable key used by
all camera/replay car-targeted operations.

---

### 4.4 `get_camera_groups`

Discover legal camera options **before** issuing camera commands.

**Input:** `{ "type": "object", "properties": {}, "additionalProperties": false }`

**Output `data`**

```jsonc
{
  "groups": [
    {
      "groupNum": 3,
      "groupName": "TV1",
      "isScenic": false,
      "cameras": [
        { "cameraNum": 1, "cameraName": "CITIZEN" },
        { "cameraNum": 2, "cameraName": "Gantry" }
      ]
    }
  ],
  "count": 18
}
```

**Source:** session YAML `CameraInfo.Groups[]`
([telemetry_11_23_15.md](../iracing/telemetry_11_23_15.md#L566)). `groupNum`/`cameraNum` feed
`camera_focus`.

---

### 4.5 `get_standings`

Current order and timing for a session.

**Input**

```jsonc
{
  "type": "object",
  "properties": {
    "sessionNum": { "type": "integer", "minimum": 0,
      "description": "Defaults to the current session." }
  },
  "additionalProperties": false
}
```

**Output `data`**

```jsonc
{
  "sessionNum": 2,
  "sessionType": "Race",
  "positions": [
    {
      "position": 1,
      "classPosition": 1,
      "carIdx": 7,
      "lap": 18,
      "lapsComplete": 17,
      "fastestLap": 12,
      "fastestTime": 92.431,
      "lastTime": 93.115,
      "incidents": 2,
      "reasonOut": "Running"
    }
  ]
}
```

**Source:** session YAML `SessionInfo.Sessions[].ResultsPositions[]`
([telemetry_11_23_15.md](../iracing/telemetry_11_23_15.md#L504)). Live race position can also be
cross-checked with `CarIdxPosition` / `CarIdxClassPosition`
([telemetry_11_23_15.md](../iracing/telemetry_11_23_15.md#L322)).

---

### 4.6 `resolve_driver`

Map a spoken/typed name to a stable `carIdx`.

**Input**

```jsonc
{
  "type": "object",
  "properties": {
    "query": { "type": "string", "minLength": 1,
      "description": "Name, partial name, initials, or car number." },
    "limit": { "type": "integer", "minimum": 1, "maximum": 10, "default": 3 }
  },
  "required": ["query"],
  "additionalProperties": false
}
```

**Output `data`**

```jsonc
{
  "bestMatch": {
    "carIdx": 7, "displayName": "Max Verstappen", "carNumber": "1",
    "confidence": 0.94, "matchReason": "given_name_prefix"
  },
  "candidates": [
    { "carIdx": 7, "displayName": "Max Verstappen", "confidence": 0.94 },
    { "carIdx": 19, "displayName": "Max Chilton", "confidence": 0.51 }
  ]
}
```

**Source:** derived from `get_roster`. `matchReason` ∈ `exact`, `given_name_prefix`,
`surname_prefix`, `initials`, `car_number`, `fuzzy`.

---

### 4.7 `get_relatives`

Live track-order and gap view for all cars on track.

This is a computed telemetry view, not a session-YAML standings snapshot. It orders the field by
live track progress and estimated gaps from telemetry so the UI can choose whether to center or
filter it later.

**Input:** `{ "type": "object", "properties": {}, "additionalProperties": false }`

**Output `data`**

```jsonc
{
  "basis": "track",
  "sessionNum": 2,
  "observedAtUtc": "2026-06-16T14:32:10Z",
  "entries": [
    {
      "position": 1,
      "classPosition": 1,
      "carIdx": 7,
      "carNumber": "1",
      "displayName": "Max Verstappen",
      "lap": 18,
      "lapDistPct": 0.413,
      "isInPit": false,
      "gapAheadSec": null,
      "gapBehindSec": 0.842,
      "deltaLaps": 0,
      "estimatedTimeSec": 92.431,
      "f2TimeSec": 4.217
    }
  ],
  "count": 24
}
```

**Source:** live telemetry arrays, primarily `CarIdxLap`, `CarIdxLapDistPct`, `CarIdxEstTime`,
`CarIdxPosition`, `CarIdxClassPosition`, `CarIdxOnPitRoad`, and `CarIdxF2Time`
([telemetry_11_23_15.md](../iracing/telemetry_11_23_15.md#L322)).

**Implementation note:** the server should compute gaps from the live arrays and keep the response
stable across the full field, rather than requiring an anchor car or returning a narrowed window.

---

## 5. Replay control tools

> All replay tools require **out of car** (see [irsdk_defines.h](../iracing/irsdk-1-20/irsdk_1_20/irsdk_defines.h#L453)).
> They pre-check eligibility and return `wrong_mode` if violated.

### 5.1 `replay_get_state` (read-only)

**Input:** `{ "type": "object", "properties": {}, "additionalProperties": false }`

**Output `data`**

```jsonc
{
  "isReplayPlaying": true,        // IsReplayPlaying
  "playSpeed": 1,                 // ReplayPlaySpeed
  "slowMotion": false,            // ReplayPlaySlowMotion
  "frameNum": 52341,              // ReplayFrameNum (60/sec)
  "frameNumEnd": 98210,           // ReplayFrameNumEnd
  "sessionNum": 2,                // ReplaySessionNum
  "sessionTime": 872.04,          // ReplaySessionTime (s)
  "camera": {                     // current camera (Cam* vars)
    "carIdx": 7, "groupNum": 3, "cameraNum": 1, "stateBits": 12
  }
}
```

**Sources:** [telemetry_11_23_15.md](../iracing/telemetry_11_23_15.md#L110) (replay vars) and
[telemetry_11_23_15.md](../iracing/telemetry_11_23_15.md#L19) (camera vars).

---

### 5.2 `replay_set_playback` (mutating, verified)

Play, pause, fast-forward, rewind, slow-motion.

**Input**

```jsonc
{
  "type": "object",
  "properties": {
    "speed": { "type": "integer", "minimum": -16, "maximum": 16,
      "description": "0 = pause, 1 = normal, >1 fast-fwd, <0 rewind." },
    "slowMotion": { "type": "boolean", "default": false,
      "description": "If true, speed is interpreted as a slow-motion divisor." },
    "waitForConfirmation": { "type": "boolean", "default": true },
    "timeoutMs": { "type": "integer", "minimum": 0, "default": 750 }
  },
  "required": ["speed"],
  "additionalProperties": false
}
```

**Broadcast:** [`irsdk_BroadcastReplaySetPlaySpeed`](../iracing/irsdk-1-20/irsdk_1_20/irsdk_defines.h#L460)
with `(speed, slowMotion, 0)`. Sample usage:
[msgtest.cpp](../iracing/irsdk-1-20/irsdk_1_20/irsdk_msgtest/msgtest.cpp#L145).

**Verification:** `ReplayPlaySpeed == speed`, `ReplayPlaySlowMotion == slowMotion`, and
`IsReplayPlaying == (speed != 0)`.

---

### 5.3 `replay_seek_frame` (mutating, verified)

Seek to an absolute or relative frame.

**Input**

```jsonc
{
  "type": "object",
  "properties": {
    "mode": { "enum": ["begin", "current", "end"], "default": "begin",
      "description": "Frame offset origin (irsdk_RpyPosMode)." },
    "frame": { "type": "integer",
      "description": "Frame offset from origin; 60 frames = 1 second." },
    "waitForConfirmation": { "type": "boolean", "default": true },
    "timeoutMs": { "type": "integer", "minimum": 0, "default": 750 }
  },
  "required": ["frame"],
  "additionalProperties": false
}
```

**Broadcast:** [`irsdk_BroadcastReplaySetPlayPosition`](../iracing/irsdk-1-20/irsdk_1_20/irsdk_defines.h#L461)
with `(mode, frame)`. `mode` maps to
[`irsdk_RpyPosMode`](../iracing/irsdk-1-20/irsdk_1_20/irsdk_defines.h#L534). Sample:
[msgtest.cpp](../iracing/irsdk-1-20/irsdk_1_20/irsdk_msgtest/msgtest.cpp#L165).

**Verification:** `ReplayFrameNum` lands within a tolerance (default ±4 frames) of the target
(`begin → frame`, `end → frameNumEnd − frame`, `current → before.frameNum + frame`).

---

### 5.4 `replay_seek_session_time` (mutating, verified)

Seek to a wall-clock session time — the **preferred** addressing for "at 14:32" style commands.

**Input**

```jsonc
{
  "type": "object",
  "properties": {
    "sessionNum": { "type": "integer", "minimum": 0 },
    "sessionTimeMs": { "type": "integer", "minimum": 0,
      "description": "Milliseconds since session start." },
    "toleranceMs": { "type": "integer", "minimum": 0, "default": 500 },
    "waitForConfirmation": { "type": "boolean", "default": true },
    "timeoutMs": { "type": "integer", "minimum": 0, "default": 1000 }
  },
  "required": ["sessionNum", "sessionTimeMs"],
  "additionalProperties": false
}
```

**Broadcast:** [`irsdk_BroadcastReplaySearchSessionTime`](../iracing/irsdk-1-20/irsdk_1_20/irsdk_defines.h#L467)
with `(sessionNum, sessionTimeMS)` (packed high/low).

**Verification:** `ReplaySessionNum == sessionNum` and
`|ReplaySessionTime*1000 − sessionTimeMs| <= toleranceMs`
([telemetry_11_23_15.md](../iracing/telemetry_11_23_15.md#L121)).

---

### 5.5 `replay_search_event` (mutating, verified)

Semantic jumps to laps, incidents, sessions, or frames.

**Input**

```jsonc
{
  "type": "object",
  "properties": {
    "mode": {
      "enum": ["to_start","to_end","prev_session","next_session",
               "prev_lap","next_lap","prev_frame","next_frame",
               "prev_incident","next_incident"]
    },
    "waitForConfirmation": { "type": "boolean", "default": true },
    "timeoutMs": { "type": "integer", "minimum": 0, "default": 750 }
  },
  "required": ["mode"],
  "additionalProperties": false
}
```

**Broadcast:** [`irsdk_BroadcastReplaySearch`](../iracing/irsdk-1-20/irsdk_1_20/irsdk_defines.h#L462)
with the corresponding [`irsdk_RpySrchMode`](../iracing/irsdk-1-20/irsdk_1_20/irsdk_defines.h#L519).
Sample: [msgtest.cpp](../iracing/irsdk-1-20/irsdk_1_20/irsdk_msgtest/msgtest.cpp#L156).

**Verification:** `ReplayFrameNum` changes from `before.frameNum` (direction-checked where the mode
implies it, e.g. `next_*` should increase). For `to_start`, expect `ReplayFrameNum → ~0`.

---

### 5.6 `replay_show_window` (composite, verified)

Convenience tool that performs a full "show this moment" sequence: seek → focus → set speed →
optionally pause at the end of the window. Built for the agent's most common request.

**Input**

```jsonc
{
  "type": "object",
  "properties": {
    "sessionNum": { "type": "integer", "minimum": 0 },
    "startTimeMs": { "type": "integer", "minimum": 0 },
    "endTimeMs": { "type": "integer", "minimum": 0 },
    "focusCarIdx": { "type": "integer", "minimum": 0 },
    "cameraGroupNum": { "type": "integer", "description": "Optional preferred group." },
    "speed": { "type": "integer", "default": 1 },
    "timeoutMs": { "type": "integer", "minimum": 0, "default": 2000 }
  },
  "required": ["sessionNum", "startTimeMs", "focusCarIdx"],
  "additionalProperties": false
}
```

**Behavior:** ensures replay is paused before seeking, then calls `replay_seek_session_time` → `camera_focus` → `replay_set_playback`.
Aggregates each step's verification into a single response so the agent can see which sub-step failed.

**Output `data`**

```jsonc
{
  "steps": [
    { "tool": "replay_seek_session_time", "verified": true },
    { "tool": "camera_focus", "verified": true },
    { "tool": "replay_set_playback", "verified": true }
  ],
  "finalState": { /* same shape as replay_get_state.data */ }
}
```

---

## 6. Camera control tools

### 6.1 `camera_focus` (mutating, verified)

Point the broadcast camera at a car (by index/number) or a special focus target.

**Input**

```jsonc
{
  "type": "object",
  "properties": {
    "carIdx": { "type": "integer", "minimum": 0,
      "description": "Preferred. Resolved to car number internally." },
    "carNumber": { "type": "integer",
      "description": "Alternative to carIdx." },
    "focusTarget": { "enum": ["incident","leader","exiting","driver"],
      "description": "Special focus (irsdk_csMode). Overrides carIdx/carNumber." },
    "groupNum": { "type": "integer", "description": "Camera group; default keep current." },
    "cameraNum": { "type": "integer", "default": 0, "description": "0 = let sim choose." },
    "waitForConfirmation": { "type": "boolean", "default": true },
    "timeoutMs": { "type": "integer", "minimum": 0, "default": 750 }
  },
  "additionalProperties": false
}
```

**Broadcast:**
[`irsdk_BroadcastCamSwitchNum`](../iracing/irsdk-1-20/irsdk_1_20/irsdk_defines.h#L458) for a specific
car number, or [`irsdk_BroadcastCamSwitchPos`](../iracing/irsdk-1-20/irsdk_1_20/irsdk_defines.h#L457)
for a running position. Special targets use
[`irsdk_csMode`](../iracing/irsdk-1-20/irsdk_1_20/irsdk_defines.h#L549)
(`incident=-3`, `leader=-2`, `exiting=-1`, `driver=0`). Car numbers are zero-padded via
[`irsdk_padCarNum`](../iracing/irsdk-1-20/irsdk_1_20/irsdk_defines.h#L617). Sample:
[msgtest.cpp](../iracing/irsdk-1-20/irsdk_1_20/irsdk_msgtest/msgtest.cpp#L130).

**Verification:** `CamCarIdx == carIdx` (when targeting a car). Group and camera inputs are
treated as best-effort hints and are surfaced in the observed telemetry, but the stable confirmation
signal is that the requested car is now in focus.
([telemetry_11_23_15.md](../iracing/telemetry_11_23_15.md#L19)).

---

### 6.2 `camera_set_state` (mutating, verified)

Toggle camera tool / UI flags.

**Input**

```jsonc
{
  "type": "object",
  "properties": {
    "camToolActive": { "type": "boolean" },
    "uiHidden": { "type": "boolean" },
    "useAutoShotSelection": { "type": "boolean" },
    "useTemporaryEdits": { "type": "boolean" },
    "useKeyAcceleration": { "type": "boolean" },
    "useKey10xAcceleration": { "type": "boolean" },
    "useMouseAimMode": { "type": "boolean" },
    "waitForConfirmation": { "type": "boolean", "default": true },
    "timeoutMs": { "type": "integer", "minimum": 0, "default": 750 }
  },
  "additionalProperties": false
}
```

**Broadcast:** [`irsdk_BroadcastCamSetState`](../iracing/irsdk-1-20/irsdk_1_20/irsdk_defines.h#L459)
with a composed [`irsdk_CameraState`](../iracing/irsdk-1-20/irsdk_1_20/irsdk_defines.h#L326) bitfield.
Sample: [msgtest.cpp](../iracing/irsdk-1-20/irsdk_1_20/irsdk_msgtest/msgtest.cpp#L187).

**Verification:** `CamCameraState` bitfield reflects the requested bits
([telemetry_11_23_15.md](../iracing/telemetry_11_23_15.md#L20)).

---

## 7. MCP discovery metadata

So clients can discover and use the server, the implementation must advertise the following.

### 7.1 `initialize` result

```jsonc
{
  "protocolVersion": "2025-06-18",
  "serverInfo": { "name": "iracing-mcp", "version": "0.1.0" },
  "capabilities": {
    "tools": { "listChanged": true }   // tool set can change when a session connects/disconnects
  },
  "instructions": "Control and interrogate a live iRacing session. Call session interrogation tools first to ground state; replay/camera commands require the player to be out of the car."
}
```

`listChanged: true` lets the server hide mutating tools (or mark them unavailable) until the SDK is
connected and the player is out of the car.

### 7.2 `tools/list` entry shape

Each tool is advertised as:

```jsonc
{
  "name": "replay_set_playback",
  "title": "Set replay playback",
  "description": "Play, pause, fast-forward, rewind, or slow-mo the replay. Requires out-of-car.",
  "inputSchema": { /* the JSON Schema from this document */ },
  "annotations": {
    "readOnlyHint": false,
    "destructiveHint": false,
    "idempotentHint": true,
    "openWorldHint": true
  }
}
```

Read-only tools set `readOnlyHint: true`. None of the replay/camera tools are `destructive` (they do
not erase data) **except** a future `replay_erase_tape` tool wrapping
[`irsdk_RpyState_EraseTape`](../iracing/irsdk-1-20/irsdk_1_20/irsdk_defines.h#L506), which would set
`destructiveHint: true`.

### 7.3 Client configuration example

```jsonc
// Example MCP client config (stdio transport), run ON the iRacing PC
{
  "mcpServers": {
    "iracing": {
      "command": "iracing-mcp.exe",
      "args": ["--transport", "stdio"],
      "env": { "IRACING_MCP_LOG": "info" }
    }
  }
}
```

See [USER_GUIDE.md](USER_GUIDE.md) for HTTP transport and remote-host setups.

---

## 8. Tool summary table

| Tool | Type | Broadcast / source | Verifies via |
| --- | --- | --- | --- |
| `get_session_overview` | read | live vars + WeekendInfo | n/a |
| `get_weekend_info` | read | YAML `WeekendInfo` | n/a |
| `get_roster` | read | YAML `DriverInfo.Drivers` | n/a |
| `get_camera_groups` | read | YAML `CameraInfo.Groups` | n/a |
| `get_standings` | read | YAML `ResultsPositions` | n/a |
| `get_relatives` | read | live car arrays | n/a |
| `resolve_driver` | read | roster | n/a |
| `replay_get_state` | read | Replay*/Cam* vars | n/a |
| `replay_set_playback` | write | `ReplaySetPlaySpeed` | `ReplayPlaySpeed`, `IsReplayPlaying` |
| `replay_seek_frame` | write | `ReplaySetPlayPosition` | `ReplayFrameNum` |
| `replay_seek_session_time` | write | `ReplaySearchSessionTime` | `ReplaySessionNum`, `ReplaySessionTime` |
| `replay_search_event` | write | `ReplaySearch` | `ReplayFrameNum` |
| `replay_show_window` | write (composite) | pause+seek+focus+play | aggregate |
| `camera_focus` | write | `CamSwitchNum`/`CamSwitchPos` | `CamCarIdx` |
| `camera_set_state` | write | `CamSetState` | `CamCameraState` |
