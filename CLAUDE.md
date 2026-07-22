# GHLG (Ghostlog)

Ghostlog is a free, fully open-source local dev-notes tool. This repo is PUBLIC.

Design principles that still hold:
- Local-first: zero network ports; frontend↔backend is Tauri IPC only,
  browser extension talks over Native Messaging (stdio).
- No cloud calls, no telemetry. AI is bring-your-own local endpoint.
- Path scoping is enforced in Rust, never trusted to the UI.
