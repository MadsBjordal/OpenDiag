# Security policy

OpenDiag talks to vehicles, so security bugs can have physical consequences.

## Reporting

Please **do not open a public issue** for:

* a way to send a vehicle-changing request without the safety policy's authorization,
* a malicious definition, snapshot or file that runs code, writes files or
  bypasses validation,
* memory-safety or parsing bugs triggered by adapter or vehicle data,
* anything that leaks VINs or reports off the device.

Use GitHub's private vulnerability reporting ("Report a vulnerability" on the
Security tab of the repository). Include steps, affected version and platform.
We aim to acknowledge within 7 days and to agree a disclosure date with you
(normally within 90 days).

Vulnerabilities **in vehicles** found while using OpenDiag should be reported to
the manufacturer's product security team, not exploited or published here.

## Supported versions

Only the latest release receives fixes during the pre-1.0 phase.
