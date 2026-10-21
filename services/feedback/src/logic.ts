// Pure rules of the service: what a shared result may contain, and how the
// published summaries are combined. No I/O here, so it is tested on its own.

export const SHARE_SCHEMA = 'opendiag-share/1';
export const SNAPSHOT_FORMAT = 'opendiag-compat-snapshot/1';
/** A combination or code is published only once this many separate shares report it. */
export const MIN_REPORTS = 2;
export const MAX_BODY = 32 * 1024;

const RESULTS = ['works', 'partial', 'fails', 'not_supported'] as const;
const DTC_KINDS = ['stored', 'pending', 'permanent'] as const;
type Result = (typeof RESULTS)[number];

export interface Share {
  schema: string;
  app_version: string;
  os: string;
  vehicle: { make: string; model: string; year: number | null; wmi: string | null; kind: string | null };
  adapter: { identity: string; firmware: string | null; family: string; clone_suspected: boolean; link: string };
  protocol: string | null;
  ecus: { label: string; calibration_id: string | null }[];
  results: { function: string; result: Result }[];
  dtcs: { code: string; kind: string; ecu: string }[];
  freeze_frames: { dtc: string | null; values: { key: string; value: number; unit: string }[] }[];
  mil_on: boolean | null;
  monitors: { name: string; complete: boolean }[];
  /** The user said the vehicle is modified (tune, ECU, engine). Missing in shares from Test build 2 = not modified. */
  modified?: boolean;
}

type Check = { ok: true; share: Share } | { ok: false; error: string };

