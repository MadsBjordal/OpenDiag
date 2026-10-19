# Extending OpenDiag

There are two kinds of extensions. **Data** is available now; **executable
plugins** are designed but not built yet (Phase 4).

## 1. Data contributions (available now)

Data cannot run anything. The engine still passes every request through the
safety policy.

| Kind | Where | Format | Check |
| --- | --- | --- | --- |
| Vehicle definitions | `definitions/vehicles/*.json` | [schema](../definitions/schema/vehicle-definition.schema.json) | `opendiag validate-definition file.json [--sim sim-car]` |
| Fault code explanations | `crates/opendiag-knowledge/data/dtc-generic.json` | see existing entries | `cargo test -p opendiag-knowledge` |
| Guided procedures | `crates/opendiag-knowledge/data/procedures.json` | see existing entries | `cargo test -p opendiag-knowledge` |
| Compatibility results | the app (Compatibility → Submit) or a snapshot file | `opendiag-compat-snapshot/1` | moderation |

`validate-definition --sim` sends each **read-only** function of the definition
to a simulated ECU and checks for a positive answer. Functions with risk level
≥ 1 are never run by this tool.

## 2. Executable plugins (design)

Plugin kinds: hardware drivers, protocol implementations, live-data dashboards,
export formats, and (only after review) verified personalization workflows.

* **Isolation.** Plugins are WebAssembly modules run by the engine in a
  sandbox (e.g. wasmtime) with no ambient access: no filesystem, network,
  credentials or OS APIs unless a permission is granted.
* **Permissions** are declared in the plugin manifest and shown to the user:
  `link:raw` (a driver gets the bytes of a link the user picked),
  `vehicle:read` (read-only requests, still classified by the policy),
  `export:write` (write one file the user chose). There is **no** permission
  that allows vehicle-changing requests: those always go through the
  configuration subsystem, verified definitions and the policy.
* **Interface.** Plugins exchange JSON messages with the host
  (`describe`, `open`, `send`, `receive`, `close` for drivers;
  `render(samples)` for dashboards; `export(report)` for formats). The
  interface crate will be Apache-2.0 so plugin authors choose their licence.
* **Trust.** Releases are signed; the publisher identity is shown; plugins
  from the official index are reviewed; reproducible builds where practical.
* **Review workflow.** Pull request with the manifest, source, tests against the
  simulator and, for drivers, recorded traces; two maintainer reviews; plugins
  that touch the configuration subsystem additionally need hardware-in-the-loop
  evidence.
