import { ReactNode, useEffect, useState } from 'react';
import { t } from './i18n';
import type { Severity, SupportStatus } from './types';

export function StatusChip({ status, label }: { status: SupportStatus; label?: string }) {
  const cls: Record<SupportStatus, string> = {
    implemented_tested: 'ok',
    implemented_not_hardware_verified: 'warn',
    partially_implemented: 'warn',
    simulated_only: 'sim',
    planned: '',
    unsupported: 'bad',
  };
  const text: Record<SupportStatus, string> = {
    implemented_tested: t('Implemented and tested'),
    implemented_not_hardware_verified: t('Not hardware-verified'),
    partially_implemented: t('Partially implemented'),
    simulated_only: t('Simulated only'),
    planned: t('Planned'),
    unsupported: t('Unsupported'),
  };
  return <span className={`chip ${cls[status]}`}>{label ?? text[status]}</span>;
}

export function SeverityChip({ severity }: { severity: Severity }) {
  const cls = { info: 'accent', low: 'accent', moderate: 'warn', high: 'bad', critical: 'bad' }[severity];
  const label = { info: t('Info'), low: t('Low'), moderate: t('Moderate'), high: t('High'), critical: t('Critical') }[severity];
  return <span className={`chip ${cls}`}>{label}</span>;
}

export function Spinner() {
  return <span className="spinner" role="status" aria-label={t('Working')} />;
}

export function ErrorBox({ error }: { error: unknown }) {
  if (!error) return null;
  const msg = error instanceof Error ? error.message : String(error);
  return (
    <div className="banner bad" role="alert">
      {msg}
    </div>
  );
}

export function SimBanner({ show }: { show: boolean }) {
  if (!show) return null;
  return <div className="banner sim">{t('SIMULATED VEHICLE: these results come from the built-in simulator, not a real vehicle.')}</div>;
}

export function Modal({ children, onClose, title }: { children: ReactNode; onClose?: () => void; title: string }) {
  useEffect(() => {
    const k = (e: KeyboardEvent) => e.key === 'Escape' && onClose?.();
    window.addEventListener('keydown', k);
    return () => window.removeEventListener('keydown', k);
  }, [onClose]);
  return (
    <div className="modal-bg" onClick={onClose}>
      <div className="modal" role="dialog" aria-modal="true" aria-label={title} onClick={(e) => e.stopPropagation()}>
        <h2 style={{ marginTop: 0 }}>{title}</h2>
        {children}
      </div>
    </div>
  );
}

export function Sparkline({ values, min, max }: { values: number[]; min?: number; max?: number }) {
  if (values.length < 2) return <svg viewBox="0 0 100 40" />;
  const lo = min ?? Math.min(...values);
  const hi = max ?? Math.max(...values);
  const span = hi - lo || 1;
  const pts = values.map((v, i) => `${(i / (values.length - 1)) * 100},${38 - ((v - lo) / span) * 36}`).join(' ');
  return (
    <svg viewBox="0 0 100 40" preserveAspectRatio="none" aria-hidden="true">
      <polyline className="line" points={pts} vectorEffect="non-scaling-stroke" />
    </svg>
  );
}

/** Runs an async action and keeps its busy/error state. */
export function useAction<A extends unknown[], R>(fn: (...a: A) => Promise<R>) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<unknown>(null);
  const run = async (...a: A): Promise<R | undefined> => {
    setBusy(true);
    setError(null);
    try {
      return await fn(...a);
    } catch (e) {
      setError(e);
      return undefined;
    } finally {
      setBusy(false);
    }
  };
  return { run, busy, error, setError };
}

export function Kv({ items }: { items: [string, ReactNode][] }) {
  return (
    <dl className="kv">
      {items.map(([k, v]) => (
        <div key={k} style={{ display: 'contents' }}>
          <dt>{k}</dt>
          <dd>{v}</dd>
        </div>
      ))}
    </dl>
  );
}
