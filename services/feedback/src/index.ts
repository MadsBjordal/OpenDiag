// OpenDiag community data service (Cloudflare Worker + D1).
//
//   POST   /v1/shares        an anonymous shared result → { id, delete_token }
//   DELETE /v1/shares/:id    with "Authorization: Bearer <delete_token>": removes it
//   GET    /v1/snapshot      combined compatibility records (app's snapshot format)
//   GET    /v1/codes         fault codes per make / model / year
//   GET    /v1/stats         totals for the download page
//
// Stored per share: the validated JSON, the day it arrived and a hash of the
// delete token. No IP addresses are stored; the daily rate limit keeps only a
// salted hash that changes every day and is deleted the next day.

import { codeSummary, compatSnapshot, MAX_BODY, sha256, validate, type Share } from './logic';

interface Env {
  DB: D1Database;
}

const MAX_PER_DAY = 30;
const SCHEMA = [
  'CREATE TABLE IF NOT EXISTS shares (id TEXT PRIMARY KEY, day TEXT NOT NULL, token_hash TEXT NOT NULL, data TEXT NOT NULL)',
  'CREATE TABLE IF NOT EXISTS rate (day TEXT NOT NULL, key TEXT NOT NULL, n INTEGER NOT NULL, PRIMARY KEY (day, key))',
  'CREATE TABLE IF NOT EXISTS salt (day TEXT PRIMARY KEY, value TEXT NOT NULL)',
];
let ready = false;

const headers = {
  'content-type': 'application/json; charset=utf-8',
  'access-control-allow-origin': '*',
  'x-content-type-options': 'nosniff',
};
const json = (data: unknown, status = 200, extra: Record<string, string> = {}) => new Response(JSON.stringify(data), { status, headers: { ...headers, ...extra } });
const fail = (status: number, error: string) => json({ error }, status);
const today = () => new Date().toISOString().slice(0, 10);

async function setup(db: D1Database) {
  if (ready) return;
  await db.batch(SCHEMA.map((s) => db.prepare(s)));
  ready = true;
}

/** Counts this sender's shares today without storing the address itself. */
async function overLimit(db: D1Database, ip: string): Promise<boolean> {
  const day = today();
  await db.batch([db.prepare('DELETE FROM rate WHERE day < ?1').bind(day), db.prepare('DELETE FROM salt WHERE day < ?1').bind(day)]);
  await db.prepare('INSERT OR IGNORE INTO salt (day, value) VALUES (?1, ?2)').bind(day, crypto.randomUUID()).run();
  const salt = await db.prepare('SELECT value FROM salt WHERE day = ?1').bind(day).first<string>('value');
  const key = await sha256(`${salt}|${ip}`);
  const row = await db
    .prepare('INSERT INTO rate (day, key, n) VALUES (?1, ?2, 1) ON CONFLICT (day, key) DO UPDATE SET n = n + 1 RETURNING n')
    .bind(day, key)
    .first<number>('n');
  return (row ?? 0) > MAX_PER_DAY;
}

async function allShares(db: D1Database): Promise<Share[]> {
  const { results } = await db.prepare('SELECT data FROM shares').all<{ data: string }>();
  return results.map((r) => JSON.parse(r.data) as Share);
}

export default {
  async fetch(req: Request, env: Env): Promise<Response> {
    const url = new URL(req.url);
    if (req.method === 'OPTIONS') return new Response(null, { headers: { ...headers, 'access-control-allow-methods': 'GET', 'access-control-max-age': '86400' } });
    await setup(env.DB);
    const cache = { 'cache-control': 'public, max-age=3600' };

    if (req.method === 'POST' && url.pathname === '/v1/shares') {
      const body = await req.text();
      if (body.length > MAX_BODY) return fail(413, 'too large');
      let parsed: unknown;
      try {
        parsed = JSON.parse(body);
      } catch {
        return fail(400, 'not JSON');
      }
      const check = validate(parsed);
      if (!check.ok) return fail(422, check.error);
      if (await overLimit(env.DB, req.headers.get('cf-connecting-ip') ?? 'unknown')) return fail(429, 'too many shares today');
      const id = crypto.randomUUID();
      const token = crypto.randomUUID() + crypto.randomUUID().replaceAll('-', '');
      await env.DB.prepare('INSERT INTO shares (id, day, token_hash, data) VALUES (?1, ?2, ?3, ?4)')
        .bind(id, today(), await sha256(token), JSON.stringify(check.share))
        .run();
      return json({ id, delete_token: token }, 201);
    }

    const del = url.pathname.match(/^\/v1\/shares\/([0-9a-f-]{36})$/);
    if (req.method === 'DELETE' && del) {
      const token = (req.headers.get('authorization') ?? '').replace(/^Bearer /, '');
      const r = await env.DB.prepare('DELETE FROM shares WHERE id = ?1 AND token_hash = ?2').bind(del[1], await sha256(token)).run();
      return r.meta.changes ? json({ deleted: true }) : fail(404, 'not found or wrong token');
    }

    if (req.method === 'GET' && url.pathname === '/v1/snapshot') {
      const now = new Date().toISOString();
      return json(await compatSnapshot(await allShares(env.DB), now, `community-${now.slice(0, 13)}`), 200, cache);
    }
    if (req.method === 'GET' && url.pathname === '/v1/codes') return json(codeSummary(await allShares(env.DB)), 200, cache);
    if (req.method === 'GET' && url.pathname === '/v1/stats') {
      const shares = await allShares(env.DB);
      const vehicles = new Set(shares.map((s) => [s.vehicle.make, s.vehicle.model, s.vehicle.year].join('|').toLowerCase()));
      const adapters = new Set(shares.map((s) => s.adapter.identity.toLowerCase()));
      return json({ shares: shares.length, vehicles: vehicles.size, adapters: adapters.size }, 200, { 'cache-control': 'public, max-age=600' });
    }
    if (req.method === 'GET' && url.pathname === '/') return json({ service: 'OpenDiag community data', docs: 'https://github.com/MadsBjordal/OpenDiag/tree/main/services/feedback' });
    return fail(404, 'not found');
  },
};
