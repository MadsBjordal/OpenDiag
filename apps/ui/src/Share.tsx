// Anonymous result sharing (opt-in). Nothing is sent until the user has seen
// the exact data and pressed Send. Sending, deleting and downloading go through
// the app shell (apps/desktop/src-tauri/src/community.rs); the engine stays offline.

import { useEffect, useState } from 'react';
import { call, inApp } from './api';
import { ErrorBox, Spinner, useAction } from './components';
import { t } from './i18n';
import { useApp } from './state';
import type { Report, Settings } from './types';

type Result = 'works' | 'partial' | 'fails' | 'not_supported';
export interface ShareTest {
  function: string;
  result: Result;
}
interface SharePayload {
  vehicle: { make: string; model: string; year: number | null; wmi: string | null };
  adapter: { identity: string; link: string };
  protocol: string | null;
  results: ShareTest[];
  dtcs: { code: string; kind: string }[];
  freeze_frames: { values: unknown[] }[];
  monitors: unknown[];
  ecus: unknown[];
}
interface ShareRecord {
  id: string;
  delete_token: string;
  report_id: number;
  sent_at: string;
}

async function shell<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
  const { invoke } = await import('@tauri-apps/api/core');
  return invoke<T>(cmd, args);
}

const WHAT = [
  'The make, model and year of the vehicle and the first 3 characters of the VIN (the manufacturer code). Never the full VIN.',
  'The adapter, connection type and protocol, and which functions worked.',
  'Fault codes, the freeze-frame values recorded with them and the readiness monitors.',
  'Not sent: your name, location, the full VIN, notes, files or anything else on this device.',
];

/** The once-only question, also shown in Settings. */
function Consent({ onAnswer }: { onAnswer: (yes: boolean) => void }) {
  return (
    <>
      <p>
        {t(
          'Help others: share anonymous results so OpenDiag learns which vehicles and adapters work and which fault codes are common. You choose each time, and you see exactly what is sent first.',
        )}
      </p>
      <ul className="small">
        {WHAT.map((w) => (
          <li key={w}>{t(w)}</li>
        ))}
      </ul>
      <div className="row">
        <button className="btn primary small" onClick={() => onAnswer(true)}>
          {t('Yes, let me share')}
        </button>
        <button className="btn small" onClick={() => onAnswer(false)}>
          {t('No thanks')}
        </button>
      </div>
    </>
  );
}

function Preview({ s }: { s: SharePayload }) {
  const v = s.vehicle;
  return (
    <div className="small">
      <table>
        <tbody>
          <tr>
            <td>{t('Vehicle')}</td>
            <td>
              {[v.make || t('make unknown'), v.model, v.year].filter(Boolean).join(' ')}
              {v.wmi && ` (${t('manufacturer code')} ${v.wmi})`}
            </td>
          </tr>
          <tr>
            <td>{t('Adapter')}</td>
            <td>
              {s.adapter.identity} · {s.adapter.link} · {s.protocol ?? '–'}
            </td>
          </tr>
          <tr>
            <td>{t('Results')}</td>
            <td>{s.results.map((r) => `${r.function}: ${r.result}`).join(', ')}</td>
          </tr>
          <tr>
            <td>{t('Fault codes')}</td>
            <td>{s.dtcs.map((d) => `${d.code} (${d.kind})`).join(', ') || t('none')}</td>
          </tr>
          <tr>
            <td>{t('Also')}</td>
            <td>{t('{f} freeze frame(s), {m} readiness monitor(s), {e} calibration ID(s)', { f: s.freeze_frames.length, m: s.monitors.length, e: s.ecus.length })}</td>
          </tr>
        </tbody>
      </table>
      <details>
        <summary>{t('Show the exact data')}</summary>
        <pre className="mono" style={{ whiteSpace: 'pre-wrap', maxHeight: 260, overflow: 'auto' }}>
          {JSON.stringify(s, null, 2)}
        </pre>
      </details>
    </div>
  );
}

/** "Share anonymously" for a saved report (scan page, reports, hardware test). */
export function ShareCard({ reportId, simulated, tests = [] }: { reportId: number; simulated: boolean; tests?: ShareTest[] }) {
  const { settings, setSettings, go } = useApp();
  const [preview, setPreview] = useState<SharePayload | null>(null);
  const [sent, setSent] = useState(false);
  const answer = useAction(async (yes: boolean) => setSettings(await call<Settings>('set_setting', { key: 'share_results', value: yes ? 'true' : 'false' })));
  const prepare = useAction(async () => setPreview(await call<SharePayload>('share_preview', { report_id: reportId, tests })));
  const send = useAction(async () => {
    if (!preview) return;
    const r = await shell<{ id: string; delete_token: string }>('share_send', { share: preview });
    await call('share_remember', { id: r.id, delete_token: r.delete_token, report_id: reportId });
    setPreview(null);
    setSent(true);
  });
  if (!settings || simulated || !inApp()) return null;
  if (settings.share_results === false) return null;
  return (
    <div className="card">
      <h3 style={{ marginTop: 0 }}>{t('Share anonymously')}</h3>
      {settings.share_results == null ? (
        <Consent onAnswer={(yes) => answer.run(yes)} />
      ) : sent ? (
        <p className="banner ok small">
          {t('Thank you! Your result was shared. You can delete it again in Settings.')}{' '}
          <button className="btn small ghost" onClick={() => go('settings')}>
            {t('Settings')}
          </button>
        </p>
      ) : preview ? (
        <>
          <p className="small">{t('This is exactly what will be sent:')}</p>
          <Preview s={preview} />
          <div className="row" style={{ marginTop: 10 }}>
            <button className="btn primary small" disabled={send.busy} onClick={() => send.run()}>
              {send.busy ? <Spinner /> : null} {t('Send')}
            </button>
            <button className="btn small" onClick={() => setPreview(null)}>
              {t('Cancel')}
            </button>
          </div>
        </>
      ) : (
        <>
          <p className="small muted">
            {t('Shares which functions worked and the fault codes found, without the full VIN or anything personal. You see the data before it is sent.')}
          </p>
          <button className="btn small" disabled={prepare.busy} onClick={() => prepare.run()}>
            {prepare.busy ? <Spinner /> : '🤝'} {t('Share this result…')}
          </button>
        </>
      )}
      <ErrorBox error={answer.error ?? prepare.error ?? send.error} />
    </div>
  );
}

