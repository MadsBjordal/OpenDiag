import { useEffect, useMemo, useState } from 'react';
import { call } from '../api';
import { ErrorBox, StatusChip, useAction } from '../components';
import { t } from '../i18n';
import { useApp } from '../state';
import type { Connector, ConnectorCategory, Device } from '../types';

const CATEGORIES: [ConnectorCategory, string, string][] = [
  ['car', '🚗', 'Car or van'],
  ['motorcycle', '🏍', 'Motorcycle or scooter'],
  ['truck', '🚚', 'Truck or bus'],
  ['powersports', '🛷', 'ATV, snowmobile, jet ski'],
  ['marine', '⛵', 'Boat engine'],
  ['agricultural', '🚜', 'Tractor or machine'],
  ['light_electric', '🚲', 'E-bike or e-scooter'],
];

const ELM: Record<Connector['elm327'], [string, string]> = {
  yes: ['ok', 'Works with a normal OBD adapter'],
  with_cable: ['warn', 'Normal OBD adapter + adapter cable'],
  limited: ['warn', 'Only partly with a normal OBD adapter'],
  no: ['bad', 'Needs a different interface'],
  not_needed: ['accent', 'No adapter needed'],
};

const ACCESS: Record<Connector['access'], string> = {
  data_link: 'Data connection',
  blink_codes: 'Blink codes only (no data)',
  data_link_or_blink_codes: 'Data or blink codes (depends on model)',
  dashboard_self_test: 'Codes shown on the dashboard',
  no_diagnostics: 'No electronic diagnostics',
};

const CONF: Record<Connector['confidence'], string> = {
  high: 'Well documented',
  medium: 'Partly documented',
  low: 'Little reliable information',
};

