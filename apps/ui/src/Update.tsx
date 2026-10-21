import { useEffect, useState } from 'react';
import { inApp } from './api';
import { ErrorBox, Spinner, useAction } from './components';
import { t } from './i18n';
import { useApp } from './state';
import type { UpdateInfo } from './types';
import { CHANGELOG, ProjectLink } from './links';

const DISMISSED = 'opendiag.update.dismissed';

async function invoke<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
  const { invoke } = await import('@tauri-apps/api/core');
  return invoke<T>(cmd, args);
}

export const checkForUpdate = () => invoke<UpdateInfo>('update_check');

/** Starts an update: Windows installs it (OpenDiag closes); Android opens the download. */
export function useInstall() {
  const { settings } = useApp();
  const [started, setStarted] = useState(false);
  const install = useAction(async (u: UpdateInfo) => {
    await invoke('update_install', { url: u.url, sha256: u.sha256 });
    setStarted(true);
  });
  const note =
    started && settings?.platform === 'android'
      ? t('The download opens in your browser. When it has finished, open the file and tap Update. Your reports and vehicles are kept.')
      : null;
  return { install, note };
}

export function UpdateButton({ info }: { info: UpdateInfo }) {
  const { summary, settings } = useApp();
  const { install, note } = useInstall();
  // Never in the middle of work with a vehicle: the Windows installer closes the app.
  const busy = !!summary && settings?.platform === 'windows';
  return (
    <>
      <button className="btn primary small" disabled={install.busy || busy} onClick={() => install.run(info)}>
        {install.busy ? <Spinner /> : null} {settings?.platform === 'windows' ? t('Update now') : t('Download update')}
      </button>
      {busy && <span className="small muted"> {t('Disconnect from the vehicle first.')}</span>}
      {note && <p className="small">{note}</p>}
      <ErrorBox error={install.error} />
    </>
  );
}

/** A build's changelog section: "### Added" headings and "- " items (Markdown kept simple). */
export function WhatsNew({ notes }: { notes: string }) {
  const plain = (l: string) => l.replace(/\*\*/g, '').replace(/\*/g, '');
  return (
    <div className="small" style={{ marginTop: 6 }}>
      {notes.split('\n').map((l, i) =>
        l.startsWith('### ') ? (
          <div key={i} style={{ fontWeight: 600, marginTop: 6 }}>
            {plain(l.slice(4))}
          </div>
        ) : l.startsWith('- ') ? (
          <div key={i} style={{ paddingLeft: 14, textIndent: -10 }}>
            • {plain(l.slice(2))}
          </div>
        ) : l.trim() ? (
          <div key={i}>{plain(l)}</div>
        ) : null,
      )}
    </div>
  );
}

/** Shown at the top of the app when a newer build exists. Checked once per start. */
export function UpdateBanner() {
  const { settings } = useApp();
  const [info, setInfo] = useState<UpdateInfo | null>(null);
  const [hidden, setHidden] = useState(false);
  const enabled = inApp() && !!settings?.update_check;
  useEffect(() => {
    if (!enabled) return;
    checkForUpdate()
      .then((u) => {
        let dismissed: string | null = null;
        try {
          dismissed = localStorage.getItem(DISMISSED);
        } catch {
          /* ignore */
        }
        if (u.available && u.tag !== dismissed) setInfo(u);
      })
      .catch(() => {
        /* offline: try again next start */
      });
  }, [enabled]);
  if (!info || hidden) return null;
  return (
    <div className="banner info" style={{ margin: '12px 16px 0' }}>
      <b>{t('A new version of OpenDiag is available')}</b> ({info.name ?? info.tag}). <UpdateButton info={info} />{' '}
      <button
        className="btn small ghost"
        onClick={() => {
          try {
            localStorage.setItem(DISMISSED, info.tag ?? '');
          } catch {
            /* ignore */
          }
          setHidden(true);
        }}
      >
        {t('Later')}
      </button>
      {info.notes && (
        <details style={{ marginTop: 6 }}>
          <summary>{t("What's new")}</summary>
          <WhatsNew notes={info.notes} />
        </details>
      )}
    </div>
  );
}

/** Settings card: switch the check on or off, check now. */
export function UpdateSettings({ onToggle }: { onToggle: (on: boolean) => void }) {
  const { settings } = useApp();
  const [info, setInfo] = useState<UpdateInfo | null>(null);
  const check = useAction(async () => setInfo(await checkForUpdate()));
  if (!settings) return null;
  return (
    <div className="card">
      <h2 style={{ marginTop: 0 }}>{t('Updates')}</h2>
      <label className="row small">
        <input type="checkbox" checked={settings.update_check} onChange={(e) => onToggle(e.target.checked)} />
        {t('Check for new versions when OpenDiag starts')}
      </label>
      <p className="small muted">
        {t('Asks GitHub which OpenDiag version is the newest. Nothing about you or your vehicle is sent. Updates are only installed when you press the button.')}
      </p>
      {!inApp() ? (
        <p className="small muted">{t('Updates are available in the installed app.')}</p>
      ) : (
        <>
          <button className="btn small" disabled={check.busy} onClick={() => check.run()}>
            {check.busy ? <Spinner /> : null} {t('Check now')}
          </button>{' '}
          <ProjectLink href={CHANGELOG}>{t('Version history')}</ProjectLink>
          <ErrorBox error={check.error} />
          {info &&
            (info.current == null ? (
              <p className="small">{t('This is a development build: it is not updated from GitHub.')}</p>
            ) : info.available ? (
              <div className="small">
                {t('New version: {name}.', {
                  name: info.name ?? info.tag ?? '',
                })}{' '}
                <UpdateButton info={info} />
                {info.notes && <WhatsNew notes={info.notes} />}
              </div>
            ) : (
              <p className="small">{t('You have the newest version ({v}).', { v: info.current })}</p>
            ))}
        </>
      )}
    </div>
  );
}
