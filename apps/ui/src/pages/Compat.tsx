import { useEffect, useState } from 'react';
import { call, saveFile } from '../api';
import { ErrorBox, StatusChip, useAction } from '../components';
import { t } from '../i18n';
import { useApp } from '../state';
import type { CatalogEntry, CompatRecord, CompatStatus, Submission } from '../types';

type Tab = 'status' | 'database' | 'submit' | 'moderate' | 'snapshots' | 'functions';

const VER: Record<string, string> = {
  simulated: 'Simulated',
  unverified_local: 'Seen on this device, unverified',
  community_reported: 'Reported, not reviewed',
  community_tested: 'Community tested',
  maintainer_verified: 'Maintainer verified',
  hil_verified: 'Verified on test bench',
};

function verChip(v: string | null) {
  if (!v) return <span className="muted">—</span>;
  const cls = v === 'simulated' ? 'sim' : v === 'maintainer_verified' || v === 'hil_verified' ? 'ok' : v === 'community_tested' ? 'accent' : 'warn';
  return <span className={`chip ${cls}`}>{t(VER[v] ?? v)}</span>;
}

function StatusTab() {
  const [s, setS] = useState<CompatStatus | null>(null);
  const load = useAction(async () => setS(await call<CompatStatus>('compat_status')));
  useEffect(() => {
    void load.run();
  }, []);
  if (!s) return <ErrorBox error={load.error} />;
  return (
    <>
      <p className={`banner ${s.simulated ? 'sim' : 'info'}`}>{s.summary}</p>
      <p className="muted small">
        {t('Platform')}: <b>{s.platform}</b> · {t('Adapter')}: <b>{s.adapter || '—'}</b> · {t('Earlier results for this adapter here')}: {s.adapter_known_records}
      </p>
      <div className="card table-wrap">
        <table>
          <thead>
            <tr>
              <th>{t('Function')}</th>
              <th>{t('Status')}</th>
              <th>{t('Last result here')}</th>
              <th>{t('Verification')}</th>
            </tr>
          </thead>
          <tbody>
            {s.functions.map((f) => (
              <tr key={f.id}>
                <td>
                  {f.name}
                  <div className="muted small">{f.note}</div>
                </td>
                <td>
                  <StatusChip status={f.status} label={f.status_label} />
                </td>
                <td>{f.local_result ?? '—'}</td>
                <td>{verChip(f.best_verification)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </>
  );
}

function DatabaseTab() {
  const [filter, setFilter] = useState({ make: '', adapter: '', os: '' });
  const [rows, setRows] = useState<CompatRecord[]>([]);
  const [hist, setHist] = useState<[number, string, string, CompatRecord][] | null>(null);
  const q = useAction(async () => setRows(await call<CompatRecord[]>('compat_query', { filter })));
  const h = useAction(async (uuid: string) => setHist(await call('compat_history', { uuid })));
  useEffect(() => {
    void q.run();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  return (
    <>
      <div className="row">
        {(['make', 'adapter', 'os'] as const).map((k) => (
          <input key={k} type="search" placeholder={t(k === 'make' ? 'Make' : k === 'adapter' ? 'Adapter' : 'Operating system')} value={filter[k]} onChange={(e) => setFilter({ ...filter, [k]: e.target.value })} style={{ flex: 1, minWidth: 140 }} />
        ))}
        <button className="btn" onClick={() => q.run()}>
          {t('Search')}
        </button>
      </div>
      <p className="muted small">{t('Each row is one combination: vehicle, ECU, adapter and firmware, operating system, protocol, function and app version.')}</p>
      <div className="card table-wrap">
        <table>
          <thead>
            <tr>
              <th>{t('Vehicle / ECU')}</th>
              <th>{t('Adapter')}</th>
              <th>OS</th>
              <th>{t('Function')}</th>
              <th>{t('Result')}</th>
              <th>{t('Verification')}</th>
              <th>v</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((r) => (
              <tr key={r.uuid}>
                <td>
                  {r.make} {r.model} {r.year ?? ''}
                  <div className="muted small mono">{[r.ecu_hw, r.ecu_sw].filter(Boolean).join(' / ')}</div>
                </td>
                <td>
                  {r.adapter}
                  <div className="muted small">{r.adapter_firmware}</div>
                </td>
                <td>{r.os}</td>
                <td>
                  {r.function}
                  <div className="muted small">
                    {r.protocol} · app {r.app_version}
                  </div>
                </td>
                <td>{r.result}</td>
                <td>{verChip(r.verification)}</td>
                <td>
                  <button className="btn small ghost" onClick={() => h.run(r.uuid!)}>
                    {r.version}
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
        {rows.length === 0 && <p className="empty">{t('No records.')}</p>}
      </div>
      {hist && (
        <div className="card">
          <h3 style={{ marginTop: 0 }}>{t('Version history')}</h3>
          <ul className="tight small">
            {hist.map(([v, at, note, rec]) => (
              <li key={v}>
                v{v} · {new Date(at).toLocaleString()} · {note} · {rec.result} · {rec.verification}
              </li>
            ))}
          </ul>
        </div>
      )}
    </>
  );
}

function SubmitTab() {
  const { settings } = useApp();
  const [local, setLocal] = useState<CompatRecord[]>([]);
  const [rec, setRec] = useState<CompatRecord | null>(null);
  const [evidence, setEvidence] = useState('');
  const [done, setDone] = useState<number | null>(null);
  useEffect(() => {
    call<CompatRecord[]>('compat_query', { filter: {} })
      .then((r) => setLocal(r.filter((x) => x.verification !== 'simulated' && x.source === 'local')))
      .catch(() => {});
  }, []);
  const submit = useAction(async () => {
    const r = await call<{ id: number }>('compat_submit', { record: { ...rec!, evidence: evidence.split('\n').filter((l) => l.trim()) } });
    setDone(r.id);
  });
  const blank: CompatRecord = { make: '', model: '', year: null, ecu_hw: '', ecu_sw: '', adapter: '', adapter_firmware: '', os: settings?.platform ?? '', protocol: '', function: 'read_dtcs', result: 'works', verification: 'community_reported', app_version: '0.1.0', evidence: [], notes: '' };
  return (
    <>
      <p className="muted">
        {t('Share what worked (or did not) with your vehicle and adapter. A submission is queued for moderation; it is never counted as verified just because it was submitted. Online sharing is not available yet: export a snapshot to send it to the project.')}
      </p>
      <div className="row">
        <select onChange={(e) => setRec(e.target.value === 'new' ? blank : local[Number(e.target.value)])} defaultValue="">
          <option value="" disabled>
            {t('Start from…')}
          </option>
          <option value="new">{t('A blank record')}</option>
          {local.map((r, i) => (
            <option key={r.uuid} value={i}>
              {r.make} · {r.adapter} · {r.function} · {r.result}
            </option>
          ))}
        </select>
      </div>
      {rec && (
        <div className="card stack" style={{ marginTop: 10 }}>
          <div className="grid two">
            {(['make', 'model', 'ecu_hw', 'ecu_sw', 'adapter', 'adapter_firmware', 'os', 'protocol', 'function'] as const).map((k) => (
              <label key={k} className="field">
                {k}
                <input type="text" value={String(rec[k] ?? '')} onChange={(e) => setRec({ ...rec, [k]: e.target.value })} />
              </label>
            ))}
            <label className="field">
              {t('Result')}
              <select value={rec.result} onChange={(e) => setRec({ ...rec, result: e.target.value as CompatRecord['result'] })}>
                <option value="works">works</option>
                <option value="partial">partial</option>
                <option value="fails">fails</option>
                <option value="not_supported">not_supported</option>
              </select>
            </label>
          </div>
          <label className="field">
            {t('Evidence (one item per line: log excerpt, test notes, photo reference). Required.')}
            <textarea value={evidence} onChange={(e) => setEvidence(e.target.value)} />
          </label>
          <label className="field">
            {t('Notes')}
            <textarea value={rec.notes} onChange={(e) => setRec({ ...rec, notes: e.target.value })} />
          </label>
          <p className="muted small">{t('Do not include your VIN or personal data.')}</p>
          <button className="btn primary" onClick={() => submit.run()}>
            {t('Queue for moderation')}
          </button>
          <ErrorBox error={submit.error} />
          {done && <p className="banner ok">{t('Queued as submission #{id}.', { id: done })}</p>}
        </div>
      )}
    </>
  );
}

function ModerateTab() {
  const [subs, setSubs] = useState<Submission[]>([]);
  const [notes, setNotes] = useState<Record<number, string>>({});
  const load = useAction(async () => setSubs(await call<Submission[]>('compat_submissions', { status: null })));
  const decide = useAction(async (id: number, decision: string) => {
    await call('compat_moderate', { id, decision, note: notes[id] ?? '' });
    await load.run();
  });
  useEffect(() => {
    void load.run();
  }, []);
  return (
    <>
      <p className="muted">{t('Review queued submissions. Approval records them as "community tested", never higher. Maintainer and test-bench verification come only from verified releases.')}</p>
      <ErrorBox error={decide.error ?? load.error} />
      {subs.length === 0 && <p className="empty">{t('Nothing to review.')}</p>}
      {subs.map((s) => (
        <div key={s.id} className="card">
          <div className="row between">
            <b>
              #{s.id} {s.record.make} {s.record.model} · {s.record.adapter} · {s.record.function}: {s.record.result}
            </b>
            <span className="chip">{s.status}</span>
          </div>
          <ul className="tight small">
            {s.record.evidence.map((e) => (
              <li key={e}>{e}</li>
            ))}
          </ul>
          {s.moderator_note && <p className="small muted">{s.moderator_note}</p>}
          {(s.status === 'pending' || s.status === 'needs_evidence') && (
            <div className="row">
              <input type="text" placeholder={t('Note (required to reject or ask for evidence)')} value={notes[s.id] ?? ''} onChange={(e) => setNotes({ ...notes, [s.id]: e.target.value })} style={{ flex: 1, minWidth: 200 }} />
              <button className="btn small primary" onClick={() => decide.run(s.id, 'approve')}>
                {t('Approve')}
              </button>
              <button className="btn small" onClick={() => decide.run(s.id, 'needs_evidence')}>
                {t('Needs evidence')}
              </button>
              <button className="btn small" onClick={() => decide.run(s.id, 'reject')}>
                {t('Reject')}
              </button>
            </div>
          )}
        </div>
      ))}
    </>
  );
}

function SnapshotsTab() {
  const [msg, setMsg] = useState<string | null>(null);
  const exp = useAction(async () => setMsg(t('Saved: {f}', { f: await saveFile(await call('compat_export')) })));
  const imp = useAction(async (file: File) => {
    const r = await call<{ imported: number }>('compat_import', { json: await file.text(), source: file.name });
    setMsg(t('{n} record(s) imported or updated.', { n: r.imported }));
  });
  return (
    <>
      <p className="muted">
        {t('Snapshots let the compatibility database work offline: import a snapshot file published by the project, or export yours to contribute. Imported records count at most as "community tested" until signed snapshots exist.')}
      </p>
      <div className="row">
        <button className="btn" onClick={() => exp.run()}>
          {t('Export snapshot')}
        </button>
        <label className="btn">
          {t('Import snapshot…')}
          <input type="file" accept=".json,application/json" hidden onChange={(e) => e.target.files?.[0] && imp.run(e.target.files[0])} />
        </label>
      </div>
      {msg && <p className="banner ok">{msg}</p>}
      <ErrorBox error={exp.error ?? imp.error} />
    </>
  );
}

function FunctionsTab() {
  const [f, setF] = useState<CatalogEntry[]>([]);
  useEffect(() => {
    call<CatalogEntry[]>('functions').then(setF).catch(() => {});
  }, []);
  return (
    <div className="card table-wrap">
      <table>
        <thead>
          <tr>
            <th>{t('Function')}</th>
            <th>{t('Risk')}</th>
            <th>Windows</th>
            <th>Android</th>
            <th>Linux</th>
          </tr>
        </thead>
        <tbody>
          {f.map((x) => (
            <tr key={x.id}>
              <td>
                {x.name}
                <div className="muted small">{x.note}</div>
              </td>
              <td>{x.risk_level}</td>
              <td>
                <StatusChip status={x.windows} />
              </td>
              <td>
                <StatusChip status={x.android} />
              </td>
              <td>
                <StatusChip status={x.linux} />
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

export default function Compat() {
  const [tab, setTab] = useState<Tab>('status');
  const tabs: [Tab, string][] = [
    ['status', 'This connection'],
    ['database', 'Database'],
    ['submit', 'Submit a result'],
    ['moderate', 'Moderation'],
    ['snapshots', 'Offline snapshots'],
    ['functions', 'All functions'],
  ];
  return (
    <>
      <h1>{t('Compatibility')}</h1>
      <p className="muted">{t('What has been shown to work, with which vehicle, ECU, adapter and operating system. Statuses are never upgraded without testing.')}</p>
      <div className="row" role="tablist" style={{ margin: '12px 0' }}>
        {tabs.map(([k, label]) => (
          <button key={k} role="tab" aria-selected={tab === k} className={`btn small ${tab === k ? 'primary' : ''}`} onClick={() => setTab(k)}>
            {t(label)}
          </button>
        ))}
      </div>
      {tab === 'status' && <StatusTab />}
      {tab === 'database' && <DatabaseTab />}
      {tab === 'submit' && <SubmitTab />}
      {tab === 'moderate' && <ModerateTab />}
      {tab === 'snapshots' && <SnapshotsTab />}
      {tab === 'functions' && <FunctionsTab />}
    </>
  );
}
