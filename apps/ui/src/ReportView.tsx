import { useState } from 'react';
import { call, saveFile } from './api';
import { ErrorBox, Kv, SimBanner, useAction } from './components';
import { DtcCard } from './DtcCard';
import { CommunityCodes, ShareCard } from './Share';
import { t } from './i18n';
import { useApp } from './state';
import type { Report } from './types';
import { format, protocolName } from './units';

export function ExportButtons({ id }: { id: number }) {
  const [includeVin, setIncludeVin] = useState(false);
  const [saved, setSaved] = useState<string | null>(null);
  const exp = useAction(async (fmt: string) => {
    const f = await call<{ filename: string; base64: string }>('export_report', { id, format: fmt, include_vin: includeVin });
    setSaved(await saveFile(f));
  });
  return (
    <div className="card">
      <h3 style={{ marginTop: 0 }}>{t('Save a copy')}</h3>
      <p className="muted small">
        {t('The file contains the vehicle identification, adapter details, fault codes, measurements and your notes. The VIN is masked unless you tick the box. Nothing is sent anywhere.')}
      </p>
      <label className="check small">
        <input type="checkbox" checked={includeVin} onChange={(e) => setIncludeVin(e.target.checked)} />
        {t('Include the full VIN')}
      </label>
      <div className="row" style={{ marginTop: 10 }}>
        {['pdf', 'json', 'csv', 'html'].map((f) => (
          <button key={f} className="btn small" disabled={exp.busy} onClick={() => exp.run(f)}>
            {f.toUpperCase()}
          </button>
        ))}
      </div>
      {saved && <p className="small banner ok">{t('Saved: {f}', { f: saved })}</p>}
      <ErrorBox error={exp.error} />
    </div>
  );
}

