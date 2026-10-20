import { useState } from 'react';
import { call, saveFile, textFile } from '../api';
import { ErrorBox, SimBanner, Spinner, useAction } from '../components';
import { t } from '../i18n';
import { useApp } from '../state';
import type { J1939Code, J1939Lamps, J1939Report } from '../types';

const LAMPS: [keyof J1939Lamps, string][] = [
  ['red_stop', 'Red stop lamp'],
  ['amber_warning', 'Amber warning lamp'],
  ['mil', 'Engine / emissions lamp (MIL)'],
  ['protect', 'Protect lamp'],
];

function Code({ c }: { c: J1939Code }) {
  return (
    <div className="card" style={{ margin: '8px 0', padding: 12 }}>
      <b className="mono">
        SPN {c.spn} · FMI {c.fmi}
      </b>{' '}
      {c.spn_name ? <span>{t(c.spn_name)}</span> : <span className="muted">{t("Parameter not in OpenDiag's list: look up the SPN in the manufacturer's manual.")}</span>}
      <div>{t(c.fmi_text)}</div>
      <div className="small muted">
        {c.occurrences != null && t('Seen {n} time(s).', { n: c.occurrences })} {c.old_conversion && t('This unit uses an older code layout, so the SPN number may be wrong.')}
      </div>
    </div>
  );
}

export default function Truck() {
  const { summary, go, refreshSummary } = useApp();
  const [report, setReport] = useState<J1939Report | null>(null);
  const scan = useAction(async () => {
    const r = await call<J1939Report>('j1939_scan');
    setReport(r);
    await refreshSummary();
    return r;
  });

  if (!summary) {
    return (
      <>
        <h1>{t('Trucks, buses and machines (J1939)')}</h1>
        <p>
          {t(
            'Connect an ELM327 (v1.4b or newer) or STN adapter first. Most trucks and buses have a 9-pin round Deutsch socket: you need a 9-pin to 16-pin cable. Newer vehicles may have a 16-pin socket.',
          )}
        </p>
        <button className="btn primary" onClick={() => go('connect')}>
          {t('Connect to Vehicle')}
        </button>
      </>
    );
  }

  const json = () =>
    JSON.stringify(
      {
        ...report,
        vin: report?.vin ? `${report.vin.slice(0, 11)}******` : null,
      },
      null,
      2,
    );

  return (
    <>
      <h1>{t('Trucks, buses and machines (J1939)')}</h1>
      <p className="muted">
        {t(
          'Reads active and previous fault codes and engine values over SAE J1939 (250 kbit/s). Read only: OpenDiag mostly listens, and only asks for codes, VIN, engine hours and distance. Takes about 15 seconds.',
        )}
      </p>
      <SimBanner show={summary.simulated} />
      <p className="small muted">{t('This switches the adapter to J1939. To read a car (OBD-II) afterwards, reconnect.')}</p>
      <button className="btn primary" disabled={scan.busy} onClick={() => scan.run()}>
        {scan.busy ? <Spinner /> : '🚛'} {scan.busy ? t('Listening to the vehicle…') : report ? t('Read again') : t('Read the vehicle')}
      </button>
      <ErrorBox error={scan.error} />

      {report && (
        <>
          {report.notes.map((n) => (
            <p key={n} className="banner warn">
              {t(n)}
            </p>
          ))}
          {report.vin && (
            <p>
              {t('VIN')}: <span className="mono">{report.vin}</span>
            </p>
          )}
          {report.modules.map((m) => {
            const lit = m.lamps ? LAMPS.filter(([k]) => m.lamps![k] === 'on') : [];
            return (
              <div key={m.address} className="card">
                <h2 style={{ marginTop: 0 }}>
                  {t(m.name)} <span className="muted small mono">SA {m.address}</span>
                </h2>
                {lit.length > 0 && (
                  <p>
                    {lit.map(([k, label]) => (
                      <span key={k} className={`chip ${k === 'red_stop' ? 'bad' : 'warn'}`} style={{ marginRight: 6 }}>
                        {t(label)}
                      </span>
                    ))}
                  </p>
                )}
                <h3>{t('Active fault codes')}</h3>
                {m.lamps == null ? (
                  <p className="muted">{t('Did not report.')}</p>
                ) : m.active.length === 0 ? (
                  <p className="chip ok">{t('None')}</p>
                ) : (
                  m.active.map((c) => <Code key={`${c.spn}-${c.fmi}`} c={c} />)
                )}
                <h3>{t('Previous fault codes')}</h3>
                {m.previous == null ? (
                  <p className="muted">{t('Did not answer.')}</p>
                ) : m.previous.length === 0 ? (
                  <p className="chip ok">{t('None')}</p>
                ) : (
                  m.previous.map((c) => <Code key={`${c.spn}-${c.fmi}`} c={c} />)
                )}
              </div>
            );
          })}
          {report.values.length > 0 && (
            <div className="card">
              <h2 style={{ marginTop: 0 }}>{t('Engine values')}</h2>
              <table>
                <tbody>
                  {report.values.map((v) => (
                    <tr key={`${v.key}-${v.source}`}>
                      <td>{t(v.name)}</td>
                      <td className="mono">
                        {v.value != null ? `${Number(v.value.toFixed(v.value >= 100 ? 1 : 2))} ${v.unit}` : <span className="muted">{t(v.status ?? 'not available')}</span>}
                      </td>
                      <td className="small muted">
                        SPN {v.spn} · SA {v.source}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
          <div className="row">
            <button className="btn" onClick={() => saveFile(textFile('opendiag-j1939.json', json()))}>
              {t('Save as file (VIN partly hidden)')}
            </button>
            <button className="btn" onClick={() => go('connect')}>
              {t('Back to the connection')}
            </button>
          </div>
        </>
      )}
    </>
  );
}