/** Counts long and short flashes and keeps the codes read so far. */
function BlinkReader({ c }: { c: Connector }) {
  const [longs, setLongs] = useState(0);
  const [shorts, setShorts] = useState(0);
  const [codes, setCodes] = useState<string[]>([]);
  const [table, setTable] = useState<{ code: string; meaning: string }[]>([]);
  const [newRow, setNewRow] = useState({ code: '', meaning: '' });
  const save = useAction(async (rows: { code: string; meaning: string }[]) => {
    await call('blink_table_set', { key: c.id, entries: rows });
    setTable(rows);
  });
  useEffect(() => {
    call<{ code: string; meaning: string }[]>('blink_table_get', { key: c.id }).then(setTable).catch(() => {});
  }, [c.id]);
  const finish = () => {
    if (longs === 0 && shorts === 0) return;
    // Many systems flash tens as long and units as short; a plain count is just shorts.
    setCodes([...codes, longs > 0 ? `${longs}${shorts}` : `${shorts}`]);
    setLongs(0);
    setShorts(0);
  };
  const meaning = (code: string) => table.find((r) => r.code === code)?.meaning;
  return (
    <div className="card" style={{ marginTop: 12 }}>
      <h3 style={{ marginTop: 0 }}>{t('Blink code reader')}</h3>
      <ol className="small">
        {c.how_to_read.map((s) => (
          <li key={s}>{s}</li>
        ))}
      </ol>
      {c.warnings.map((w) => (
        <p key={w} className="banner warn small">
          {w}
        </p>
      ))}
      <p className="muted small">{t('Tap once per flash while the light blinks. Press "Code done" at the pause between codes.')}</p>
      <div className="row">
        <button className="btn" onClick={() => setLongs(longs + 1)}>
          {t('Long flash')} ({longs})
        </button>
        <button className="btn" onClick={() => setShorts(shorts + 1)}>
          {t('Short flash')} ({shorts})
        </button>
        <button className="btn primary" onClick={finish}>
          {t('Code done')}
        </button>
        <button className="btn ghost" onClick={() => (setCodes([]), setLongs(0), setShorts(0))}>
          {t('Reset')}
        </button>
      </div>
      {codes.length > 0 && (
        <table style={{ marginTop: 10 }}>
          <tbody>
            {codes.map((code, i) => (
              <tr key={i}>
                <td className="mono">
                  <b>{code}</b>
                </td>
                <td>{meaning(code) ?? <span className="muted">{t('Not in your table yet: look it up in the service manual for your model.')}</span>}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      <details style={{ marginTop: 10 }}>
        <summary>
          {t('Your code table')} ({table.length})
        </summary>
        <p className="muted small">
          {t('OpenDiag does not ship blink-code meanings: they differ per model and must come from the manufacturer\'s documentation. Copy the table from your service manual once; it is saved on this device only.')}
        </p>
        <table>
          <tbody>
            {table.map((r) => (
              <tr key={r.code}>
                <td className="mono">{r.code}</td>
                <td>{r.meaning}</td>
                <td>
                  <button className="btn small ghost" onClick={() => save.run(table.filter((x) => x.code !== r.code))}>
                    ✕
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
        <div className="row" style={{ marginTop: 6 }}>
          <input type="text" placeholder={t('Code')} value={newRow.code} onChange={(e) => setNewRow({ ...newRow, code: e.target.value })} style={{ width: 100 }} />
          <input type="text" placeholder={t('Meaning (from your manual)')} value={newRow.meaning} onChange={(e) => setNewRow({ ...newRow, meaning: e.target.value })} style={{ flex: 1, minWidth: 180 }} />
          <button
            className="btn small"
            disabled={!newRow.code.trim() || !newRow.meaning.trim()}
            onClick={() => {
              void save.run([...table.filter((x) => x.code !== newRow.code.trim()), { code: newRow.code.trim(), meaning: newRow.meaning.trim() }]);
              setNewRow({ code: '', meaning: '' });
            }}
          >
            {t('Add')}
          </button>
        </div>
        <ErrorBox error={save.error} />
      </details>
    </div>
  );
}

interface WakeResult {
  awake: boolean;
  wake_reply: string;
  init_reply: string;
}

/** Experimental, read-only Honda motorcycle K-line mode (expert mode only). */
function HondaPanel() {
  const [devices, setDevices] = useState<Device[]>([]);
  const [address, setAddress] = useState('');
  const [variant, setVariant] = useState<'ff_ff' | 'ping72'>('ff_ff');
  const [table, setTable] = useState('17');
  const [connected, setConnected] = useState(false);
  const [lines, setLines] = useState<string[]>([]);
  const add = (l: string) => setLines((x) => [...x, l]);
  const load = useAction(async () => {
    const d = (await call<Device[]>('devices')).filter((x) => x.address.endsWith(':honda') || x.kind === 'usb_serial' || x.kind === 'android_usb');
    setDevices(d);
    if (!address && d[0]) setAddress(d[0].address);
  });
  const open = useAction(async () => {
    const a = address.startsWith('kline-sim:') ? address : `kline:${address.replace(/^serial:/, '')}`;
    await call('honda_connect', { address: a });
    setConnected(true);
    add(`${t('Opened')} ${a}`);
  });
  const wake = useAction(async () => {
    const r = await call<WakeResult>('honda_wake', { variant });
    add(`${t('Wake-up')} (${variant}): ${r.awake ? t('ECU answered') : t('no valid answer')} · ${t('reply')} "${r.wake_reply}" / "${r.init_reply}"`);
  });
  const read = useAction(async () => {
    const n = table.trim().toLowerCase().startsWith('0x') ? parseInt(table, 16) : parseInt(table, 10);
    const r = await call<{ hex: string; simulated: boolean }>('honda_read_table', { table: n });
    add(`${t('Table')} ${n} (0x${n.toString(16).toUpperCase()}): ${r.hex}${r.simulated ? ' [SIMULATED]' : ''}`);
  });
  const close = useAction(async () => {
    await call('honda_disconnect');
    setConnected(false);
  });
  useEffect(() => {
    void load.run();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  const err = open.error ?? wake.error ?? read.error ?? close.error ?? load.error;
  return (
    <div className="card" style={{ marginTop: 12 }}>
      <h3 style={{ marginTop: 0 }}>{t('Experimental: Honda K-line (read only)')}</h3>
      <p className="small">
        {t('Needs a raw K-line cable (FTDI-type) wired to the red DLC. Based on open-source projects, not Honda documentation, and not yet tested on a real bike by OpenDiag. Only the wake-up and table reads can be sent; table bytes are shown raw because their meaning differs per model.')}
      </p>
      <div className="row">
        <select value={address} onChange={(e) => setAddress(e.target.value)} style={{ flex: 1, minWidth: 220 }} aria-label={t('Cable')}>
          {devices.map((d) => (
            <option key={d.address} value={d.address}>
              {d.name}
            </option>
          ))}
        </select>
        <button className="btn small" disabled={!address || connected} onClick={() => open.run()}>
          {t('Open cable')}
        </button>
      </div>
      <div className="row" style={{ marginTop: 8 }}>
        <select value={variant} onChange={(e) => setVariant(e.target.value as typeof variant)} style={{ maxWidth: 260 }} aria-label={t('Wake-up message')}>
          <option value="ff_ff">{t('Wake-up FE 04 FF FF')}</option>
          <option value="ping72">{t('Wake-up FE 04 72 8C')}</option>
        </select>
        <button className="btn small primary" disabled={!connected} onClick={() => wake.run()}>
          {t('Wake up')}
        </button>
        <input type="text" value={table} onChange={(e) => setTable(e.target.value)} style={{ width: 90 }} aria-label={t('Table number')} />
        <button className="btn small" disabled={!connected} onClick={() => read.run()}>
          {t('Read table')}
        </button>
        <button className="btn small ghost" disabled={!connected} onClick={() => close.run()}>
          {t('Close')}
        </button>
      </div>
      <p className="muted small">{t('Sources disagree on the wake-up message: if one gets no answer, try the other. The ECU goes back to sleep after a few seconds: wake it again before reading.')}</p>
      <ErrorBox error={err} />
      {lines.length > 0 && <div className="console" style={{ marginTop: 8 }}>{lines.join('\n')}</div>}
    </div>
  );
}

function ConnectorCard({ c, open, onOpen }: { c: Connector; open: boolean; onOpen: () => void }) {
  const { go, settings } = useApp();
  const [cls, label] = ELM[c.elm327];
  const blink = c.access === 'blink_codes' || c.access === 'dashboard_self_test' || c.access === 'data_link_or_blink_codes';
  return (
    <div className="card">
      <div className="row between">
        <div>
          <b style={{ fontSize: 17 }}>{c.name}</b>
          <div className="muted small">
            {c.makes.map((m) => (m === '*' ? t('other makes') : m)).join(', ')} · {c.applies_to}
          </div>
        </div>
        <StatusChip status={c.support} />
      </div>
      <div className="row small" style={{ marginTop: 8 }}>
        <span className="chip">{c.pins ? t('{n} pins', { n: c.pins }) : t('pin count unknown')}</span>
        <span className="chip">{t(ACCESS[c.access])}</span>
        <span className={`chip ${cls}`}>{t(label)}</span>
        {c.voltage && <span className="chip">{c.voltage}</span>}
        <span className={`chip ${c.confidence === 'high' ? 'ok' : c.confidence === 'medium' ? '' : 'warn'}`}>{t(CONF[c.confidence])}</span>
      </div>
      <p className="small">{c.description}</p>
      <p className="small">
        <b>{t('You need')}:</b> {c.adapter}
      </p>
      <p className="small">
        <b>{t('OpenDiag today')}:</b> {c.support_note}
      </p>
      <div className="row">
        <button className="btn small" onClick={onOpen}>
          {open ? t('Less') : t('Details and sources')}
        </button>
        {c.access === 'no_diagnostics' && (
          <button className="btn small primary" onClick={() => go('guided')}>
            🧭 {t('Guided Diagnostics')}
          </button>
        )}
        {c.elm327 === 'yes' || c.elm327 === 'with_cable' ? (
          <button className="btn small primary" onClick={() => go('connect')}>
            {t('Connect')}
          </button>
        ) : null}
      </div>
      {open && (
        <div style={{ marginTop: 10 }}>
          {c.protocols.length > 0 && (
            <p className="small">
              <b>{t('Protocols')}:</b> {c.protocols.join(' · ')}
            </p>
          )}
          {c.pinout.length > 0 ? (
            <table className="small">
              <thead>
                <tr>
                  <th>{t('Pin')}</th>
                  <th>{t('Signal (as the sources give it)')}</th>
                </tr>
              </thead>
              <tbody>
                {c.pinout.map((p) => (
                  <tr key={p.pin}>
                    <td className="mono">{p.pin}</td>
                    <td>{p.signal}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          ) : (
            <p className="muted small">{t('No reliably sourced pin layout yet. Check the wiring diagram for your model before connecting anything.')}</p>
          )}
          {c.warnings.length > 0 && !blink && (
            <ul className="tight small">
              {c.warnings.map((w) => (
                <li key={w}>{w}</li>
              ))}
            </ul>
          )}
          {c.notes.length > 0 && (
            <ul className="tight small muted">
              {c.notes.map((n) => (
                <li key={n}>{n}</li>
              ))}
            </ul>
          )}
          <p className="small">
            <b>{t('Sources')}:</b>
          </p>
          <ul className="tight small">
            {c.sources.map((s) => (
              <li key={s.url}>
                <a href={s.url} target="_blank" rel="noreferrer">
                  {s.url}
                </a>{' '}
                {s.note && <span className="muted">· {s.note}</span>}
              </li>
            ))}
          </ul>
          {settings?.mode === 'expert' && <p className="mono small muted">id: {c.id}</p>}
          {blink && c.how_to_read.length > 0 && <BlinkReader c={c} />}
          {c.id === 'honda-moto-4pin-dlc' &&
            (settings?.mode === 'expert' ? (
              <HondaPanel />
            ) : (
              <p className="banner info small">{t('Turn on expert mode in Settings to try the experimental Honda K-line data mode.')}</p>
            ))}
        </div>
      )}
    </div>
  );
}

export default function Connectors() {
  const { arg } = useApp();
  const [category, setCategory] = useState<ConnectorCategory | null>((arg as ConnectorCategory) || null);
  const [all, setAll] = useState<Connector[]>([]);
  const [make, setMake] = useState('');
  const [q, setQ] = useState('');
  const [open, setOpen] = useState<string | null>(null);
  const load = useAction(async () => setAll(await call<Connector[]>('connectors', { category: null, make: null })));
  useEffect(() => {
    void load.run();
  }, []);
  const makes = useMemo(
    () => [...new Set(all.filter((c) => !category || c.category === category).flatMap((c) => c.makes).filter((m) => m !== '*'))].sort(),
    [all, category],
  );
  const shown = all
    .filter((c) => !category || c.category === category)
    .filter((c) => !make || c.makes.includes(make) || c.makes.includes('*'))
    .filter((c) => !q || `${c.name} ${c.makes.join(' ')} ${c.applies_to} ${c.description}`.toLowerCase().includes(q.toLowerCase()))
    .sort((a, b) => Number(a.makes.includes('*')) - Number(b.makes.includes('*')) || a.name.localeCompare(b.name));
  return (
    <>
      <h1>{t('Which connector does my vehicle have?')}</h1>
      <p className="muted">
        {t('Vehicles use many different diagnostic sockets. Find yours to see what it can do, which adapter or cable you need, and what OpenDiag supports today.')}
      </p>
      <ErrorBox error={load.error} />
      <div className="grid tiles" style={{ marginTop: 10 }}>
        {CATEGORIES.map(([k, icon, label]) => (
          <button key={k} className={`tile ${category === k ? 'primary' : ''}`} onClick={() => (setCategory(category === k ? null : k), setMake(''))}>
            <span className="ico" aria-hidden="true">
              {icon}
            </span>
            <b>{t(label)}</b>
            <span className="muted small">{t('{n} connector types', { n: all.filter((c) => c.category === k).length })}</span>
          </button>
        ))}
      </div>
      <div className="row" style={{ marginTop: 14 }}>
        <select value={make} onChange={(e) => setMake(e.target.value)} style={{ maxWidth: 260 }} aria-label={t('Make')}>
          <option value="">{t('All makes')}</option>
          {makes.map((m) => (
            <option key={m} value={m}>
              {m}
            </option>
          ))}
        </select>
        <input type="search" placeholder={t('Search, e.g. "4-pin" or "Africa Twin"')} value={q} onChange={(e) => setQ(e.target.value)} style={{ flex: 1, minWidth: 200 }} />
      </div>
      <p className="muted small">
        {t('Not sure? Look where the manual says the diagnostic connector is, count its pins and compare with the descriptions below. Never connect anything until you are sure: some sockets carry 24 V or have unusual pins.')}
      </p>
      <div className="stack" style={{ marginTop: 10 }}>
        {shown.map((c) => (
          <ConnectorCard key={c.id} c={c} open={open === c.id} onOpen={() => setOpen(open === c.id ? null : c.id)} />
        ))}
        {shown.length === 0 && !load.busy && <p className="empty">{t('Nothing found. Your vehicle may use a connector we have not documented yet: please open a "Vehicle definition" issue with a photo.')}</p>}
      </div>
    </>
  );
}
