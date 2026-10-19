# Contributing knowledge

## Fault code explanations (`crates/opendiag-knowledge/data/dtc-generic.json`)

* Only **generic** codes (P0xxx, P2xxx, U0xxx…) whose meaning is defined by
  SAE J2012 / ISO 15031-6. Write the title in your own words.
* `plain`: one or two sentences a non-mechanic understands.
* `possible_causes`: things to check, most common first. They are shown as
  hypotheses; never write "replace X".
* `suggested_tests`: concrete checks. Values are "general guidance" unless you
  cite documentation.
* `related_measurements`: live data keys (see `crates/opendiag-obd/src/pid.rs`).
* Manufacturer-specific codes need a separate, sourced file per make (planned).

## Guided procedures (`crates/opendiag-knowledge/data/procedures.json`)

* Every measurement step has exactly one of: `spec` (with `kind` and `source`),
  `relative_to` an earlier measurement, or `user_spec` (the user types the value
  from their own manual). Never invent manufacturer specifications.
* Outcomes are `finding` (follows from a measurement against a spec) or
  `hypothesis` (a likely explanation to confirm).
* `applies_to` limits where it is offered (vehicle kinds, codes, systems).
* `cargo test -p opendiag-knowledge` validates every link and spec.

## Vehicle definitions (`definitions/vehicles/`)

Schema: `definitions/schema/vehicle-definition.schema.json`. Check with
`cargo run -p opendiag-cli -- validate-definition your.json --sim sim-car`.
Changing functions (risk ≥ 2) need maintainer/HIL verification, evidence, the
exact ECU ids and a rollback description, or they are rejected.
