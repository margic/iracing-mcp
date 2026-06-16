# iracing-mcp — User Guide

This guide explains how to install, configure, and use the `iracing-mcp` server with an LLM agent to
interrogate and direct a live iRacing session.

## 1. Concept

`iracing-mcp` is an [MCP](https://modelcontextprotocol.io) server. An MCP-capable agent (your "sim
racecenter" agent) connects to it and calls tools to:

- **Interrogate** the session — roster, weekend info, camera groups, standings, overview.
- **Direct** the replay and cameras — play/seek/jump and focus cameras.

The agent handles natural language; the server provides deterministic, **verified** actions. For the
full tool list see [TOOL_REFERENCE.md](TOOL_REFERENCE.md).

## 2. Requirements

| Requirement | Detail |
| --- | --- |
| OS | **Windows** (same machine as iRacing) |
| iRacing | Installed and running; SDK telemetry enabled |
| The server | `iracing-mcp.exe` (built from this repo) |
| An MCP client | Any MCP-capable agent host / IDE |

> **Why same machine?** The iRacing SDK uses a local shared-memory map and a local broadcast Windows
> message (`IRSDK_BROADCASTMSG`). The server must run on the sim PC. See
> [irsdk_defines.h](../iracing/irsdk-1-20/irsdk_1_20/irsdk_defines.h#L85) and
> [irsdk_utils.cpp](../iracing/irsdk-1-20/irsdk_1_20/irsdk_utils.cpp#L347).

## 3. Build

This repo's dev container is configured for Rust cross-compiling to Windows
(`x86_64-pc-windows-gnu`; see [.devcontainer/devcontainer.json](../.devcontainer/devcontainer.json)).

```bash
# from the repo root
cargo build --release --target x86_64-pc-windows-gnu
```

The resulting `iracing-mcp.exe` must be copied to and run on the Windows iRacing PC.

> If you build natively on Windows instead, a plain `cargo build --release` produces the same binary.

## 4. Enable iRacing telemetry

1. Launch iRacing and enter a session (practice, race, or a replay).
2. Telemetry/live data is exposed automatically to the SDK while the sim is running.
3. For **replay/camera control**, make sure you are **out of the car** (in the replay/spectator
   view). This is an iRacing SDK constraint, not a server limitation
   ([irsdk_defines.h](../iracing/irsdk-1-20/irsdk_1_20/irsdk_defines.h#L453)).

## 5. Configure your MCP client

### 5.1 stdio (recommended, local)

Run the server as a child process of the agent host on the iRacing PC:

```jsonc
{
  "mcpServers": {
    "iracing": {
      "command": "C:\\tools\\iracing-mcp\\iracing-mcp.exe",
      "args": ["--transport", "stdio"],
      "env": { "IRACING_MCP_LOG": "info" }
    }
  }
}
```

### 5.2 HTTP (agent on another machine)

If your agent runs on a different machine on the LAN, start the server with the HTTP transport on the
iRacing PC:

```powershell
iracing-mcp.exe --transport http --bind 0.0.0.0:8765
```

Then point the client at it:

```jsonc
{
  "mcpServers": {
    "iracing": { "url": "http://<sim-pc-ip>:8765/mcp" }
  }
}
```

> The **server binary still runs on the sim PC** — only the agent is remote. Restrict the bind
> address / firewall to trusted hosts on your LAN.

## 6. Verify the connection

Once configured, your client should discover the server and list its tools (via `tools/list`). A
quick smoke test from the agent:

1. Call `get_session_overview`. You should get `connected: true` and the current track/session.
2. Call `replay_get_state`. You should see live `frameNum`, `playSpeed`, and camera info.

If `get_session_overview` returns `error.code: not_connected`, confirm iRacing is running and the
server is on the same PC.

## 7. Using the agent

You drive the agent in natural language; it calls the tools. Example director commands and what the
agent does under the hood:

| You say | Agent calls (in order) |
| --- | --- |
| "Who's in the race?" | `get_roster` |
| "Where are we racing and what's the weather?" | `get_weekend_info` |
| "What camera angles do we have?" | `get_camera_groups` |
| "Show the standings." | `get_standings` |
| "Focus on Max." | `resolve_driver` → `camera_focus` |
| "Play the replay at normal speed." | `replay_set_playback{ speed: 1 }` |
| "Pause." | `replay_set_playback{ speed: 0 }` |
| "Jump to the previous incident." | `replay_search_event{ mode: "prev_incident" }` |
| "Show the replay of the incident at 14:32 involving Max." | `resolve_driver` → `replay_seek_session_time` → `replay_search_event` → `camera_focus` → `replay_show_window` |

The last row is the headline use case; the full sequence diagram is in
[IMPLEMENTATION_PLAN.md](IMPLEMENTATION_PLAN.md#6-example-use-case-end-to-end).

## 8. Understanding tool responses

Every tool returns a common envelope (`ok`, `asOfUtc`, `data`, `warnings`, `error`). **Mutating**
tools (replay/camera) also return a verification block:

```jsonc
{
  "commandAccepted": true,   // message was sent
  "verified": true,          // telemetry confirmed it worked — trust this
  "observed": { "camCarIdx": 7 }
}
```

If `verified` is `false`, the action did not take effect (e.g. nothing earlier to jump to, or the
player is in the car). See [FEEDBACK_VERIFICATION.md](FEEDBACK_VERIFICATION.md) for the full model.

## 9. Troubleshooting

| Symptom | Likely cause | Fix |
| --- | --- | --- |
| `not_connected` | iRacing not running / server not on sim PC | Start iRacing; run server locally |
| `wrong_mode` | You are in the car | Exit to replay/spectator view ([SDK note](../iracing/irsdk-1-20/irsdk_1_20/irsdk_defines.h#L453)) |
| `verified: false` on seek | Target out of range | Try a different time/frame; check `frameNumEnd` |
| `target_not_found` | Name didn't resolve | Re-ask with car number or fuller name |
| Camera doesn't move | Group/camera not legal | Call `get_camera_groups` first |
| Tools missing from list | Server hides write tools until eligible | Connect a session and exit the car (`listChanged`) |

## 10. Safety notes

- The server only issues **replay and camera** broadcast messages by default. It does not send pit or
  chat commands.
- A potential future `replay_erase_tape` tool wraps
  [`irsdk_RpyState_EraseTape`](../iracing/irsdk-1-20/irsdk_1_20/irsdk_defines.h#L506) and is the only
  destructive action; it would be clearly marked and require explicit confirmation.

## 11. Next steps

- Author your Agent Skills following the [agentskills.io specification](https://agentskills.io/specification),
  mapping director phrases to the tool sequences above.
- Read [TESTING.md](TESTING.md) to validate the server against a live session before going on air.
