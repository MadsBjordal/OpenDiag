import { useEffect, useState } from 'react';
import { Modal } from './components';
import { t } from './i18n';
import { NOTICE, NOTICE_TITLE, PRIVACY } from './notice';
import { Page, useApp } from './state';
import Home from './pages/Home';
import Connect from './pages/Connect';
import Scan from './pages/Scan';
import Codes from './pages/Codes';
import Live from './pages/Live';
import Guided from './pages/Guided';
import Config from './pages/Config';
import Reports from './pages/Reports';
import Garage from './pages/Garage';
import Compat from './pages/Compat';
import SettingsPage from './pages/Settings';
import Console from './pages/Console';
import Simulator from './pages/Simulator';
import Connectors from './pages/Connectors';
import HardwareTest from './pages/HardwareTest';
import { format } from './units';

const NAV: { page: Page; icon: string; label: string; mobile?: boolean; expert?: boolean }[] = [
  { page: 'home', icon: '⌂', label: 'Home', mobile: true },
  { page: 'connect', icon: '🔌', label: 'Connect', mobile: true },
  { page: 'connectors', icon: '🔍', label: 'Find my connector' },
  { page: 'scan', icon: '🩺', label: 'Scan Vehicle', mobile: true },
  { page: 'codes', icon: '⚠', label: 'Fault Codes' },
  { page: 'live', icon: '📈', label: 'Live Data', mobile: true },
  { page: 'guided', icon: '🧭', label: 'Guided Diagnostics' },
  { page: 'config', icon: '⚙', label: 'Vehicle Settings' },
  { page: 'reports', icon: '📄', label: 'Reports' },
  { page: 'garage', icon: '🚗', label: 'My Garage' },
  { page: 'compat', icon: '✔', label: 'Compatibility' },
  { page: 'hwtest', icon: '🧪', label: 'Hardware test' },
  { page: 'console', icon: '⌨', label: 'Expert console', expert: true },
  { page: 'settings', icon: '☰', label: 'Settings' },
];

const NOTICE_KEY = 'opendiag.notice-seen';

export default function App() {
  const { page, go, settings, summary } = useApp();
  const [showNotice, setShowNotice] = useState(() => {
    try {
      return localStorage.getItem(NOTICE_KEY) !== '1';
    } catch {
      return true;
    }
  });
  useEffect(() => {
    document.title = 'OpenDiag';
  }, []);
  const expert = settings?.mode === 'expert';
  const nav = NAV.filter((n) => !n.expert || expert);
  const closeNotice = () => {
    try {
      localStorage.setItem(NOTICE_KEY, '1');
    } catch {
      /* private mode */
    }
    setShowNotice(false);
  };

  const pages: Record<Page, JSX.Element> = {
    home: <Home />,
    connect: <Connect />,
    scan: <Scan />,
    codes: <Codes />,
    live: <Live />,
    guided: <Guided />,
    config: <Config />,
    reports: <Reports />,
    garage: <Garage />,
    compat: <Compat />,
    settings: <SettingsPage />,
    console: <Console />,
    simulator: <Simulator />,
    connectors: <Connectors />,
    hwtest: <HardwareTest />,
    menu: (
      <>
        <h1>{t('Menu')}</h1>
        <div className="stack">
          {nav.map((n) => (
            <button key={n.page} className="nav card" onClick={() => go(n.page)}>
              <span className="ico" aria-hidden="true">
                {n.icon}
              </span>
              {t(n.label)}
            </button>
          ))}
        </div>
      </>
    ),
  };

  return (
    <div className="app">
      <nav className="side" aria-label={t('Main')}>
        <div className="brand">
          <img src="./icon.svg" alt="" /> OpenDiag
        </div>
        {nav.map((n) => (
          <button key={n.page} className="nav" aria-current={page === n.page ? 'page' : undefined} onClick={() => go(n.page)}>
            <span className="ico" aria-hidden="true">
              {n.icon}
            </span>
            {t(n.label)}
          </button>
        ))}
        <div className="spacer" />
        <p className="muted small" style={{ padding: '0 12px' }}>
          {t('Free and open source · GPL-3.0')}
        </p>
      </nav>
      <div className="main">
        <header className="topbar">
          {summary ? (
            <>
              <span className={`chip ${summary.vehicle_connected ? 'ok' : 'warn'}`}>
                <span className="dot" /> {summary.vehicle_connected ? t('Vehicle connected') : t('Adapter only')}
              </span>
              {summary.simulated && (
                <button className="chip sim" style={{ border: 0, cursor: 'pointer' }} onClick={() => go('simulator')}>
                  {t('SIMULATED')} · {t('controls')}
                </button>
              )}
              <span className="chip">{summary.adapter.identity}</span>
              {summary.adapter_voltage !== null && <span className="chip">🔋 {format(summary.adapter_voltage, 'V', 'metric')}</span>}
            </>
          ) : (
            <span className="chip">
              <span className="dot" /> {t('Not connected')}
            </span>
          )}
          <span style={{ flex: 1 }} />
          {settings && (
            <button
              className="chip accent"
              style={{ border: 0, cursor: 'pointer' }}
              title={t('Switch between beginner and expert mode')}
              onClick={() => go('settings')}
            >
              {settings.mode === 'expert' ? t('Expert mode') : t('Beginner mode')}
            </button>
          )}
        </header>
        <main className="page">{pages[page] ?? <Home />}</main>
      </div>
      <nav className="mobile-nav" aria-label={t('Main')}>
        {nav
          .filter((n) => n.mobile)
          .map((n) => (
            <button key={n.page} aria-current={page === n.page ? 'page' : undefined} onClick={() => go(n.page)}>
              <span className="ico" aria-hidden="true">
                {n.icon}
              </span>
              {t(n.label)}
            </button>
          ))}
        <button aria-current={page === 'menu' ? 'page' : undefined} onClick={() => go('menu')}>
          <span className="ico" aria-hidden="true">
            ☰
          </span>
          {t('More')}
        </button>
      </nav>
      {showNotice && (
        <Modal title={t(NOTICE_TITLE)}>
          {NOTICE.map((p) => (
            <p key={p}>{t(p)}</p>
          ))}
          <h3>{t('Your privacy')}</h3>
          <ul className="tight">
            {PRIVACY.map((p) => (
              <li key={p}>{t(p)}</li>
            ))}
          </ul>
          <p className="muted small">
            {t('Reading diagnostics works right away. Before any function that changes the vehicle you will be asked to accept this notice in Settings.')}
          </p>
          <div className="row" style={{ marginTop: 14 }}>
            <button className="btn primary" onClick={closeNotice}>
              {t('Continue')}
            </button>
            <button
              className="btn"
              onClick={() => {
                closeNotice();
                go('settings');
              }}
            >
              {t('Read it again in Settings')}
            </button>
          </div>
        </Modal>
      )}
    </div>
  );
}
