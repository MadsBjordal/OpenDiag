import { useEffect, useState } from 'react';
import { call } from '../api';
import { ErrorBox, useAction } from '../components';
import { t } from '../i18n';
import { useApp } from '../state';
import type { SimSettings } from '../types';

const FAULTS: [keyof SimSettings['faults'], string][] = [
  ['ignition_off', 'Ignition off (vehicle does not answer)'],
  ['no_response', 'Adapter stops answering (timeouts)'],
  ['malformed', 'Malformed answers'],
  ['truncated', 'Long answers cut off'],
  ['low_voltage', 'Low battery voltage (10.6 V)'],
  ['wrong_vin', 'Invalid VIN'],
  ['bus_error', 'Vehicle bus errors'],
  ['clone_adapter', 'Behave like a cheap clone adapter (reconnect to apply)'],
];

export default function Simulator() {
  const { summary, refreshSummary } = useApp();
  const [s, setS] = useState<SimSettings | null>(null);
  const load = useAction(async () => setS(await call<SimSettings>('sim_get')));
  const save = useAction(async (n: SimSettings) => {
    setS(await call<SimSettings>('sim_set', { settings: n }));
    await refreshSummary();
  });
  useEffect(() => {
    void load.run();
  }, []);
  if (!summary?.simulated || !s) return <p className="empty">{t('Connect to a simulated vehicle to use these controls.')}</p>;
  return (
    <>
      <h1>{t('Simulator controls')}</h1>
      <p className="banner sim">{t('These controls change the SIMULATED vehicle only. They exist to test OpenDiag without a car.')}</p>
      <ErrorBox error={save.error} />
      <div className="card stack">
        <label className="check">
          <input type="checkbox" checked={s.engine_running} onChange={(e) => save.run({ ...s, engine_running: e.target.checked })} />
          {t('Engine running')} <span className="muted small">{t('(switch off to test clearing codes: the ignition stays on)')}</span>
        </label>
        {FAULTS.map(([k, label]) => (
          <label key={k} className="check">
            <input type="checkbox" checked={!!s.faults[k]} onChange={(e) => save.run({ ...s, faults: { ...s.faults, [k]: e.target.checked } })} />
            {t(label)}
          </label>
        ))}
        <label className="check">
          <input type="checkbox" checked={s.faults.disconnect_after !== null} onChange={(e) => save.run({ ...s, faults: { ...s.faults, disconnect_after: e.target.checked ? 5 : null } })} />
          {t('Disconnect after 5 more requests')}
        </label>
        <label className="field" style={{ maxWidth: 240 }}>
          {t('Bus latency (ms)')}
          <input type="number" min={0} max={500} value={s.latency_ms} onChange={(e) => save.run({ ...s, latency_ms: Number(e.target.value) })} />
        </label>
      </div>
    </>
  );
}
