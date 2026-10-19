// Every visible text goes through t(). English is the key; other languages are
// dictionaries keyed by the English text. Add a language by adding a dictionary
// and listing it in LANGUAGES (and in the engine's allowed "language" setting).

type Dict = Record<string, string>;

export const LANGUAGES: { code: string; name: string }[] = [{ code: 'en', name: 'English' }];

const dictionaries: Record<string, Dict> = { en: {} };

let current = 'en';

export function setLanguage(code: string): void {
  current = dictionaries[code] ? code : 'en';
}

export function t(text: string, values: Record<string, string | number> = {}): string {
  const s = dictionaries[current]?.[text] ?? text;
  return s.replace(/\{(\w+)\}/g, (_, k: string) => (k in values ? String(values[k]) : `{${k}}`));
}
