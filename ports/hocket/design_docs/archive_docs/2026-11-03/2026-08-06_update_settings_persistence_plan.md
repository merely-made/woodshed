# Hocket Update Settings Persistence

**Date**: 2026-08-06  
**Status**: landed

`UpdateSettings` is device-local startup policy. It is stored in
`update-settings.json` and exposed through `genet_host_api::settings` at the
`pelt/update` reference. The provider owns atomic temporary-file plus rename
writes; the shared contract owns only the typed description and axes.

`HOCKET_SETTINGS` is an explicit isolated-file override. The old
`HOCKET_UPDATE_POLICY` environment variable is no longer read. The update-now
CLI and normal startup load the same provider-backed file, so they cannot
silently disagree about policy.

The provider exposes policy, channel, and check interval. All are device-local,
ordinary, and startup-only because the update worker receives its settings at
startup. A later live settings surface can use the same provider and restart
the worker as a separate decision.

Receipt: `cargo test -p hocket-genet update::tests::device_provider_describes_and_persists_update_settings -- --nocapture`.
