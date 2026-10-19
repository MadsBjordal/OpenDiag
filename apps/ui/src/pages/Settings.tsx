import { useState } from 'react';
import { call } from '../api';
import { ErrorBox, useAction } from '../components';
import { LANGUAGES, t } from '../i18n';
import { NOTICE, NOTICE_TITLE, PRIVACY } from '../notice';
import { useApp } from '../state';
import type { Settings } from '../types';

export default function SettingsPage() {
  const { settings, setSettings } = useApp();
  const [authority, setAuthority] = useState(false);
  const [understood, setUnderstood] = useState(false);
  const set = useAction(async (key: string, value: string) => setSettings(await call<Settings>('set_setting', { key, value })));
  const terms = useAction(async (accept: boolean) => setSettings(await call<Settings>('accept_terms', { accept })));
  const wipe = useAction(async () => {
    if (!confirm(t('Delete every vehicle, report and log on this device? This cannot be undone.'))) return;
    await call('delete_all_data');
    alert(t('All vehicle data was deleted.'));
  });
  if (!settings) return null;
  const select = (key: keyof Settings, options: [string, string][], label: string, help?: string) => (
    <label className="field">
      {t(label)}
      <select value={String(settings[key])} onChange={(e) => set.run(key, e.target.value)}>
        {options.map(([v, l]) => (
          <option key={v} value={v}>
            {t(l)}
          </option>
        ))}
      </select>
      {help && <span className="muted small" style={{ fontWeight: 400 }}>{t(help)}</span>}
    </label>
  );
  return (
    <>
      <h1>{t('Settings')}</h1>
      <ErrorBox error={set.error ?? terms.error ?? wipe.error} />
      <div className="card">
        <h2 style={{ marginTop: 0 }}>{t('Display')}</h2>
        <div className="grid two">
          {select('mode', [['beginner', 'Beginner'], ['expert', 'Expert']], 'Mode', 'Expert mode shows ECU addresses, raw values, the adapter log and the read-only console. It never skips safety checks.')}
          {select('units', [['metric', 'Metric (°C, km/h, kPa)'], ['imperial', 'Imperial (°F, mph, psi)']], 'Units')}
          {select('theme', [['system', 'Follow the system'], ['light', 'Light'], ['dark', 'Dark']], 'Theme')}
          {select('language', LANGUAGES.map((l) => [l.code, l.name]), 'Language', 'More languages can be added by translators (apps/ui/src/i18n.ts).')}
        </div>
      </div>
      <div className="card">
        <h2 style={{ marginTop: 0 }}>{t(NOTICE_TITLE)}</h2>
        {NOTICE.map((p) => (
          <p key={p} className="small">
            {t(p)}
          </p>
        ))}
        {settings.terms_accepted ? (
          <div className="banner ok">
            {t('You accepted this notice (version {v}). Functions that change the vehicle are available, with their own checks.', { v: settings.terms_version })}{' '}
            <button className="btn small" onClick={() => terms.run(false)}>
              {t('Withdraw')}
            </button>
          </div>
        ) : (
          <div className="stack">
            <label className="check">
              <input type="checkbox" checked={understood} onChange={(e) => setUnderstood(e.target.checked)} />
              {t('I have read and understood the notice and the risks.')}
            </label>
            <label className="check">
              <input type="checkbox" checked={authority} onChange={(e) => setAuthority(e.target.checked)} />
              {t('I will only change vehicles I own or am allowed to work on.')}
            </label>
            <button className="btn primary" disabled={!authority || !understood} onClick={() => terms.run(true)}>
              {t('Accept for vehicle-changing functions')}
            </button>
            <p className="muted small">{t('Reading diagnostics never needs this.')}</p>
          </div>
        )}
      </div>
      <div className="card">
        <h2 style={{ marginTop: 0 }}>{t('Vehicle configuration')}</h2>
        <p className="small">{t('A separate subsystem for personalization and service settings. Off by default. Even when on, only verified operations for your exact ECU can run, and none exist yet.')}</p>
        {select('configuration_enabled', [['false', 'Off'], ['true', 'On']], 'Vehicle configuration')}
      </div>
      <div className="card">
        <h2 style={{ marginTop: 0 }}>{t('Privacy')}</h2>
        <ul className="tight small">
          {PRIVACY.map((p) => (
            <li key={p}>{t(p)}</li>
          ))}
        </ul>
        {select('show_vin_in_log', [['false', 'Hidden'], ['true', 'Shown']], 'VIN in the adapter log')}
        <button className="btn danger" style={{ marginTop: 12 }} onClick={() => wipe.run()}>
          {t('Delete all vehicle data on this device')}
        </button>
      </div>
      <div className="card">
        <h2 style={{ marginTop: 0 }}>{t('About')}</h2>
        <p className="small">
          OpenDiag 0.1.0 · {settings.platform} · {t('Licensed under the GNU General Public License v3.0 or later.')}{' '}
          <a href="https://github.com/MadsBjordal/OpenDiag" target="_blank" rel="noreferrer">
            {t('Source code')}
          </a>
        </p>
      </div>
    </>
  );
}
