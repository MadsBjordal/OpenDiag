import { useEffect, useMemo, useRef, useState } from 'react';
import { call, saveFile, textFile } from '../api';
import { ErrorBox, SimBanner, Sparkline } from '../components';
import { t } from '../i18n';
import { useApp } from '../state';
import type { LiveParam, LiveSample } from '../types';
import { convert, format, liveCsv } from '../units';

const HISTORY = 120;
const RATES = [250, 500, 1000, 2000];

export default function Live() {
  const { summary, settings, arg, go } = useApp();
  const units = settings?.units ?? 'metric';
  const [params, setParams] = useState<LiveParam[]>([]);
  const [chosen, setChosen] = useState<string[]>([]);
  const [running, setRunning] = useState(false);
  const [rate, setRate] = useState(500);
  const [latest, setLatest] = useState<Record<string, LiveSample>>({});
  const [history, setHistory] = useState<Record<string, number[]>>({});
  const [error, setError] = useState<unknown>(null);
  const recording = useRef<{ t: number; values: Record<string, number | null> }[]>([]);
  const [recCount, setRecCount] = useState(0);
  const [rec, setRec] = useState(false);

  useEffect(() => {
    if (!summary?.vehicle_connected) return;
    call<LiveParam[]>('live_params')
      .then((p) => {
        setParams(p);
        const wanted = arg ? arg.split(',') : p.filter((x) => x.common).map((x) => x.key);
        setChosen(wanted.filter((k) => p.some((x) => x.key === k)).slice(0, 8));
      })
      .catch(setError);
  }, [summary?.vehicle_connected, arg]);

  useEffect(() => {
    if (!running || chosen.length === 0) return;
    let stop = false;
    const loop = async () => {
      while (!stop) {
        const start = Date.now();
        try {
          const samples = await call<LiveSample[]>('live_read', { keys: chosen });
          setLatest(Object.fromEntries(samples.map((s) => [s.key, s])));
          setHistory((h) => {
            const n = { ...h };
            for (const s of samples) if (s.value !== null) n[s.key] = [...(n[s.key] ?? []), s.value].slice(-HISTORY);
            return n;
          });
          if (rec) {
            recording.current.push({ t: Date.now(), values: Object.fromEntries(samples.map((s) => [s.key, s.value])) });
            setRecCount(recording.current.length);
          }
          setError(null);
        } catch (e) {
          setError(e);
          setRunning(false);
          return;
        }
        // The engine already spaces requests; this sets the refresh rate.
        await new Promise((r) => setTimeout(r, Math.max(0, rate - (Date.now() - start))));
      }
    };
    void loop();
    return () => {
      stop = true;
    };
  }, [running, chosen, rate, rec]);

  const byKey = useMemo(() => Object.fromEntries(params.map((p) => [p.key, p])), [params]);
  if (!summary?.vehicle_connected) {
    return (
      <>
        <h1>{t('Live Data')}</h1>
        <p className="empty">{t('Connect to a vehicle first.')}</p>
        <div className="row" style={{ justifyContent: 'center' }}>
          <button className="btn primary" onClick={() => go('connect')}>
            {t('Connect to Vehicle')}
          </button>
        </div>
      </>
    );
  }
  const toggle = (k: string) => setChosen((c) => (c.includes(k) ? c.filter((x) => x !== k) : c.length >= 12 ? c : [...c, k]));
  const exportCsv = async () => {
    const keys = chosen.map((k) => ({ key: k, name: byKey[k]?.name ?? k, unit: byKey[k]?.unit ?? '' }));
    await saveFile(textFile(`opendiag-live-${new Date().toISOString().replace(/[:.]/g, '-')}${summary.simulated ? '-SIMULATED' : ''}.csv`, liveCsv(keys, recording.current)));
  };
  return (
    <>
      <div className="row between">
        <h1>{t('Live Data')}</h1>
        <div className="row">
          <label className="row small">
            {t('Refresh')}
            <select value={rate} onChange={(e) => setRate(Number(e.target.value))} style={{ width: 110 }}>
              {RATES.map((r) => (
                <option key={r} value={r}>
                  {r} ms
                </option>
              ))}
            </select>
          </label>
          <button className={`btn ${running ? '' : 'primary'}`} onClick={() => setRunning(!running)} disabled={chosen.length === 0}>
            {running ? '⏸ ' + t('Pause') : '▶ ' + t('Start')}
          </button>
        </div>
      </div>
      <SimBanner show={summary.simulated} />
      <p className="muted small">
        {t('Values are read one after another; more values means each one updates less often. The engine spaces requests so the adapter and vehicle bus are not overloaded.')}
      </p>
      <ErrorBox error={error} />
      <div className="gauge-grid">
        {chosen.map((k) => {
          const p = byKey[k];
          const s = latest[k];
          const hist = (history[k] ?? []).map((v) => convert(v, p?.unit ?? '', units).value);
          return (
            <div key={k} className="gauge">
              <div className="row between small">
                <span>{p?.name ?? k}</span>
                {p && p.source !== 'vehicle' && <span className="chip">{p.source === 'adapter' ? t('adapter') : t('calculated')}</span>}
              </div>
              <div className="val">{s?.error ? <span className="small muted">{s.error}</span> : format(s?.value ?? null, p?.unit ?? '', units)}</div>
              <Sparkline values={hist} />
            </div>
          );
        })}
      </div>
      <div className="card" style={{ marginTop: 14 }}>
        <div className="row between">
          <h3 style={{ margin: 0 }}>{t('Record')}</h3>
          <div className="row">
            <button className="btn small" onClick={() => setRec(!rec)}>
              {rec ? '■ ' + t('Stop recording') : '● ' + t('Record')}
            </button>
            <button className="btn small" disabled={recCount === 0} onClick={exportCsv}>
              {t('Export CSV ({n} rows)', { n: recCount })}
            </button>
            <button
              className="btn small ghost"
              disabled={recCount === 0}
              onClick={() => {
                recording.current = [];
                setRecCount(0);
              }}
            >
              {t('Discard')}
            </button>
          </div>
        </div>
      </div>
      <h2>{t('Choose values ({n}/12)', { n: chosen.length })}</h2>
      <div className="grid tiles">
        {params.map((p) => (
          <label key={p.key} className="check card" style={{ padding: 10 }}>
            <input type="checkbox" checked={chosen.includes(p.key)} onChange={() => toggle(p.key)} />
            <span>
              {p.name} <span className="muted small">({convert(0, p.unit, units).unit})</span>
            </span>
          </label>
        ))}
      </div>
    </>
  );
}
