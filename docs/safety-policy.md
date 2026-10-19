# Safety policy

OpenDiag assumes that a wrong diagnostic or configuration operation can damage
a vehicle, corrupt an ECU, immobilise the vehicle, erase diagnostic data, make
the vehicle unsafe or unlawful, or cost a lot to repair. Safety is enforced in
code (`crates/opendiag-safety`), not only in text.

## Risk levels

| Level | Meaning | Status in OpenDiag |
| --- | --- | --- |
| 0 | Read only | Available. |
| 1 | Standard maintenance: clear supported codes, documented service resets | Clearing emissions codes available behind checks; service resets planned. |
| 2 | Verified personalization for an exact ECU revision | Subsystem exists, off by default; no verified operations yet. |
| 3 | Advanced service: calibrations, adaptations, actuator tests, ECU reset | As level 2, and a verified rollback is required. |
| 4 | Programming, security access, immobiliser, safety-critical changes | Refused in code. Needs a separate, professionally validated and legally reviewed implementation. |

## How it is enforced

1. **Every request is classified** by service id (`classify`). Unknown services
   count as level 4.
2. **Drivers check every request** with `may_send`. Read-only requests pass.
   Anything else needs an `Authorization` for exactly those bytes.
3. **Only the policy engine can make an `Authorization`** (private fields).
   Plugins, the expert console and definitions cannot create one.
4. **The expert console accepts read-only services only**, in expert mode too.
5. **Definitions are data.** Validation rejects level 4 functions, requests
   whose real risk is higher than declared, and changing functions without
   maintainer/test-bench verification, evidence, exact ECU ids and a rollback.

## Clearing fault codes

* Never automatic, never at the end of a session.
* First the codes, freeze frame and readiness are read and saved (snapshot).
* The user sees the codes that will be erased, the permanent codes that stay,
  the modules the request reaches, and what clearing does (not a repair; loss
  of freeze frame; readiness reset; possible effect on inspections).
* Three explicit confirmations, then a plan that expires after 5 minutes.
* Conditions are read again when the user confirms: risk notice accepted,
  snapshot present, engine speed 0 if readable, battery ≥ 11.0 V if readable,
  no recent communication errors. Any failure blocks the request; nothing is sent.
* Afterwards the codes are read back and the result is logged with the session.

## Configuration changes (when they exist)

For every change: check vehicle + exact ECU identity against a verified
definition; check adapter capability and link reliability; check ignition and
battery (≥ 12.2 V, battery support recommended); explain consequences; read and
save the original value; require a verified rollback for level 3; preview;
specific confirmation; send only the validated request; read back to verify;
record the result. If any of this cannot be established, the change is blocked.

OpenDiag never implements generic memory writing, unverified flashing, or
bypasses of security gateways, immobilisers or emissions controls.
