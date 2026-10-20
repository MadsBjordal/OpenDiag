// Run: npm test   (Node 22.18+ runs TypeScript directly)
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { codeSummary, compatSnapshot, validate, type Share } from '../src/logic.ts';

const share = (over: Partial<Share> = {}): Share => ({
  schema: 'opendiag-share/1',
  app_version: '0.1.0',
  os: 'windows',
  vehicle: { make: 'Volkswagen', model: 'Golf', year: 2015, wmi: 'WVW', kind: 'car' },
  adapter: { identity: 'ELM327 v1.4b', firmware: null, family: 'elm327', clone_suspected: false, link: 'usb_serial' },
  protocol: 'ISO 15765-4 CAN (11 bit, 500 kbaud)',
  ecus: [{ label: 'Engine', calibration_id: 'CAL123' }],
  results: [{ function: 'read_dtcs', result: 'works' }],
  dtcs: [{ code: 'P0420', kind: 'stored', ecu: 'Engine' }],
  freeze_frames: [{ dtc: 'P0420', values: [{ key: 'rpm', value: 812, unit: 'rpm' }] }],
  mil_on: true,
  monitors: [{ name: 'Catalyst', complete: false }],
  modified: false,
  ...over,
});

test('accepts exactly what the app sends', () => {
  assert.equal(validate(share()).ok, true);
});

test('refuses extra fields, full VINs and odd values', () => {
  assert.equal(validate({ ...share(), vin: 'WVWZZZ1KZAW000001' }).ok, false);
  assert.equal(validate(share({ vehicle: { ...share().vehicle, wmi: 'WVWZZZ1KZAW000001' } })).ok, false);
  assert.equal(validate(share({ dtcs: [{ code: 'DROP TABLE', kind: 'stored', ecu: 'x' }] })).ok, false);
  assert.equal(validate(share({ results: [{ function: 'read_dtcs', result: 'great' as never }] })).ok, false);
  assert.equal(validate(share({ adapter: { ...share().adapter, identity: '<script>' } })).ok, false);
  assert.equal(validate(null).ok, false);
});

test('publishes a combination only after two separate reports', async () => {
  let snap = await compatSnapshot([share()], 'now', 's1');
  assert.equal(snap.records.length, 0);
  snap = await compatSnapshot([share(), share({ results: [{ function: 'read_dtcs', result: 'partial' }] }), share()], 'now', 's1');
  assert.equal(snap.format, 'opendiag-compat-snapshot/1');
  assert.equal(snap.records.length, 1);
  const r = snap.records[0];
  assert.equal(r.result, 'works');
  assert.equal(r.verification, 'community_reported');
  assert.match(r.notes, /3 community reports: 2 works, 1 partial/);
});

test('fault codes are counted per model, once per share', () => {
  const twice = share({ dtcs: [{ code: 'P0420', kind: 'stored', ecu: 'Engine' }, { code: 'P0420', kind: 'pending', ecu: 'Engine' }] });
  assert.deepEqual(codeSummary([twice]), []);
  const c = codeSummary([twice, share(), share({ vehicle: { ...share().vehicle, model: 'Polo' } })]);
  assert.deepEqual(c, [{ make: 'Volkswagen', model: 'Golf', year: 2015, code: 'P0420', reports: 2, modified: false }]);
});

test('modified vehicles are kept apart from standard ones', async () => {
  const mod = share({ modified: true });
  // One standard + one modified report: neither group reaches two.
  assert.deepEqual(codeSummary([share(), mod]), []);
  assert.equal((await compatSnapshot([share(), mod], 'now', 's')).records.length, 0);
  const c = codeSummary([mod, mod]);
  assert.equal(c[0].modified, true);
  const snap = await compatSnapshot([mod, mod], 'now', 's');
  assert.equal(snap.records[0].model, 'Golf (modified)');
});

test('shares from older app builds without the modified answer count as standard', () => {
  const { modified, ...old } = share();
  const r = validate(old);
  assert.equal(r.ok && r.share.modified, false);
  assert.equal(validate({ ...share(), modified: 'yes' }).ok, false);
});
