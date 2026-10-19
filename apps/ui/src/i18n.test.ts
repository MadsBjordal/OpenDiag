import { expect, it } from 'vitest';
import { t } from './i18n';

it('fills placeholders', () => {
  expect(t('{n} fault codes', { n: 3 })).toBe('3 fault codes');
  expect(t('Hello {who}')).toBe('Hello {who}');
});
