export type UnitSystem = 'metric' | 'imperial';

interface Converted {
  value: number;
  unit: string;
}

/** Converts an engine value (always metric) for display. */
export function convert(value: number, unit: string, system: UnitSystem): Converted {
  if (system === 'metric') return { value, unit };
  switch (unit) {
    case '°C':
      return { value: (value * 9) / 5 + 32, unit: '°F' };
    case 'km/h':
      return { value: value / 1.609344, unit: 'mph' };
    case 'km':
      return { value: value / 1.609344, unit: 'mi' };
    case 'kPa':
      return { value: value * 0.1450377, unit: 'psi' };
    case 'L/h':
      return { value: value / 3.785412, unit: 'gal/h' };
    case 'g/s':
      return { value: value * 0.1322774, unit: 'lb/min' };
    default:
      return { value, unit };
  }
}

export function format(value: number | null | undefined, unit: string, system: UnitSystem): string {
  if (value === null || value === undefined || Number.isNaN(value)) return '—';
  const c = convert(value, unit, system);
  const digits = Number.isInteger(c.value) || Math.abs(c.value) >= 100 ? 0 : Math.abs(c.value) >= 10 ? 1 : 2;
  return `${c.value.toFixed(digits)} ${c.unit}`.trim();
}

/** CSV for a live data recording: one row per sample time, one column per value. */
export function liveCsv(keys: { key: string; name: string; unit: string }[], rows: { t: number; values: Record<string, number | null> }[]): string {
  const esc = (s: string) => (/[",\n]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s);
  const head = ['time_iso', ...keys.map((k) => esc(`${k.name} (${k.unit})`))].join(',');
  const body = rows.map((r) => [new Date(r.t).toISOString(), ...keys.map((k) => (r.values[k.key] ?? '') + '')].join(','));
  return [head, ...body].join('\n') + '\n';
}

const PROTOCOLS: Record<string, string> = {
  j1850_pwm: 'SAE J1850 PWM',
  j1850_vpw: 'SAE J1850 VPW',
  iso9141_2: 'ISO 9141-2 (K-line)',
  iso14230_slow: 'ISO 14230-4 KWP (5 baud init)',
  iso14230_fast: 'ISO 14230-4 KWP (fast init)',
  can11_500: 'CAN 11 bit, 500 kbaud (ISO 15765-4)',
  can29_500: 'CAN 29 bit, 500 kbaud (ISO 15765-4)',
  can11_250: 'CAN 11 bit, 250 kbaud (ISO 15765-4)',
  can29_250: 'CAN 29 bit, 250 kbaud (ISO 15765-4)',
};

export const protocolName = (p: string | null | undefined): string => (p ? PROTOCOLS[p] ?? p : '—');
