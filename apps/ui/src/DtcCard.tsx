import { SeverityChip } from './components';
import { t } from './i18n';
import { useApp } from './state';
import type { DtcFinding, Explanation, Measurement } from './types';
import { format } from './units';

const KIND_TEXT = {
  stored: ['Stored', 'Confirmed by the module.'],
  pending: ['Pending', 'Seen once, not confirmed yet. It becomes stored if it happens again.'],
  permanent: ['Permanent', 'Cannot be cleared by a tool. It clears itself after the vehicle\'s own test passes.'],
} as const;

export function DtcCard({ finding, explanation, observed = [] }: { finding?: DtcFinding; explanation?: Explanation; observed?: Measurement[] }) {
  const { settings, go } = useApp();
  const e = finding?.explanation ?? explanation!;
  const units = settings?.units ?? 'metric';
  const expert = settings?.mode === 'expert';
  const related = observed.filter((m) => e.related_measurements.includes(m.key));
  return (
    <div className={`card dtc sev-${e.severity}`}>
      <div className="row between">
        <div className="row">
          <span className="code">{e.code}</span>
          <SeverityChip severity={e.severity} />
          {finding && (
            <span className={`chip ${finding.kind === 'pending' ? 'warn' : finding.kind === 'permanent' ? 'accent' : 'bad'}`} title={t(KIND_TEXT[finding.kind][1])}>
              {t(KIND_TEXT[finding.kind][0])}
            </span>
          )}
        </div>
        {finding && (
          <span className="muted small">
            {finding.ecu_label} {expert && <span className="mono">({finding.ecu})</span>}
          </span>
        )}
      </div>
      <h3 style={{ marginTop: 8 }}>{e.title}</h3>
      <p>{e.plain}</p>
      {e.driving_advice && <p className="banner warn small">{e.driving_advice}</p>}
      {related.length > 0 && (
        <p className="small">
          <b>{t('Measured now')}:</b> {related.map((m) => `${m.name} ${format(m.value, m.unit, units)}`).join(' · ')}
        </p>
      )}
      <details>
        <summary>{t('Details, possible causes and tests')}</summary>
        {expert && <p className="mono small">{e.technical}</p>}
        <p className="small muted">{e.severity_note}</p>
        {e.possible_causes.length > 0 && (
          <>
            <h3>{t('Possible causes (to check, not conclusions)')}</h3>
            <ul className="tight">
              {e.possible_causes.map((c) => (
                <li key={c}>{c}</li>
              ))}
            </ul>
          </>
        )}
        {e.suggested_tests.length > 0 && (
          <>
            <h3>{t('Suggested tests')}</h3>
            <ul className="tight">
              {e.suggested_tests.map((c) => (
                <li key={c}>{c}</li>
              ))}
            </ul>
          </>
        )}
        <div className="row">
          {e.related_measurements.length > 0 && (
            <button className="btn small" onClick={() => go('live', e.related_measurements.join(','))}>
              📈 {t('Watch related live data')}
            </button>
          )}
          <button className="btn small" onClick={() => go('guided', e.code)}>
            🧭 {t('Guided help')}
          </button>
        </div>
        <p className="muted small">
          {t('Source')}: {e.source}
        </p>
      </details>
    </div>
  );
}
