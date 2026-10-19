import { useEffect, useState } from 'react';
import { call } from '../api';
import { t } from '../i18n';
import { useApp } from '../state';

const LEVELS = [
  ['0', 'Read only', 'Reading information and diagnostic data.', 'Available'],
  ['1', 'Standard maintenance', 'Clearing supported codes and documented service resets.', 'Clearing codes is available (with checks); service resets are planned.'],
  ['2', 'Verified personalization', 'Low-risk changes documented and verified for an exact ECU revision.', 'No verified operations yet'],
  ['3', 'Advanced service', 'Calibrations, adaptations, component procedures. Needs a verified rollback.', 'No verified operations yet'],
  ['4', 'High risk', 'ECU programming, security access, immobiliser, safety-critical changes.', 'Not available in OpenDiag'],
];

interface Def {
  id: string;
  name: string;
  functions: { id: string; risk_level: number; verification: string }[];
}

export default function Config() {
  const { settings, go } = useApp();
  const [defs, setDefs] = useState<Def[]>([]);
  useEffect(() => {
    call<Def[]>('definitions').then(setDefs).catch(() => {});
  }, []);
  const changing = defs.flatMap((d) => d.functions.filter((f) => f.risk_level >= 2).map((f) => ({ ...f, def: d.name })));
  return (
    <>
      <h1>{t('Vehicle Settings')}</h1>
      <p className="muted">
        {t('Changing how a vehicle behaves (lighting, locking, display…) is a separate part of OpenDiag from diagnostics. Diagnostics never need it.')}
      </p>
      <div className={`banner ${settings?.configuration_enabled ? 'warn' : 'info'}`}>
        {settings?.configuration_enabled ? t('Vehicle configuration is turned on.') : t('Vehicle configuration is turned off (default).')}{' '}
        <button className="btn small" onClick={() => go('settings')}>
          {t('Settings')}
        </button>
      </div>
      <div className="card">
        <h2 style={{ marginTop: 0 }}>{t('Available changes for this vehicle')}</h2>
        {changing.length === 0 ? (
          <p>
            {t('None. OpenDiag only offers a change when a definition for your exact ECU hardware and software has been verified on real hardware, with a tested way back. No such definitions exist yet.')}
          </p>
        ) : (
          <ul>
            {changing.map((f) => (
              <li key={f.id}>
                {f.def}: {f.id} ({f.verification})
              </li>
            ))}
          </ul>
        )}
      </div>
      <h2>{t('Risk levels')}</h2>
      <div className="table-wrap card">
        <table>
          <thead>
            <tr>
              <th>{t('Level')}</th>
              <th>{t('What')}</th>
              <th>{t('Examples')}</th>
              <th>{t('In OpenDiag')}</th>
            </tr>
          </thead>
          <tbody>
            {LEVELS.map(([n, name, ex, status]) => (
              <tr key={n}>
                <td>
                  <b>{n}</b>
                </td>
                <td>{t(name)}</td>
                <td className="small">{t(ex)}</td>
                <td className="small">{t(status)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <h2>{t('Before any change, OpenDiag will')}</h2>
      <ol className="small">
        {[
          'Check the vehicle and the exact ECU hardware/software against a verified definition.',
          'Check the adapter can do it and the connection is reliable.',
          'Check ignition and battery voltage (a battery support unit is recommended).',
          'Explain the consequences and whether a tested way back exists.',
          'Read and save the original setting.',
          'Show the change and ask for your specific confirmation.',
          'Send only that validated request, then read it back to verify.',
          'Record the result and any errors.',
        ].map((s) => (
          <li key={s}>{t(s)}</li>
        ))}
      </ol>
      <p className="muted small">{t('Plugins, expert mode and the raw console cannot skip these checks: every request passes the same safety policy.')}</p>
    </>
  );
}
