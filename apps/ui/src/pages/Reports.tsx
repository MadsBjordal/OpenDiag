import { useEffect, useState } from 'react';
import { call } from '../api';
import { ErrorBox, useAction } from '../components';
import { t } from '../i18n';
import { ReportView } from '../ReportView';
import { useApp } from '../state';
import type { Report, ReportSummary } from '../types';

interface Comparison {
  older: string;
  newer: string;
  new_codes: [string, string][];
  still_present: [string, string][];
  gone: [string, string][];
}

export default function Reports() {
  const { arg, go } = useApp();
  const [list, setList] = useState<ReportSummary[]>([]);
  const [open, setOpen] = useState<Report | null>(null);
  const [pick, setPick] = useState<number[]>([]);
  const [cmp, setCmp] = useState<Comparison | null>(null);
  const load = useAction(async () => setList(await call<ReportSummary[]>('reports', { vehicle_id: null })));
  const openR = useAction(async (id: number) => setOpen(await call<Report>('report', { id })));
  const compare = useAction(async () => {
    const [a, b] = [...pick].sort((x, y) => x - y);
    setCmp(await call<Comparison>('compare', { older: a, newer: b }));
  });
  const del = useAction(async (id: number) => {
    if (!confirm(t('Delete this report? This cannot be undone.'))) return;
    await call('delete_report', { id });
    setOpen(null);
    go('reports');
    await load.run();
  });
  useEffect(() => {
    void load.run();
  }, []);
  useEffect(() => {
    if (arg) void openR.run(Number(arg));
    else setOpen(null);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [arg]);

  if (open) {
    return (
      <>
        <div className="row between">
          <button className="btn small ghost" onClick={() => go('reports')}>
            ← {t('All reports')}
          </button>
          <button className="btn small" onClick={() => del.run(Number(open.id))}>
            🗑 {t('Delete')}
          </button>
        </div>
        <h1>{t('Report #{id}', { id: open.id })}</h1>
        <ReportView report={open} />
      </>
    );
  }
  const togglePick = (id: number) => setPick((p) => (p.includes(id) ? p.filter((x) => x !== id) : [...p.slice(-1), id]));
  return (
    <>
      <h1>{t('Reports')}</h1>
      <p className="muted">{t('Every scan is saved here on this device. Open one to add notes or export it; tick two to compare.')}</p>
      <ErrorBox error={load.error ?? openR.error} />
      {list.length === 0 ? (
        <p className="empty">{t('No reports yet. Run a scan to create one.')}</p>
      ) : (
        <div className="card table-wrap">
          <table>
            <thead>
              <tr>
                <th>{t('Compare')}</th>
                <th>{t('Report')}</th>
                <th>{t('Date')}</th>
                <th>{t('Codes')}</th>
              </tr>
            </thead>
            <tbody>
              {list.map((r) => (
                <tr key={r.id}>
                  <td>
                    <input type="checkbox" aria-label={t('Select for comparison')} checked={pick.includes(r.id)} onChange={() => togglePick(r.id)} />
                  </td>
                  <td>
                    <a href={`#/reports/${r.id}`}>
                      #{r.id} {r.title}
                    </a>{' '}
                    {r.simulated && <span className="chip sim">{t('SIMULATED')}</span>}
                  </td>
                  <td className="small">{new Date(r.created_at).toLocaleString()}</td>
                  <td>{r.dtc_count}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      <div className="row" style={{ marginTop: 12 }}>
        <button className="btn" disabled={pick.length !== 2} onClick={() => compare.run()}>
          {t('Compare the two selected')}
        </button>
      </div>
      <ErrorBox error={compare.error} />
      {cmp && (
        <div className="card" style={{ marginTop: 12 }}>
          <h2 style={{ marginTop: 0 }}>{t('Changes between the scans')}</h2>
          <p className="muted small">
            {new Date(cmp.older).toLocaleString()} → {new Date(cmp.newer).toLocaleString()}
          </p>
          <div className="grid two">
            {[
              ['New', cmp.new_codes, 'bad'],
              ['Still present', cmp.still_present, 'warn'],
              ['Gone', cmp.gone, 'ok'],
            ].map(([label, codes, cls]) => (
              <div key={label as string}>
                <h3>
                  <span className={`chip ${cls}`}>{t(label as string)}</span>
                </h3>
                {(codes as [string, string][]).length === 0 ? (
                  <p className="muted">—</p>
                ) : (
                  <ul className="tight">
                    {(codes as [string, string][]).map(([c, k]) => (
                      <li key={c + k}>
                        <b className="mono">{c}</b> <span className="muted">{k}</span>
                      </li>
                    ))}
                  </ul>
                )}
              </div>
            ))}
          </div>
        </div>
      )}
    </>
  );
}
