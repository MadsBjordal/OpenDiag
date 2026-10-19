import { useEffect, useState } from 'react';
import { call } from '../api';
import { ErrorBox, SimBanner, useAction } from '../components';
import { t } from '../i18n';
import { useApp } from '../state';
import type { Answer, GuidedState, LiveSample, Procedure } from '../types';

const SPEC_KIND = {
  documented: 'Documented specification (entered by you)',
  general_guidance: 'General guidance, not a manufacturer value',
  derived: 'Worked out from your earlier measurement',
};

function Run({ proc, onBack }: { proc: Procedure; onBack: () => void }) {
  const { summary } = useApp();
  const [answers, setAnswers] = useState<Answer[]>([]);
  const [state, setState] = useState<GuidedState | null>(null);
  const [value, setValue] = useState('');
  const [spec, setSpec] = useState({ min: '', max: '' });
  const [fromVehicle, setFromVehicle] = useState(false);
  const evaluate = useAction(async (a: Answer[]) => {
    const s = await call<GuidedState>('guided', { id: proc.id, answers: a });
    setState(s);
    if (!s.error) setAnswers(a);
    setValue('');
    setFromVehicle(false);
  });
  const readLive = useAction(async (key: string) => {
    const [s] = await call<LiveSample[]>('live_read', { keys: [key] });
    if (s.value === null) throw new Error(s.error ?? t('No value'));
    setValue(String(Math.round(s.value * 100) / 100));
    setFromVehicle(true);
  });
  useEffect(() => {
    void evaluate.run([]);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [proc.id]);
  const step = state?.current;
  const answer = (a: Omit<Answer, 'step'>) => evaluate.run([...answers, { step: step!.id, ...a }]);
  return (
    <>
      <button className="btn small ghost" onClick={onBack}>
        ← {t('All procedures')}
      </button>
      <h1>{proc.title}</h1>
      <p className="muted small">
        {t('Source')}: {proc.source}
      </p>
      <SimBanner show={!!summary?.simulated} />
      {state?.history.map((h, i) => (
        <div key={i} className="card" style={{ padding: 12 }}>
          <p className="small muted">{h.text}</p>
          {h.choice && <b>{h.choice === 'ok' ? t('Done') : h.choice === 'yes' ? t('Yes') : t('No')}</b>}
          {h.measured !== null && (
            <div className="grid two small">
              <div>
                <span className="chip accent">{t('Measured')}</span>{' '}
                <b>
                  {h.measured} {h.unit}
                </b>{' '}
                <span className="muted">{h.value_source === 'read_from_vehicle' ? t('(read from the vehicle)') : t('(by you)')}</span>
              </div>
              {h.spec && (
                <div>
                  <span className="chip">{t('Specification')}</span> {h.spec.min}–{h.spec.max} {h.unit} <span className="muted">· {t(SPEC_KIND[h.spec.kind])}</span>
                </div>
              )}
              {h.explanation && (
                <div style={{ gridColumn: '1 / -1' }}>
                  <span className={`chip ${h.comparison === 'in_range' ? 'ok' : 'warn'}`}>{h.comparison === 'in_range' ? t('Within range') : h.comparison === 'below' ? t('Below range') : t('Above range')}</span>{' '}
                  {h.explanation}
                </div>
              )}
            </div>
          )}
        </div>
      ))}
      <ErrorBox error={evaluate.error ?? (state?.error ? new Error(state.error) : null)} />
      {state?.outcome && (
        <div className={`banner ${state.outcome.kind === 'finding' ? 'ok' : 'warn'}`}>
          <span className="chip">{state.outcome.kind === 'finding' ? t('Finding') : t('Likely explanation (to confirm)')}</span>
          <p>{state.outcome.text}</p>
          <button className="btn small" onClick={() => evaluate.run([])}>
            {t('Start again')}
          </button>
        </div>
      )}
      {step && (
        <div className="card">
          <p style={{ fontSize: 17 }}>{step.text}</p>
          {step.help && <p className="muted small">{step.help}</p>}
          {step.kind === 'question' && (
            <div className="row">
              <button className="btn primary" onClick={() => answer({ choice: 'yes' })}>
                {t('Yes')}
              </button>
              <button className="btn" onClick={() => answer({ choice: 'no' })}>
                {t('No')}
              </button>
            </div>
          )}
          {step.kind === 'info' && (
            <button className="btn primary" onClick={() => answer({ choice: 'ok' })}>
              {t('Done, continue')}
            </button>
          )}
          {step.kind === 'measurement' && (
            <div className="stack">
              <div className="row">
                <label className="field" style={{ flex: 1, minWidth: 160 }}>
                  {t('Your measurement ({unit})', { unit: step.measurement?.unit ?? '' })}
                  <input type="number" inputMode="decimal" value={value} onChange={(e) => (setValue(e.target.value), setFromVehicle(false))} />
                </label>
                {step.live_pid && summary?.vehicle_connected && (
                  <button className="btn" style={{ alignSelf: 'flex-end' }} onClick={() => readLive.run(step.live_pid!)}>
                    📈 {t('Read from vehicle')}
                  </button>
                )}
              </div>
              {step.measurement?.user_spec && (
                <div className="row">
                  <label className="field" style={{ flex: 1 }}>
                    {t('Minimum from your manual')}
                    <input type="number" inputMode="decimal" value={spec.min} onChange={(e) => setSpec({ ...spec, min: e.target.value })} />
                  </label>
                  <label className="field" style={{ flex: 1 }}>
                    {t('Maximum from your manual')}
                    <input type="number" inputMode="decimal" value={spec.max} onChange={(e) => setSpec({ ...spec, max: e.target.value })} />
                  </label>
                </div>
              )}
              <ErrorBox error={readLive.error} />
              <button
                className="btn primary"
                disabled={value === ''}
                onClick={() =>
                  answer({
                    value: Number(value),
                    value_source: fromVehicle ? 'read_from_vehicle' : 'measured_by_user',
                    ...(step.measurement?.user_spec && spec.min !== '' && spec.max !== '' ? { spec_min: Number(spec.min), spec_max: Number(spec.max) } : {}),
                  })
                }
              >
                {t('Check')}
              </button>
            </div>
          )}
        </div>
      )}
    </>
  );
}

export default function Guided() {
  const { arg, report } = useApp();
  const [procs, setProcs] = useState<Procedure[]>([]);
  const [open, setOpen] = useState<Procedure | null>(null);
  useEffect(() => {
    call<Procedure[]>('procedures').then(setProcs).catch(() => {});
  }, []);
  if (open) return <Run proc={open} onBack={() => setOpen(null)} />;
  const codes = new Set([...(report?.dtcs.map((d) => d.code) ?? []), ...(arg ? [arg] : [])]);
  const relevant = procs.filter((p) => p.applies_to.codes.some((c) => codes.has(c)));
  return (
    <>
      <h1>{t('Guided Diagnostics')}</h1>
      <p className="muted">{t('Step-by-step checks. OpenDiag keeps what you measured, the specification and what it suggests clearly apart.')}</p>
      {relevant.length > 0 && (
        <>
          <h2>{t('Suggested for your codes')}</h2>
          <div className="grid tiles">
            {relevant.map((p) => (
              <button key={p.id} className="tile primary" onClick={() => setOpen(p)}>
                <b>{p.title}</b>
                <span className="muted small">{p.summary}</span>
              </button>
            ))}
          </div>
        </>
      )}
      <h2>{t('All procedures')}</h2>
      <div className="grid tiles">
        {procs.map((p) => (
          <button key={p.id} className="tile" onClick={() => setOpen(p)}>
            <b>{p.title}</b>
            <span className="muted small">{p.summary}</span>
            <span className="chip">{p.applies_to.vehicle_kinds.join(' · ')}</span>
          </button>
        ))}
      </div>
      <p className="muted small" style={{ marginTop: 14 }}>
        {t('Want to add a procedure? See docs/contributing-knowledge.md: procedures are data files with sources and compatibility limits.')}
      </p>
    </>
  );
}
