import { useEffect, useState } from 'react';
import { call } from '../api';
import { SimBanner } from '../components';
import { t } from '../i18n';
import { Page, useApp } from '../state';
import type { ReportSummary } from '../types';
import { protocolName } from '../units';

const TILES: { page: Page; icon: string; title: string; text: string; needsVehicle?: boolean }[] = [
  { page: 'connect', icon: '🔌', title: 'Connect to Vehicle', text: 'Find your adapter and connect' },
  { page: 'scan', icon: '🩺', title: 'Scan Vehicle', text: 'Read fault codes and status', needsVehicle: true },
  { page: 'live', icon: '📈', title: 'Live Data', text: 'Watch sensors in real time', needsVehicle: true },
  { page: 'codes', icon: '⚠', title: 'Fault Codes', text: 'What the codes mean, clear them' },
  { page: 'guided', icon: '🧭', title: 'Guided Diagnostics', text: 'Step-by-step troubleshooting' },
  { page: 'config', icon: '⚙', title: 'Vehicle Settings', text: 'Personalization (separate, off by default)' },
  { page: 'reports', icon: '📄', title: 'Reports', text: 'Saved scans, export and compare' },
  { page: 'garage', icon: '🚗', title: 'My Garage', text: 'Your vehicles and their history' },
];

export default function Home() {
  const { go, summary, report } = useApp();
  const [reports, setReports] = useState<ReportSummary[]>([]);
  useEffect(() => {
    call<ReportSummary[]>('reports', { vehicle_id: null }).then(setReports).catch(() => {});
  }, []);
  const connected = !!summary?.vehicle_connected;
  const scanned = report?.modules.filter((m) => m.status === 'scanned').length ?? 0;
  return (
    <>
      <h1>{t('OpenDiag')}</h1>
      <p className="muted">{t('Open-source vehicle diagnostics. Connect an OBD adapter, scan, understand, save.')}</p>
      <SimBanner show={!!summary?.simulated} />
      {summary && (
        <div className="card" style={{ marginTop: 14 }}>
          <div className="row between">
            <h2 style={{ margin: 0 }}>{t('Vehicle summary')}</h2>
            <button className="btn small" onClick={() => go('compat')}>
              {t('Compatibility status')}
            </button>
          </div>
          <div className="grid two" style={{ marginTop: 10 }}>
            <dl className="kv">
              <dt>{t('Vehicle')}</dt>
              <dd>{report?.vehicle.vin ? `${report.vehicle.vin.manufacturer ?? t('Unknown make')} · ${report.vehicle.vin.vin}` : t('Not identified yet: run a scan')}</dd>
              <dt>{t('Interface')}</dt>
              <dd>
                {summary.adapter.identity} · {summary.link.name}
              </dd>
              <dt>{t('Protocol')}</dt>
              <dd>{protocolName(summary.protocol)}</dd>
            </dl>
            <dl className="kv">
              <dt>{t('Modules detected')}</dt>
              <dd>{summary.ecus.length}</dd>
              <dt>{t('Modules scanned')}</dt>
              <dd>{report ? scanned : t('not scanned yet')}</dd>
              <dt>{t('Faults')}</dt>
              <dd>{report ? report.dtcs.length : '—'}</dd>
              <dt>{t('Warnings')}</dt>
              <dd>{report ? report.warnings.length + report.communication_errors.length : summary.adapter.notes.length}</dd>
              <dt>{t('Saved reports')}</dt>
              <dd>{reports.length}</dd>
            </dl>
          </div>
          {report && <p className={`banner ${report.health.verdict === 'faults_found' ? 'warn' : report.health.verdict === 'incomplete' ? 'bad' : 'info'}`}>
            <b>{report.health.headline}</b> {report.health.detail}
          </p>}
          {summary.vehicle_error && <p className="banner warn">{summary.vehicle_error}</p>}
        </div>
      )}
      <h2>{t('What would you like to do?')}</h2>
      <div className="grid tiles">
        {TILES.map((tile, i) => {
          const disabled = tile.needsVehicle && !connected;
          return (
            <button
              key={tile.page}
              className={`tile ${i === (connected ? 1 : 0) ? 'primary' : ''}`}
              onClick={() => go(disabled ? 'connect' : tile.page)}
              title={disabled ? t('Connect to a vehicle first') : undefined}
            >
              <span className="ico" aria-hidden="true">
                {tile.icon}
              </span>
              <b>{t(tile.title)}</b>
              <span className="muted small">{disabled ? t('Connect to a vehicle first') : t(tile.text)}</span>
            </button>
          );
        })}
      </div>
    </>
  );
}
