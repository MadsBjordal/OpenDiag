# Licensing

## Recommendation: GPL-3.0-or-later for the code

| Licence | For OpenDiag | Against |
| --- | --- | --- |
| **GPL-3.0** | Keeps every improved version open, including forks sold with adapters; patent grant; anti-tivoization (users can install their own builds on devices). | Proprietary in-process plugins are not allowed; incompatible with Apple App Store terms; some companies avoid it. |
| **LGPL-3.0** | Allows proprietary programs to link the engine as a library. | Weaker protection of the app; the mobile/static-linking rules are hard to follow correctly. |
| **Apache-2.0** | Maximum adoption, patent grant, easy for companies. | Anyone can ship closed versions (e.g. a locked-down "pro" fork) without giving back. |

The project's goal is a community ecosystem without vendor lock-in, so the
protection of GPL-3.0 outweighs the adoption advantage of Apache-2.0.

**Plugins.** Data-only contributions (vehicle definitions, fault code texts,
procedures, compatibility records) are data, not code linked to the program.
Executable plugins (Phase 4) will run in a sandbox (WebAssembly) and talk to the
host through a documented message interface. Whether a plugin is a "derivative
work" depends on that coupling; the plugin SDK and its interface definitions
will be released under **Apache-2.0** so that plugin authors can choose their
own licence, and the plugin documentation will say so explicitly. Have counsel
confirm this before Phase 4.

## Data

* Vehicle definitions and compatibility records: **CC0-1.0** (facts; maximum reuse).
* Knowledge texts (fault code explanations, procedures): contributed under
  **CC-BY-SA-4.0** unless they say otherwise in their `source`.

## Contributions

* **Developer Certificate of Origin** (`Signed-off-by:` on every commit) instead
  of a CLA: contributors keep their copyright and certify they have the right
  to contribute. No relicensing is possible without contributors' agreement,
  which is intended.
* Never contribute manufacturer manuals, databases, ODX/PDX/CDD files, ECU
  dumps or text copied from paid services.

## Dependency checks

`deny.toml` lists licences allowed for Rust dependencies (permissive and
GPL-compatible) and bans the rest. CI runs `cargo deny check licenses`. JavaScript
dependencies are few (React, Tauri API, Vite) and all MIT/Apache-2.0.
