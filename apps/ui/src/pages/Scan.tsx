import { useEffect, useRef, useState } from 'react';
import { call, listen } from '../api';
import { ErrorBox, Spinner, useAction } from '../components';
import { t } from '../i18n';
import { ReportView } from '../ReportView';
import { useApp } from '../state';
import type { Report } from '../types';

interface Progress {
  step: number;
  total: number;
  label: string;
}

const STEPS = [
  'Checking the adapter',
  'Checking communication with the vehicle',
  'Identifying the vehicle',
  'Finding control modules',
  'Reading fault codes',
  'Reading readiness monitors',
  'Reading freeze frame data',
  'Reading measurements',
];

export default function Scan() {
  const { summary, report, setReport, go, arg, refreshSummary } = useApp();
  const [progress, setProgress] = useState<Progress | null>(null);
  const started = useRef(false);
  const scan = useAction(async () => {
    setProgress({ step: 0, total: STEPS.length, label: '' });
    const stop = await listen<Progress>('scan_progress', setProgress);
    try {
      const r = await call<Report>('scan');
      setReport(r);
      await refreshSummary();
    } finally {
      stop();
      setProgress(null);
    }
  });
  useEffect(() => {
    if (arg === 'start' && !started.current && summary?.vehicle_connected) {
      started.current = true;
      void scan.run();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [arg, summary]);

  if (!summary?.vehicle_connected) {
    return (
      <>
        <h1>{t('Scan Vehicle')}</h1>
        <p className="empty">{t('Connect to a vehicle first.')}</p>
        <div className="row" style={{ justifyContent: 'center' }}>
          <button className="btn primary" onClick={() => go('connect')}>
            {t('Connect to Vehicle')}
          </button>
        </div>
      </>
    );
  }
  return (
    <>
      <div className="row between">
        <h1>{t('Scan Vehicle')}</h1>
        <button className="btn primary" onClick={() => scan.run()} disabled={scan.busy}>
          {scan.busy ? <Spinner /> : '🩺'} {report ? t('Scan again') : t('Start scan')}
        </button>
      </div>
      <p className="muted">{t('Reads fault codes, readiness, freeze frame and measurements from every module that answers generic OBD-II. Nothing on the vehicle is changed.')}</p>
      {progress && (
        <div className="card" aria-live="polite">
          <div className="progress">
            <div style={{ width: `${(progress.step / progress.total) * 100}%` }} />
          </div>
          <ul className="steps" style={{ marginTop: 10 }}>
            {STEPS.map((s, i) => (
              <li key={s}>
                {i + 1 < progress.step ? '✓' : i + 1 === progress.step ? <Spinner /> : '○'} <span className={i + 1 > progress.step ? 'muted' : ''}>{t(s)}</span>
              </li>
            ))}
          </ul>
        </div>
      )}
      <ErrorBox error={scan.error} />
      {report && !scan.busy && (
        <>
          <div className="row" style={{ margin: '12px 0' }}>
            {report.dtcs.length > 0 && (
              <button className="btn" onClick={() => go('codes')}>
                ⚠ {t('Fault codes and clearing')}
              </button>
            )}
            <button className="btn" onClick={() => go('live')}>
              📈 {t('Live Data')}
            </button>
            <button className="btn" onClick={() => go('reports', report.id)}>
              📄 {t('Saved as report #{id}', { id: report.id })}
            </button>
          </div>
          <ReportView report={report} />
        </>
      )}
    </>
  );
}
