import { useEffect, useRef, useState } from 'react';
import { call } from '../api';
import { ErrorBox, SimBanner } from '../components';
import { t } from '../i18n';
import { useApp } from '../state';
import type { MisfireReport } from '../types';

const REFRESH_MS = 2000;

export default function Misfire() {
  const { summary, go } = useApp();
  const [report, setReport] = useState<MisfireReport | null>(null);
  const [running, setRunning] = useState(false);
  const [error, setError] = useState<unknown>(null);
  // The first count seen per cylinder while watching, to show what was added since.
  const start = useRef<Record<number, number>>({});

  useEffect(() => {
    if (!running) return;
    let stop = false;
    const loop = async () => {
      while (!stop) {
        const began = Date.now();
        try {
          const r = await call<MisfireReport>('misfires');
          for (const c of r.cylinders) if (c.current !== null && start.current[c.cylinder] === undefined) start.current[c.cylinder] = c.current;
          setReport(r);
          setError(null);
          if (!r.supported) {
            setRunning(false);
            return;
          }
        } catch (e) {
          setError(e);
          setRunning(false);
          return;
        }
        await new Promise((res) => setTimeout(res, Math.max(0, REFRESH_MS - (Date.now() - began))));
      }
    };
    void loop();
    return () => {
      stop = true;
    };
  }, [running]);

  if (!summary?.vehicle_connected) {
    return (
      <>
        <h1>{t('Misfires')}</h1>
        <p>{t('Connect to a vehicle first.')}</p>
        <button className="btn primary" onClick={() => go('connect')}>
          {t('Connect to Vehicle')}
        </button>
      </>
    );
  }

  const begin = () => {
    start.current = {};
    setReport(null);
    setRunning(true);
  };
  const cylinders = report?.cylinders ?? [];
  const added = (c: MisfireReport['cylinders'][number]) =>
    c.current === null || start.current[c.cylinder] === undefined ? null : Math.max(0, c.current - start.current[c.cylinder]);
  const top = Math.max(1, ...cylinders.map((c) => c.current ?? 0));
  const worst = cylinders.reduce<number | null>(
    (w, c) => ((c.current ?? 0) > 0 && (w === null || (c.current ?? 0) > (cylinders.find((x) => x.cylinder === w)?.current ?? 0)) ? c.cylinder : w),
    null,
  );

  return (
    <>
      <h1>{t('Misfires per cylinder')}</h1>
      <SimBanner show={!!report?.simulated} />
      <p>
        {t(
          "The engine computer's own misfire counters (OBD service 06). Start the engine and watch: a cylinder whose count keeps rising is misfiring. The car decides how often it updates the counts; some only update after a drive.",
        )}
      </p>
      <div className="row" style={{ gap: 8 }}>
        {running ? (
          <button className="btn" onClick={() => setRunning(false)}>
            {t('Stop')}
          </button>
        ) : (
          <button className="btn primary" onClick={begin}>
            {report ? t('Start again') : t('Start')}
          </button>
        )}
        {running && <span className="muted small">{t('Reading every {s} seconds…', { s: REFRESH_MS / 1000 })}</span>}
      </div>
      <ErrorBox error={error} />
      {report && !report.supported && <div className="banner warn">{t(report.reason ?? 'Not supported by this vehicle.')}</div>}
      {report?.supported && (
        <div className="card" style={{ marginTop: 12 }}>
          {worst !== null && <p>{t('Most misfires this drive: cylinder {n}.', { n: worst })}</p>}
          <div className="table-wrap">
            <table>
              <thead>
                <tr>
                  <th>{t('Cylinder')}</th>
                  <th style={{ width: '45%' }}>{t('This drive')}</th>
                  <th>{t('Since you pressed Start')}</th>
                  <th>{t('Average of the last 10 drives')}</th>
                </tr>
              </thead>
              <tbody>
                {cylinders.map((c) => {
                  const plus = added(c);
                  return (
                    <tr key={c.cylinder}>
                      <td className="mono">{c.cylinder}</td>
                      <td>
                        {c.current === null ? (
                          <span className="muted">{t('not reported')}</span>
                        ) : (
                          <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                            <div style={{ flex: 1, background: 'var(--border)', height: 12, borderRadius: 6 }}>
                              <div style={{ width: `${(100 * c.current) / top}%`, height: '100%', borderRadius: 6, background: plus ? 'var(--bad)' : 'var(--accent)' }} />
                            </div>
                            <span className="mono">{c.current}</span>
                          </div>
                        )}
                      </td>
                      <td className="mono">{plus === null ? '–' : plus > 0 ? <b>+{plus}</b> : '0'}</td>
                      <td className="mono">{c.average ?? '–'}</td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
          {report.notes.length > 0 && (
            <ul className="small muted">
              {report.notes.map((n) => (
                <li key={n}>{n}</li>
              ))}
            </ul>
          )}
          <p className="small muted">
            {t('Read from ECU {ecu}. A few counts now and then can be normal; a steady rise on one cylinder points at its coil, plug, injector or compression.', {
              ecu: report.ecu ?? '?',
            })}
          </p>
        </div>
      )}
    </>
  );
}
