import { useEffect, useState } from 'react';
import { call } from '../api';
import { ErrorBox, Modal, SimBanner, Spinner, useAction } from '../components';
import { DtcCard } from '../DtcCard';
import { t } from '../i18n';
import { useApp } from '../state';
import type { ClearPlan, ClearResult, Explanation } from '../types';

function ClearFlow({ onClose }: { onClose: () => void }) {
  const { settings, go, refreshSummary } = useApp();
  const [plan, setPlan] = useState<ClearPlan | null>(null);
  const [result, setResult] = useState<ClearResult | null>(null);
  const [ack, setAck] = useState({ repair: false, authority: false, systems: false });
  const prepare = useAction(async () => setPlan(await call<ClearPlan>('prepare_clear')));
  const execute = useAction(async () => {
    setResult(await call<ClearResult>('execute_clear', { plan_id: plan!.id, confirmed: true }));
    await refreshSummary();
  });
  useEffect(() => {
    void prepare.run();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  const allAck = ack.repair && ack.authority && ack.systems;

  if (result) {
    return (
      <Modal title={result.performed ? t('Clear finished') : t('Nothing was cleared')} onClose={onClose}>
        {!result.performed && (
          <div className="banner bad">
            <b>{t('Blocked by the safety checks:')}</b>
            <ul className="tight">
              {result.decision.blockers.map((b) => (
                <li key={b}>{b}</li>
              ))}
            </ul>
          </div>
        )}
        {result.performed && <p className="banner ok">{result.summary}</p>}
        {result.modules.map((m) => (
          <p key={m.ecu}>
            {m.accepted ? '✓' : '✗'} {m.label}: {m.message}
          </p>
        ))}
        {result.performed && (
          <>
            <h3>{t('Read back after clearing')}</h3>
            {result.remaining.length === 0 ? (
              <p>{t('No codes reported.')}</p>
            ) : (
              <ul className="tight">
                {result.remaining.map((d) => (
                  <li key={d.code + d.kind}>
                    <b className="mono">{d.code}</b> ({d.kind}) {d.explanation.title}
                  </li>
                ))}
              </ul>
            )}
            <p className="muted small">{t('Run a new scan to save the state after clearing as a report.')}</p>
          </>
        )}
        <div className="row">
          <button className="btn primary" onClick={onClose}>
            {t('Close')}
          </button>
          {result.performed && (
            <button className="btn" onClick={() => go('scan', 'start')}>
              {t('Scan again')}
            </button>
          )}
        </div>
      </Modal>
    );
  }

  return (
    <Modal title={t('Clear fault codes')} onClose={onClose}>
      {prepare.busy && (
        <p>
          <Spinner /> {t('Reading and saving the current codes first…')}
        </p>
      )}
      <ErrorBox error={prepare.error} />
      {plan && (
        <>
          <SimBanner show={plan.simulated} />
          <p className="banner ok small">{t('Saved before clearing: the codes, freeze frame and readiness (snapshot #{id}).', { id: plan.snapshot_id })}</p>
          <h3>{t('These codes will be erased')}</h3>
          {plan.codes.length === 0 ? (
            <p>{t('No stored or pending codes.')}</p>
          ) : (
            <ul className="tight">
              {plan.codes.map((d) => (
                <li key={d.code + d.kind + d.ecu}>
                  <b className="mono">{d.code}</b> ({d.kind}, {d.ecu_label}) {d.explanation.title}
                </li>
              ))}
            </ul>
          )}
          {plan.permanent.length > 0 && (
            <p className="small">
              {t('Permanent codes that will stay')}: <span className="mono">{plan.permanent.map((d) => d.code).join(', ')}</span>
            </p>
          )}
          <h3>{t('Systems the request goes to')}</h3>
          <p>{plan.affected_modules.map(([, l]) => l).join(' · ') || t('every emissions-related module that answers')}</p>
          <h3>{t('What clearing does')}</h3>
          <ul className="tight small">
            {plan.consequences.map((c) => (
              <li key={c}>{c}</li>
            ))}
          </ul>
          {plan.preview.blockers.length > 0 && (
            <div className="banner bad">
              <b>{t('Clearing is blocked right now:')}</b>
              <ul className="tight">
                {plan.preview.blockers.map((b) => (
                  <li key={b}>{b}</li>
                ))}
              </ul>
              {!settings?.terms_accepted && (
                <button className="btn small" onClick={() => go('settings')}>
                  {t('Open Settings')}
                </button>
              )}
            </div>
          )}
          {plan.preview.warnings.map((w) => (
            <p key={w} className="banner warn small">
              {w}
            </p>
          ))}
          <div className="stack" style={{ marginTop: 12 }}>
            <label className="check">
              <input type="checkbox" checked={ack.repair} onChange={(e) => setAck({ ...ack, repair: e.target.checked })} />
              {t('I understand that clearing codes does not repair the fault.')}
            </label>
            <label className="check">
              <input type="checkbox" checked={ack.systems} onChange={(e) => setAck({ ...ack, systems: e.target.checked })} />
              {t('I understand the freeze frame and readiness status of the systems listed above will be reset.')}
            </label>
            <label className="check">
              <input type="checkbox" checked={ack.authority} onChange={(e) => setAck({ ...ack, authority: e.target.checked })} />
              {t('I own this vehicle or have the owner\'s permission.')}
            </label>
          </div>
          <div className="row" style={{ marginTop: 14 }}>
            <button className="btn danger" disabled={!allAck || execute.busy} onClick={() => execute.run()}>
              {execute.busy ? <Spinner /> : null} {t('Clear the codes')}
            </button>
            <button className="btn" onClick={onClose}>
              {t('Cancel')}
            </button>
          </div>
          <ErrorBox error={execute.error} />
        </>
      )}
    </Modal>
  );
}

export default function Codes() {
  const { report, summary } = useApp();
  const [q, setQ] = useState('');
  const [found, setFound] = useState<Explanation[]>([]);
  const [clearing, setClearing] = useState(false);
  useEffect(() => {
    const h = setTimeout(() => {
      if (q.trim().length >= 2) call<Explanation[]>('dtc_search', { q }).then(setFound).catch(() => setFound([]));
      else setFound([]);
    }, 200);
    return () => clearTimeout(h);
  }, [q]);
  const lookup = useAction(async () => setFound([await call<Explanation>('dtc_explain', { code: q.trim() })]));
  return (
    <>
      <div className="row between">
        <h1>{t('Fault Codes')}</h1>
        {summary?.vehicle_connected && (
          <button className="btn danger" onClick={() => setClearing(true)}>
            {t('Clear codes…')}
          </button>
        )}
      </div>
      <SimBanner show={!!report?.simulated} />
      {report ? (
        <>
          <h2>{t('From the last scan')}</h2>
          {report.dtcs.length === 0 ? <p className="empty">{t('No fault codes in the scanned modules.')}</p> : report.dtcs.map((d) => <DtcCard key={d.code + d.kind + d.ecu} finding={d} observed={report.measurements} />)}
        </>
      ) : (
        <p className="muted">{t('Scan the vehicle to see its codes here. You can also look up any code below.')}</p>
      )}
      <h2>{t('Look up a code')}</h2>
      <div className="row">
        <input type="search" value={q} onChange={(e) => setQ(e.target.value)} placeholder={t('e.g. P0301 or "misfire"')} style={{ flex: 1, minWidth: 200 }} aria-label={t('Search fault codes')} />
        <button className="btn" onClick={() => lookup.run()} disabled={q.trim().length !== 5}>
          {t('Explain')}
        </button>
      </div>
      <p className="muted small">{t('The offline library covers common generic codes. Manufacturer-specific codes are shown with their structure only.')}</p>
      <div className="stack" style={{ marginTop: 10 }}>
        {found.slice(0, 30).map((e) => (
          <DtcCard key={e.code} explanation={e} />
        ))}
      </div>
      {clearing && <ClearFlow onClose={() => setClearing(false)} />}
    </>
  );
}
