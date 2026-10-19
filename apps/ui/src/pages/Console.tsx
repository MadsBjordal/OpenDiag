import { useEffect, useState } from 'react';
import { call } from '../api';
import { ErrorBox, useAction } from '../components';
import { t } from '../i18n';
import { useApp } from '../state';
import { protocolName } from '../units';

interface Msg {
  ecu: string;
  data: number[];
}
interface Log {
  ms: number;
  dir: 'tx' | 'rx' | 'note';
  text: string;
}

const hex = (d: number[]) => d.map((b) => b.toString(16).toUpperCase().padStart(2, '0')).join(' ');

export default function Console() {
  const { settings, summary } = useApp();
  const [req, setReq] = useState('01 0C');
  const [out, setOut] = useState<string[]>([]);
  const [log, setLog] = useState<Log[]>([]);
  const send = useAction(async () => {
    try {
      const r = await call<Msg[]>('raw', { hex: req });
      setOut((o) => [...o, `> ${req}`, ...r.map((m) => `  ${m.ecu}: ${hex(m.data)}`)]);
    } catch (e) {
      setOut((o) => [...o, `> ${req}`, `  ✗ ${(e as Error).message}`]);
    }
  });
  const loadLog = useAction(async () => setLog(await call<Log[]>('log')));
  useEffect(() => {
    void loadLog.run();
  }, []);
  if (settings?.mode !== 'expert') return <p className="empty">{t('Turn on expert mode in Settings to use the console.')}</p>;
  return (
    <>
      <h1>{t('Expert console')}</h1>
      <p className="muted">{t('Send read-only diagnostic requests (OBD services 01, 02, 03, 06, 07, 09, 0A; UDS read services). Anything that could change the vehicle is refused by the safety policy, here too.')}</p>
      {summary && (
        <p className="small mono">
          {summary.adapter.identity} · {protocolName(summary.protocol)} · ECUs {summary.ecus.map((e) => e.address).join(', ')}
        </p>
      )}
      <div className="row">
        <input type="text" className="mono" value={req} onChange={(e) => setReq(e.target.value)} onKeyDown={(e) => e.key === 'Enter' && send.run()} style={{ flex: 1, minWidth: 200 }} aria-label={t('Request in hex')} />
        <button className="btn primary" onClick={() => send.run()} disabled={!summary?.vehicle_connected}>
          {t('Send')}
        </button>
      </div>
      <ErrorBox error={send.error} />
      <div className="console" style={{ marginTop: 10, minHeight: 80 }}>
        {out.join('\n') || t('Answers appear here.')}
      </div>
      <div className="row between" style={{ marginTop: 18 }}>
        <h2 style={{ margin: 0 }}>{t('Adapter log')}</h2>
        <button className="btn small" onClick={() => loadLog.run()}>
          ↻
        </button>
      </div>
      <p className="muted small">{t('VIN answers are hidden unless you allow them in Settings → Privacy.')}</p>
      <div className="console">{log.map((l) => `${String(l.ms).padStart(7)} ${l.dir === 'tx' ? '→' : l.dir === 'rx' ? '←' : '·'} ${l.text}`).join('\n')}</div>
    </>
  );
}
