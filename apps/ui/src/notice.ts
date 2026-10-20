// The open-source notice and risk disclosure. Must be reviewed by qualified
// legal counsel before a public release (see docs/legal.md).

export const NOTICE_TITLE = 'Open-source software notice and risk disclosure';

export const NOTICE: string[] = [
  'OpenDiag is free, open-source software (GNU GPL version 3 or later). It is provided without warranty, to the extent permitted by applicable law. Nothing here limits rights you have under consumer protection law that cannot be excluded.',
  'Compatibility is not guaranteed. A function is only marked as verified when it has been tested with that combination of vehicle, ECU, adapter and operating system. Many adapters, especially inexpensive clones, support only part of what they claim.',
  'Reading diagnostic information is generally low risk. Functions that change the vehicle, even clearing fault codes, carry risk: loss of stored diagnostic data, failed emissions inspections, or, for advanced operations, a vehicle that does not work properly.',
  'Only diagnose or change vehicles you own or have the owner\'s permission to work on. You are responsible for choosing suitable procedures and compatible hardware, and for following the law where you are, including rules on roadworthiness and emissions systems.',
  'Some operations must be done by a qualified professional. OpenDiag does not offer ECU programming, immobiliser or security functions, and never bypasses manufacturer security or emissions controls.',
  'Fault code explanations and possible causes are general guidance, not a diagnosis. Confirm with tests before replacing parts.',
];

export const PRIVACY: string[] = [
  'Everything is stored on this device only. No account is needed.',
  'Nothing is uploaded automatically: not your VIN, not your reports, not your location.',
  'The app only asks GitHub whether a newer version exists (no data about you or your vehicle is sent). You can turn this off under Updates.',
  'Sharing results is optional: you are asked once, you see exactly what is sent each time, and you can delete what you shared under Sharing.',
  'Exports mask the VIN unless you choose to include it.',
  'You can delete all vehicle data in Settings at any time.',
];