const isObj = (v: unknown): v is Record<string, unknown> => typeof v === 'object' && v !== null && !Array.isArray(v);
const text = (v: unknown, max: number, pattern = /^[\p{L}\p{N} .,:;()/+_#&'-]*$/u): v is string => typeof v === 'string' && v.length <= max && pattern.test(v);
const optText = (v: unknown, max: number, pattern?: RegExp) => v === null || text(v, max, pattern);
const onlyKeys = (o: Record<string, unknown>, keys: string[]) => Object.keys(o).every((k) => keys.includes(k)) && keys.every((k) => k in o);
const list = (v: unknown, max: number): v is unknown[] => Array.isArray(v) && v.length <= max;

/** Accepts exactly the fields the app sends (crates/opendiag-engine/src/share.rs) and nothing else. */
export function validate(body: unknown): Check {
  const bad = (error: string): Check => ({ ok: false, error });
  if (!isObj(body)) return bad('not an object');
  const keys = ['schema', 'app_version', 'os', 'vehicle', 'adapter', 'protocol', 'ecus', 'results', 'dtcs', 'freeze_frames', 'mil_on', 'monitors'];
  const { modified, ...rest } = body;
  if (!onlyKeys(rest, keys)) return bad('unexpected or missing fields');
  if (!(modified === undefined || typeof modified === 'boolean')) return bad('modified');
  const b = body;
  if (b.schema !== SHARE_SCHEMA) return bad('unknown schema');
  if (!text(b.app_version, 20, /^[0-9A-Za-z.+-]+$/) || !['windows', 'android', 'linux', 'macos', 'ios'].includes(b.os as string)) return bad('app version or os');
  const v = b.vehicle;
  if (!isObj(v) || !onlyKeys(v, ['make', 'model', 'year', 'wmi', 'kind'])) return bad('vehicle');
  if (!text(v.make, 40) || !text(v.model, 40) || !optText(v.wmi, 3, /^[A-HJ-NPR-Z0-9]{3}$/) || !optText(v.kind, 20)) return bad('vehicle fields');
  if (!(v.year === null || (Number.isInteger(v.year) && (v.year as number) >= 1980 && (v.year as number) <= 2100))) return bad('vehicle year');
  const a = b.adapter;
  if (!isObj(a) || !onlyKeys(a, ['identity', 'firmware', 'family', 'clone_suspected', 'link'])) return bad('adapter');
  if (!text(a.identity, 60) || !optText(a.firmware, 60) || !text(a.family, 20) || typeof a.clone_suspected !== 'boolean' || !text(a.link, 30)) return bad('adapter fields');
  if (!optText(b.protocol, 60)) return bad('protocol');
  if (!list(b.ecus, 32) || !b.ecus.every((e) => isObj(e) && onlyKeys(e, ['label', 'calibration_id']) && text(e.label, 60) && optText(e.calibration_id, 20))) return bad('ecus');
  if (!list(b.results, 40) || !b.results.every((r) => isObj(r) && onlyKeys(r, ['function', 'result']) && text(r.function, 40, /^[a-z0-9_]+$/) && RESULTS.includes(r.result as Result)))
    return bad('results');
  if (!list(b.dtcs, 100) || !b.dtcs.every((d) => isObj(d) && onlyKeys(d, ['code', 'kind', 'ecu']) && text(d.code, 8, /^[PCBU][0-9A-F]{4}$/) && DTC_KINDS.includes(d.kind as never) && text(d.ecu, 60)))
    return bad('dtcs');
  const val = (x: unknown) => isObj(x) && onlyKeys(x, ['key', 'value', 'unit']) && text(x.key, 40, /^[a-z0-9_]+$/) && typeof x.value === 'number' && Number.isFinite(x.value) && text(x.unit, 12, /^[\p{L}\p{N} %/°.-]*$/u);
  if (!list(b.freeze_frames, 16) || !b.freeze_frames.every((f) => isObj(f) && onlyKeys(f, ['dtc', 'values']) && optText(f.dtc, 8, /^[PCBU][0-9A-F]{4}$/) && list(f.values, 60) && f.values.every(val)))
    return bad('freeze frames');
  if (!(b.mil_on === null || typeof b.mil_on === 'boolean')) return bad('mil');
  if (!list(b.monitors, 20) || !b.monitors.every((m) => isObj(m) && onlyKeys(m, ['name', 'complete']) && text(m.name, 40) && typeof m.complete === 'boolean')) return bad('monitors');
  return { ok: true, share: { ...(b as unknown as Share), modified: modified === true } };
}

const norm = (s: string) => s.trim().toLowerCase();

export interface SnapshotRecord {
  uuid: string;
  make: string;
  model: string;
  year: number | null;
  ecu_hw: string;
  ecu_sw: string;
  adapter: string;
  adapter_firmware: string;
  os: string;
  protocol: string;
  function: string;
  result: Result;
  verification: 'community_reported';
  app_version: string;
  evidence: string[];
  notes: string;
  source: string;
  version: number;
  updated_at: string;
}

/** Combines shares into compatibility records in the app's snapshot format (imported as "community reported"). */
export async function compatSnapshot(shares: Share[], now: string, id: string) {
  const groups = new Map<string, { base: Omit<SnapshotRecord, 'result' | 'notes' | 'uuid'>; counts: Record<Result, number> }>();
  for (const s of shares) {
    if (!s.vehicle.make) continue;
    for (const r of s.results) {
      const base = {
        make: s.vehicle.make,
        // Modified vehicles are their own group, never mixed with standard ones.
        model: s.modified ? `${s.vehicle.model || s.vehicle.make} (modified)` : s.vehicle.model,
        year: s.vehicle.year,
        ecu_hw: '',
        ecu_sw: '',
        adapter: s.adapter.identity,
        adapter_firmware: s.adapter.firmware ?? '',
        os: s.os,
        protocol: s.protocol ?? '',
        function: r.function,
        verification: 'community_reported' as const,
        app_version: 'community',
        evidence: [],
        source: 'community',
        version: 1,
        updated_at: now,
      };
      const key = [base.make, base.model, base.year ?? '', base.adapter, base.adapter_firmware, base.os, base.protocol, base.function].map((x) => norm(String(x))).join('|');
      const g = groups.get(key) ?? { base, counts: { works: 0, partial: 0, fails: 0, not_supported: 0 } };
      g.counts[r.result] += 1;
      groups.set(key, g);
    }
  }
  const records: SnapshotRecord[] = [];
  for (const [key, g] of groups) {
    const total = Object.values(g.counts).reduce((x, y) => x + y, 0);
    if (total < MIN_REPORTS) continue;
    const result = (Object.entries(g.counts) as [Result, number][]).sort((x, y) => y[1] - x[1])[0][0];
    const parts = RESULTS.filter((r) => g.counts[r] > 0).map((r) => `${g.counts[r]} ${r.replace('_', ' ')}`);
    records.push({ ...g.base, uuid: `community-${await sha256(key)}`, result, notes: `${total} community reports: ${parts.join(', ')}` });
  }
  return { format: SNAPSHOT_FORMAT, snapshot_id: id, created_at: now, records };
}

/** How often each fault code was reported per make / model / year, standard and modified vehicles apart (only codes reported at least MIN_REPORTS times). */
export function codeSummary(shares: Share[]) {
  const counts = new Map<string, { make: string; model: string; year: number | null; code: string; reports: number; modified: boolean }>();
  for (const s of shares) {
    if (!s.vehicle.make) continue;
    for (const code of new Set(s.dtcs.map((d) => d.code))) {
      const modified = s.modified === true;
      const key = [norm(s.vehicle.make), norm(s.vehicle.model), s.vehicle.year ?? '', code, modified].join('|');
      const c = counts.get(key) ?? { make: s.vehicle.make, model: s.vehicle.model, year: s.vehicle.year, code, reports: 0, modified };
      c.reports += 1;
      counts.set(key, c);
    }
  }
  return [...counts.values()].filter((c) => c.reports >= MIN_REPORTS).sort((a, b) => b.reports - a.reports);
}

export async function sha256(s: string): Promise<string> {
  const d = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(s));
  return [...new Uint8Array(d)].map((b) => b.toString(16).padStart(2, '0')).join('');
}

// ── Suggestions from the app (become GitHub issues) ──────────────────────────

export interface Suggestion {
  kind: 'feature' | 'problem';
  text: string;
  app_version: string;
  os: string;
}

/** A suggestion typed in the app: kind, 10–2000 characters of text, version and OS. Nothing else. */
export function validateSuggestion(body: unknown): { ok: true; s: Suggestion } | { ok: false; error: string } {
  if (!isObj(body) || !onlyKeys(body, ['kind', 'text', 'app_version', 'os'])) return { ok: false, error: 'unexpected or missing fields' };
  if (body.kind !== 'feature' && body.kind !== 'problem') return { ok: false, error: 'kind' };
  if (typeof body.text !== 'string' || body.text.trim().length < 10 || body.text.length > 2000) return { ok: false, error: 'text must be 10 to 2000 characters' };
  if (!text(body.app_version, 20, /^[0-9A-Za-z.+-]+$/) || !['windows', 'android', 'linux', 'macos', 'ios'].includes(body.os as string)) return { ok: false, error: 'app version or os' };
  return { ok: true, s: body as unknown as Suggestion };
}

/** The GitHub issue for a suggestion. Mentions are defused so nobody gets pinged. */
export function issueFor(s: Suggestion, release: string | null) {
  const clean = s.text.trim().replace(/\r\n/g, '\n').replace(/@(?=\w)/g, '@​');
  const first = clean.split('\n')[0].trim();
  const short = first.length > 70 ? `${first.slice(0, 67).trimEnd()}…` : first;
  const label = s.kind === 'feature' ? 'feature request' : 'bug';
  return {
    title: `${s.kind === 'feature' ? 'Feature request' : 'Problem'}: ${short}`,
    body: `${clean}\n\n---\nSent from the OpenDiag app (${s.os}, ${release ?? s.app_version}). Not reviewed yet.`,
    labels: [label, 'from app'],
  };
}