/** Community fault code counts for the scanned vehicle, from the last download. */
export function CommunityCodes({ report }: { report: Report }) {
  const make = report.vehicle.vin?.manufacturer;
  const [rows, setRows] = useState<{ code: string; model: string; year: number | null; reports: number }[]>([]);
  useEffect(() => {
    if (!make) return;
    call<typeof rows>('community_codes', { make }).then(setRows, () => setRows([]));
  }, [make]);
  const mine = new Set(report.dtcs.map((d) => d.code));
  const hits = rows.filter((r) => mine.has(r.code));
  if (!make || hits.length === 0) return null;
  return (
    <div className="card">
      <h3 style={{ marginTop: 0 }}>{t('Seen by others')}</h3>
      <ul className="tight small">
        {hits.map((r) => (
          <li key={`${r.code}-${r.model}-${r.year}`}>
            {t('{code}: in {n} shared scans of {vehicle}', { code: r.code, n: r.reports, vehicle: [make, r.model, r.year].filter(Boolean).join(' ') })}
          </li>
        ))}
      </ul>
      <p className="small muted">{t('Community reports, not verified. From the last download (Settings → Sharing).')}</p>
    </div>
  );
}

/** Settings card: opt in or out, delete what was shared, download community summaries. */
export function SharingSettings() {
  const { settings, setSettings } = useApp();
  const [list, setList] = useState<{ shares: ShareRecord[]; community_updated: string | null } | null>(null);
  const [downloaded, setDownloaded] = useState<number | null>(null);
  const reload = () => call<NonNullable<typeof list>>('shares').then(setList, () => setList(null));
  useEffect(() => {
    void reload();
  }, []);
  const set = useAction(async (yes: boolean) => setSettings(await call<Settings>('set_setting', { key: 'share_results', value: yes ? 'true' : 'false' })));
  const remove = useAction(async (s: ShareRecord) => {
    await shell('share_delete', { id: s.id, token: s.delete_token });
    await call('share_forget', { id: s.id });
    await reload();
  });
  const removeAll = useAction(async () => {
    for (const s of list?.shares ?? []) {
      await shell('share_delete', { id: s.id, token: s.delete_token });
      await call('share_forget', { id: s.id });
    }
    await reload();
  });
  const download = useAction(async () => {
    const d = await shell<{ snapshot: string; codes: unknown }>('community_fetch');
    const r = await call<{ imported: number }>('community_import', { snapshot: d.snapshot, codes: d.codes });
    setDownloaded(r.imported);
    await reload();
  });
  if (!settings) return null;
  return (
    <div className="card">
      <h2 style={{ marginTop: 0 }}>{t('Sharing')}</h2>
      {!inApp() ? (
        <p className="small muted">{t('Sharing is available in the installed app.')}</p>
      ) : (
        <>
          {settings.share_results == null ? (
            <Consent onAnswer={(yes) => set.run(yes)} />
          ) : (
            <label className="row small">
              <input type="checkbox" checked={!!settings.share_results} onChange={(e) => set.run(e.target.checked)} />
              {t('Offer to share anonymous results after scans and hardware tests')}
            </label>
          )}
          <h3>{t('What you shared')}</h3>
          {!list || list.shares.length === 0 ? (
            <p className="small muted">{t('Nothing shared from this device.')}</p>
          ) : (
            <>
              <ul className="tight small">
                {list.shares.map((s) => (
                  <li key={s.id}>
                    {new Date(s.sent_at).toLocaleString()} · {t('report {n}', { n: s.report_id })}{' '}
                    <button className="btn small ghost" disabled={remove.busy} onClick={() => remove.run(s)}>
                      {t('Delete')}
                    </button>
                  </li>
                ))}
              </ul>
              <button className="btn small" disabled={removeAll.busy} onClick={() => confirm(t('Delete everything you shared from the OpenDiag data service?')) && removeAll.run()}>
                {t('Delete all my shared results')}
              </button>
            </>
          )}
          <h3>{t('Community data')}</h3>
          <p className="small muted">
            {t('Combined results from everyone who shares: which vehicles and adapters work, and which fault codes are common. Shown as "community reported", never as verified.')}
          </p>
          <button className="btn small" disabled={download.busy} onClick={() => download.run()}>
            {download.busy ? <Spinner /> : '⬇'} {t('Download community data')}
          </button>
          {list?.community_updated && <span className="small muted"> {t('Last downloaded: {d}', { d: new Date(list.community_updated).toLocaleString() })}</span>}
          {downloaded != null && <p className="small banner ok">{t('{n} compatibility record(s) added or updated.', { n: downloaded })}</p>}
        </>
      )}
      <ErrorBox error={set.error ?? remove.error ?? removeAll.error ?? download.error} />
    </div>
  );
}
