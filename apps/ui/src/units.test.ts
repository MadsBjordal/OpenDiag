import { describe, expect, it } from 'vitest';
import { convert, format, liveCsv, protocolName } from './units';

describe('units', () => {
  it('converts to imperial', () => {
    expect(convert(100, '°C', 'imperial')).toEqual({ value: 212, unit: '°F' });
    expect(convert(100, 'km/h', 'imperial').value).toBeCloseTo(62.14, 2);
    expect(convert(100, 'kPa', 'imperial').value).toBeCloseTo(14.5, 1);
    expect(convert(12.5, 'V', 'imperial')).toEqual({ value: 12.5, unit: 'V' });
  });
  it('formats', () => {
    expect(format(1726, 'rpm', 'metric')).toBe('1726 rpm');
    expect(format(14.06, 'V', 'metric')).toBe('14.1 V');
    expect(format(null, 'V', 'metric')).toBe('—');
    expect(format(0, 'km/h', 'metric')).toBe('0 km/h');
    expect(format(4.5, 'V', 'metric')).toBe('4.50 V');
  });
  it('writes CSV', () => {
    const csv = liveCsv([{ key: 'rpm', name: 'Engine speed', unit: 'rpm' }], [{ t: 0, values: { rpm: 800 } }, { t: 1000, values: { rpm: null } }]);
    expect(csv.split('\n')[0]).toBe('time_iso,Engine speed (rpm)');
    expect(csv.split('\n')[1]).toBe('1970-01-01T00:00:00.000Z,800');
    expect(csv.split('\n')[2]).toBe('1970-01-01T00:00:01.000Z,');
  });
});

it('names protocols', () => {
  expect(protocolName('iso9141_2')).toBe('ISO 9141-2 (K-line)');
  expect(protocolName(null)).toBe('—');
});