export function ReportView({ report }: { report: Report }) {
  const { settings } = useApp();
  const units = settings?.units ?? 'metric';
  const expert = settings?.mode === 'expert';
  const [notes, setNotes] = useState(report.notes);
  const save = useAction(async () => call('report_notes', { id: Number(report.id), notes }));
  const v = report.vehicle;
  const verdictCls = report.health.verdict === 'faults_found' ? 'warn' : report.health.verdict === 'incomplete' ? 'bad' : 'info';
  const scanned = report.modules.filter((m) => m.status !== 'not_scanned');
  const notScanned = report.modules.filter((m) => m.status === 'not_scanned');
  return (
    <div className="stack">
      <SimBanner show={report.simulated} />
      <div className={`banner ${verdictCls}`}>
        <b style={{ fontSize: 17 }}>{report.health.headline}</b>
        <div>{report.health.detail}</div>
      </div>
      <div className="grid two">
        <div className="card">
          <h3 style={{ marginTop: 0 }}>{t('Vehicle')}</h3>
          <Kv
            items={[
              [t('VIN'), v.vin ? <span className="mono">{v.vin.vin}</span> : t('not reported')],
              [t('Make (from VIN)'), v.vin?.manufacturer ?? t('unknown')],
              [t('Model year (from VIN)'), v.vin?.model_years.length ? v.vin.model_years.join(t(' or ')) : '—'],
              [t('OBD standard'), v.obd_standard ?? '—'],
              [t('Fuel'), v.fuel_type ?? '—'],
            ]}
          />
          <p className="muted small">{v.note}</p>
        </div>
        <div className="card">
          <h3 style={{ marginTop: 0 }}>{t('Adapter and connection')}</h3>
          <Kv
            items={[
              [t('Adapter'), report.adapter.identity],
              [t('Connection'), report.link.name],
              [t('Protocol'), protocolName(report.protocol)],
              [t('Scanned'), new Date(report.created_at).toLocaleString()],
              [t('App'), `${report.app} ${report.app_version} · ${report.platform}`],
            ]}
          />
        </div>
      </div>
      <div className="card">
        <h3 style={{ marginTop: 0 }}>{t('Modules')}</h3>
        <ul className="tight">
          {scanned.map((m) => (
            <li key={m.label}>
              <span className={`chip ${m.status === 'scanned' ? 'ok' : 'warn'}`}>{m.status === 'scanned' ? t('Scanned') : t('Partly scanned')}</span> {m.label}{' '}
              <span className="muted small">{m.detail}</span>
            </li>
          ))}
        </ul>
        <details>
          <summary>
            {t('Not scanned')} ({notScanned.length})
          </summary>
          <p className="muted small">{notScanned[0]?.detail}</p>
          <p>{notScanned.map((m) => m.label).join(' · ')}</p>
        </details>
      </div>
      <h2>{t('Fault codes')}</h2>
      {report.dtcs.length === 0 ? (
        <p className="empty">{t('No fault codes reported by the modules that were scanned.')}</p>
      ) : (
        report.dtcs.map((d) => <DtcCard key={d.code + d.kind + d.ecu} finding={d} observed={report.measurements} />)
      )}
      <CommunityCodes report={report} />
      {(report.warnings.length > 0 || report.communication_errors.length > 0) && (
        <div className="card">
          <h3 style={{ marginTop: 0 }}>{t('Warnings and communication problems')}</h3>
          <ul className="tight">
            {report.warnings.map((w) => (
              <li key={w}>{w}</li>
            ))}
            {report.communication_errors.map((w) => (
              <li key={w} style={{ color: 'var(--bad)' }}>
                {w}
              </li>
            ))}
          </ul>
        </div>
      )}
      <div className="grid two">
        {report.readiness.map(([ecu, r]) => (
          <div key={ecu} className="card">
            <h3 style={{ marginTop: 0 }}>
              {t('Readiness monitors')} {expert && <span className="mono muted">{ecu}</span>}
            </h3>
            <p>
              {t('Check engine light')}: <b>{r.mil_on ? t('ON') : t('off')}</b> · {t('{n} stored code(s) reported', { n: r.dtc_count })}
            </p>
            <table>
              <tbody>
                {r.monitors
                  .filter((m) => m.available)
                  .map((m) => (
                    <tr key={m.name}>
                      <td>{m.name}</td>
                      <td>{m.complete ? <span className="chip ok">{t('Complete')}</span> : <span className="chip warn">{t('Not complete')}</span>}</td>
                    </tr>
                  ))}
              </tbody>
            </table>
          </div>
        ))}
        {report.freeze_frames.map((f) => (
          <div key={f.ecu} className="card">
            <h3 style={{ marginTop: 0 }}>{t('Freeze frame: conditions when {c} was stored', { c: f.dtc ?? '?' })}</h3>
            <table>
              <tbody>
                {f.values.map((m) => (
                  <tr key={m.key}>
                    <td>{m.name}</td>
                    <td>{format(m.value, m.unit, units)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        ))}
      </div>
      {report.measurements.length > 0 && (
        <div className="card">
          <h3 style={{ marginTop: 0 }}>{t('Measurements at scan time')}</h3>
          <div className="table-wrap">
            <table>
              <tbody>
                {report.measurements.map((m) => (
                  <tr key={m.key}>
                    <td>{m.name}</td>
                    <td>{format(m.value, m.unit, units)}</td>
                    <td className="muted small">{m.source === 'adapter' ? t('measured by the adapter') : t('reported by the vehicle')}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}
      {report.operations.length > 0 && (
        <div className="card">
          <h3 style={{ marginTop: 0 }}>{t('Operations performed in this session')}</h3>
          <ul className="tight">
            {report.operations.map((o) => (
              <li key={o.at + o.what}>
                {new Date(o.at).toLocaleString()} · {o.what}: {o.detail}
              </li>
            ))}
          </ul>
        </div>
      )}
      <div className="card">
        <h3 style={{ marginTop: 0 }}>{t('Notes')}</h3>
        <textarea value={notes} onChange={(e) => setNotes(e.target.value)} placeholder={t('What you checked, repairs done, observations…')} />
        <div className="row" style={{ marginTop: 8 }}>
          <button className="btn small" onClick={() => save.run()} disabled={save.busy || notes === report.notes}>
            {t('Save notes')}
          </button>
          <ErrorBox error={save.error} />
        </div>
      </div>
      <ExportButtons id={Number(report.id)} />
      <ShareCard reportId={Number(report.id)} simulated={report.simulated} />
      <div className="card">
        <h3 style={{ marginTop: 0 }}>{t('Important')}</h3>
        <ul className="tight small">
          {report.cautions.map((c) => (
            <li key={c}>{c}</li>
          ))}
        </ul>
      </div>
    </div>
  );
}
