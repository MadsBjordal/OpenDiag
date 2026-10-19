# Contributing to OpenDiag

Thank you! The most valuable contribution right now is **testing on real
hardware** ([docs/hardware-testing.md](docs/hardware-testing.md)).

## Ground rules

* **Do not fabricate.** No invented ECU addresses, data identifiers, specifications,
  test results or compatibility claims. Every definition, explanation and procedure
  names its source.
* **No proprietary data.** Never commit service manuals, manufacturer databases,
  ODX/PDX/CDD files, ECU dumps or text from paid services without a licence that
  allows redistribution.
* **No bypasses.** Nothing that circumvents security gateways, immobilisers,
  emissions controls or the safety policy will be accepted.
* **Honest status.** New functions start as "Implemented but not hardware-verified".
  Only maintainers change a status to "Implemented and tested", with evidence.

## Developer Certificate of Origin

Sign off every commit (`git commit -s`), certifying the
[DCO](https://developercertificate.org/): you wrote it or have the right to
contribute it under the project's licences (code GPL-3.0-or-later, data CC0-1.0,
knowledge texts CC-BY-SA-4.0).

## Code

* `cargo test`, `cargo clippy --all-targets`, `cargo fmt --check`,
  `pnpm --filter @opendiag/ui typecheck`, `pnpm --filter @opendiag/ui test` must pass.
* Protocol code needs tests with real or standard-derived byte sequences.
  New engine workflows need a simulator test.
* Every visible text in the UI goes through `t()` (apps/ui/src/i18n.ts).
* Anything that can send a non-read request must use `opendiag-safety`; reviewers
  reject code that works around it.

### Review requirements

* One maintainer review for docs, UI and read-only code.
* Two maintainer reviews for: drivers, protocol parsing, the safety policy,
  anything in the vehicle configuration subsystem, definition validation.

## Supported-function approval process

1. Implementation merged with automated tests → status *Implemented but not hardware-verified*.
2. Compatibility reports from testers (issue template + exported snapshot).
3. A maintainer reproduces it (or reviews hardware-in-the-loop evidence) on that
   platform → row added to [docs/compatibility.md](docs/compatibility.md) for that OS.
4. Only then may the status for that platform become *Implemented and tested*.

Experimental features stay behind expert mode and are labelled "experimental";
they are never presented as verified.

## Vehicle definitions, fault codes, procedures

See [docs/plugin-sdk.md](docs/plugin-sdk.md) and
[docs/contributing-knowledge.md](docs/contributing-knowledge.md).
