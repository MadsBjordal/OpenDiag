import { createContext, ReactNode, useCallback, useContext, useEffect, useState } from 'react';
import { call } from './api';
import { setLanguage } from './i18n';
import type { Report, Settings, Summary } from './types';

export type Page =
  | 'home'
  | 'connect'
  | 'scan'
  | 'codes'
  | 'live'
  | 'guided'
  | 'config'
  | 'reports'
  | 'garage'
  | 'compat'
  | 'settings'
  | 'console'
  | 'simulator'
  | 'menu'
  | 'connectors'
  | 'hwtest'
  | 'truck';

interface AppState {
  page: Page;
  go: (p: Page, arg?: string) => void;
  arg: string | null;
  settings: Settings | null;
  setSettings: (s: Settings) => void;
  summary: Summary | null;
  setSummary: (s: Summary | null) => void;
  refreshSummary: () => Promise<void>;
  report: Report | null;
  setReport: (r: Report | null) => void;
}

const Ctx = createContext<AppState | null>(null);

function parseHash(): [Page, string | null] {
  const [p, a] = location.hash.replace(/^#\/?/, '').split('/');
  return [((p || 'home') as Page), a ? decodeURIComponent(a) : null];
}

export function AppProvider({ children }: { children: ReactNode }) {
  const [[page, arg], setRoute] = useState(parseHash);
  const [settings, setSettingsState] = useState<Settings | null>(null);
  const [summary, setSummary] = useState<Summary | null>(null);
  const [report, setReport] = useState<Report | null>(null);

  useEffect(() => {
    const h = () => setRoute(parseHash());
    window.addEventListener('hashchange', h);
    return () => window.removeEventListener('hashchange', h);
  }, []);

  const setSettings = useCallback((s: Settings) => {
    setSettingsState(s);
    setLanguage(s.language);
    const dark = s.theme === 'dark' || (s.theme === 'system' && matchMedia('(prefers-color-scheme: dark)').matches);
    document.documentElement.dataset.theme = dark ? 'dark' : 'light';
  }, []);

  const refreshSummary = useCallback(async () => {
    try {
      setSummary(await call<Summary>('summary'));
    } catch {
      setSummary(null);
    }
  }, []);

  useEffect(() => {
    call<Settings>('settings').then(setSettings).catch(() => {});
    void refreshSummary();
  }, [setSettings, refreshSummary]);

  const go = (p: Page, a?: string) => {
    location.hash = `#/${p}${a ? '/' + encodeURIComponent(a) : ''}`;
  };

  return (
    <Ctx.Provider value={{ page, arg, go, settings, setSettings, summary, setSummary, refreshSummary, report, setReport }}>{children}</Ctx.Provider>
  );
}

export function useApp(): AppState {
  const c = useContext(Ctx);
  if (!c) throw new Error('no AppProvider');
  return c;
}
