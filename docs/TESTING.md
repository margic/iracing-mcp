# iracing-mcp — Test Strategy

This document defines how to validate `iracing-mcp` against a **live iRacing session** using real
broadcast commands and real telemetry responses. Because the SDK is fire-and-forget
([FEEDBACK_VERIFICATION.md](FEEDBACK_VERIFICATION.md)), there is no substitute for asserting on the
actual telemetry the sim produces.

## 1. Test levels

| Level | Scope | Sim required? |
| --- | --- | --- |
| **Unit** | Schema (de)serialization, parameter encoding, verification predicate logic | No |
| **Contract** | MCP `initialize` / `tools/list` shapes, envelope, error taxonomy | No (mock SDK) |
| **Integration (live)** | Real commands against running iRacing; assert real telemetry | **Yes** |

This document focuses on the **integration (live)** level. Unit/contract tests run in CI; live tests
run on the Windows iRacing PC.

## 2. Live test prerequisites

1. Windows PC running iRacing on the same machine as the server.
2. A session loaded with a **recorded replay available** (run a few laps of an AI race, or load an
   existing replay) so there are frames and incidents to seek.
3. **Out of the car** (replay/spectator view) — required for replay & camera control
   ([irsdk_defines.h](../iracing/irsdk-1-20/irsdk_1_20/irsdk_defines.h#L453)).
4. At least **two cars** in the session so camera focus has a distinct target.
5. Server running and connected to an MCP client (see
   [USER_GUIDE.md](USER_GUIDE.md#5-configure-your-mcp-client)).

A test run should first call a **fixture check** to confirm these hold and skip (not fail) if not:

```jsonc
// expected get_session_overview snapshot for a valid fixture
{ "connected": true, "isReplay": true, "isInCar": false, "carCount": ">= 2" }
```

## 3. The agent is the test runner

> The user requirement: *"You are an agent you can call the mcp tool directly to test it when we are
> ready to test."*

Live integration tests are executed by an **agent driving the MCP tools directly**. For each case the
agent:

1. **Arranges** — reads current state via a read tool (snapshot).
2. **Acts** — calls the mutating tool with concrete arguments.
3. **Asserts** — checks `data.verified == true` **and** independently re-reads state with a read tool
   to confirm the observed value, rather than trusting the tool's own verification block alone.

This double-check (tool-reported `verified` **plus** an independent `replay_get_state` /
`get_session_overview` read) is what makes the suite trustworthy.

## 4. Test matrix (live)

Each row is one integration test. "Independent assert" is a second, separate read used to confirm.

| # | Tool under test | Arrange | Act | Assert (`verified`) | Independent assert |
| --- | --- | --- | --- | --- | --- |
| T1 | `get_session_overview` | — | call | `ok==true` | `connected==true` |
| T2 | `get_roster` | — | call | `ok==true` | car count ≥ 2, names non-empty |
| T3 | `get_camera_groups` | — | call | `ok==true` | at least one group with a number |
| T4 | `resolve_driver` | pick a name from T2 | call with that name | `ok==true` | returns matching `carIdx` |
| T5 | `replay_set_playback` (pause) | `replay_get_state` | `{speed:0}` | `verified==true` | `IsReplayPlaying==false` |
| T6 | `replay_set_playback` (play) | T5 | `{speed:1}` | `verified==true` | `ReplayPlaySpeed==1`, `IsReplayPlaying==true` |
| T7 | `replay_set_playback` (slow-mo) | T6 | `{speed:2, slowMotion:true}` | `verified==true` | `ReplayPlaySlowMotion==true` |
| T8 | `replay_seek_frame` | read `ReplayFrameNum` = F | `{frame: F+600}` | `verified==true` | `|ReplayFrameNum-(F+600)| <= 4` |
| T9 | `replay_seek_session_time` | read session time | `{sessionNum, sessionTimeMs}` | `verified==true` | `|ReplaySessionTime*1000 - target| <= 500` |
| T10 | `replay_search_event` (prev incident) | seek mid-replay | `{mode:"prev_incident"}` | `verified==true` *or* warned at-bounds | `ReplayFrameNum` changed in expected direction |
| T11 | `camera_focus` | pick `carIdx` from T4 | `{carIdx}` | `verified==true` | `CamCarIdx==carIdx` |
| T12 | `camera_focus` + group | T11 + a group from T3 | `{carIdx, groupNumber}` | `verified==true` | `CamGroupNumber==groupNumber` |
| T13 | `camera_set_state` | `replay_get_state` | toggle a state bit | `verified==true` | `CamCameraState` bit matches |
| T14 | `replay_show_window` (composite) | resolve + time | full headline sequence | `verified==true` | frame near target **and** `CamCarIdx` matches |

Telemetry variables referenced above are documented in
[telemetry_11_23_15.md](../iracing/telemetry_11_23_15.md) — replay vars near
[L110](../iracing/telemetry_11_23_15.md#L110), camera vars near
[L19](../iracing/telemetry_11_23_15.md#L19).

## 5. Negative / edge cases

| # | Scenario | Setup | Expected |
| --- | --- | --- | --- |
| N1 | In-car guard | Get in the car, then call `camera_focus` | `error.code == wrong_mode`, `retryable==false` |
| N2 | Not connected | Close iRacing, call `get_session_overview` | `error.code == not_connected` |
| N3 | Unknown driver | `resolve_driver{ name:"Nobody XYZ" }` | `error.code == target_not_found` |
| N4 | Seek past end | `replay_seek_frame{ frame: frameNumEnd + 100000 }` | clamped + `verified==true` near end, or `not_verified` with reason |
| N5 | At-bounds search | At first frame, `replay_search_event{ mode:"prev_incident" }` | `verified==false`, warning "nothing earlier" |
| N6 | Invalid arg | `replay_set_playback{ speed: 999999 }` (out of range) | `error.code == invalid_argument` |

## 6. Assertions & tolerances

- **Frames:** ±4 frames (~66 ms at 60 fps), per
  [FEEDBACK_VERIFICATION.md](FEEDBACK_VERIFICATION.md#tolerances--timing-guidance).
- **Session time:** ±500 ms.
- **Booleans/ints** (`IsReplayPlaying`, `CamCarIdx`, `ReplayPlaySpeed`): exact match.
- **Timeouts:** use each tool's default; a test may pass a larger `timeoutMs` for flaky-timing
  retries but must record it.

## 7. Test ordering & isolation

1. Run **read-only** tests (T1–T4) first; abort the suite if the fixture is invalid.
2. Run **playback** tests (T5–T10), then **camera** tests (T11–T13), then the **composite** (T14).
3. **Teardown:** after the suite, restore a neutral state — `replay_set_playback{ speed: 0 }` (pause)
   and optionally reset the camera to the original `CamCarIdx`/group captured at the start. No
   destructive tools (e.g. erase-tape) are exercised by this suite.

## 8. Running the live suite (agent-driven)

When the sim is ready, instruct the agent:

> "Run the iracing-mcp live integration suite. For each test in TESTING.md §4 and §5: arrange,
> act, and assert using a separate read tool. Report a table of pass/fail with the observed
> telemetry values, then restore neutral state."

The agent should:

1. Validate the fixture (§2) and skip the suite with a clear message if invalid.
2. Execute T1–T14 and N1–N6 in the order of §7.
3. Produce a results table: test id, tool, args, `verified`, independent-assert value, pass/fail.
4. Perform teardown (§7.3) and report.

## 9. CI vs manual

| Suite | Where | Trigger |
| --- | --- | --- |
| Unit + contract | CI (Linux/Windows) | every push |
| Live integration | Windows iRacing PC, agent-driven | manual, before a broadcast |

Live tests are intentionally **not** in CI because they require a running sim and a human-curated
replay fixture. They are the final gate before relying on the server on air.

## 10. Definition of done for a release

- All unit + contract tests green in CI.
- Live suite (§4) all `verified==true` with independent asserts matching.
- Negative cases (§5) return the correct typed errors.
- Teardown leaves the sim paused and the camera restored.
