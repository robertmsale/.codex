# Linux Support

Linux is a first-class target for the core Robdex bridge, CLI, orchestration,
and Requirements workflow. macOS remains the primary local development target,
but the public bootstrap path should work on a normal Linux developer machine.

## Expected Prerequisites

- `bash` and `zsh`
- `curl`
- `python3`
- `rustup`, `cargo`, and `rustc`
- `codex` on `PATH`, or `CODEX_BIN` pointing at a compatible binary
- optional: `systemctl --user` for service-managed start/stop

## Paths

Use the same public variables as macOS:

- `ROBDEX_HOME`
- `CODEX_HOME`
- `ROBDEX_STATE_HOME`
- `ROBDEX_BRIDGE_BASE_URL`
- `ROBDEX_BRIDGE_APP_SERVER_URL`

No core bootstrap script should require `/Users/robertsale` or another
developer-specific absolute path.

## Services

For Linux user services, set:

```sh
export ROBDEX_SERVICE_MANAGER=systemd
export ROBDEX_APP_SERVER_UNIT=robdex-app-server.service
export ROBDEX_BRIDGE_UNIT=robdex-bridge.service
```

Then use:

```sh
robdex start
robdex status
robdex stop
```

Systemd unit generation is still staged work. Until generated templates exist,
configure `supervisor` explicitly or start the binaries by hand for debugging
outside `robdex-service`. `robdex-service` intentionally refuses unmanaged
pid-file fallback processes.

## Shell Behavior

The core bootstrap path assumes POSIX-style paths and shells. The existing zsh
wrapper is expected to work on Linux when `zsh` is installed. Privileged-exec
policy must be configured deliberately for the host; do not copy Robert's live
policy blindly.

## GUI

The Flutter desktop GUI has been reported to compile and run on Ubuntu. The
public support position is:

- core bridge/CLI/orchestration: intended Linux target;
- Flutter Linux GUI: feasible, but validate per host;
- drag-and-drop images and other desktop integrations: unverified unless a
  focused Linux GUI test covers them.

## Not Supported

Windows is not supported yet. A real Windows design must cover shell behavior,
path handling, service management, PTY behavior, and privileged execution.
