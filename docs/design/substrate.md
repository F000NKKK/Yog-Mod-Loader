# Shared infrastructure and the mod API

Substrate supplies the generic infrastructure used by Yog's protected engine.
Yog keeps Minecraft, mod identities, the native ABI and debugger request handling
in its own adapters.

| Concern | Implementation | Yog consumers |
|---|---|---|
| Data model, schemas, text, binary and frames | `substrate-data` | `yog-data` adds Yog's domain vocabulary |
| Poison-tolerant blocking and nonblocking locks | `substrate-lock` | `yog-runtime`, `yog-hot-reload`, `yog-symbols`, `yog-debugger` |
| DAP JSON transport | `substrate-framing` | `yog-debugger`; request dispatch stays in Yog |
| External compiler environment | `substrate-process` | `yog-script-build` starts `javac` outside the AppImage environment |

The engine uses standard `Mutex` and `RwLock` values with Substrate's `Locked`
and `ReadWrite` traits. `try_locked` returns `None` when busy and recovers the
guard after a panic, matching the runtime's nonblocking tick behavior.

Substrate Platform is available under AGPL-3.0-only or a separate commercial
agreement. Its dependencies stay in Yog's AGPL engine and compiler crates.
`yog-api`, `yog-data` and the mod-facing crates remain MIT OR Apache-2.0.
Substrate Data 0.4.0 is also MIT OR Apache-2.0. `yog-sync` remains the permissive
lock facade for those crates; they do not depend on `substrate-lock`.

`yog-debugger` 0.19.0 uses `substrate_framing::{read_frame, write_frame}` instead
of defining `dap::read_message` and `dap::write_message`. External transports
should call Substrate directly; `dap::Server` continues dispatching Yog requests.
