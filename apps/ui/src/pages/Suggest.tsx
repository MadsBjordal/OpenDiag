import { useState } from 'react';
import { inApp } from '../api';
import { ErrorBox, Spinner, useAction } from '../components';
import { t } from '../i18n';
import { ProjectLink } from '../links';

const NEW_ISSUE = 'https://github.com/MadsBjordal/OpenDiag/issues/new';
const LIST = 'https://github.com/MadsBjordal/OpenDiag/issues?q=label%3A%22feature+request%22';

/** Feature requests and problem reports from inside the app; they become GitHub issues. */
export default function Suggest() {
  const [kind, setKind] = useState<'feature' | 'problem'>('feature');
  const [text, setText] = useState('');
  const [done, setDone] = useState<{ number: number; url: string } | null>(null);
  const send = useAction(async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    setDone(await invoke<{ number: number; url: string }>('suggest_send', { kind, text }));
    setText('');
  });
  const long = text.trim().length >= 10;
  return (
    <>
      <h1>{t('Suggest a feature')}</h1>
      <p className="muted">{t('Tell us what OpenDiag should do, or what did not work. Every suggestion becomes a public issue on GitHub, where you can follow it.')}</p>
      <div className="card">
        <div className="row">
          <label className="row small">
            <input type="radio" name="kind" checked={kind === 'feature'} onChange={() => setKind('feature')} />
            {t('A new feature or improvement')}
          </label>
          <label className="row small">
            <input type="radio" name="kind" checked={kind === 'problem'} onChange={() => setKind('problem')} />
            {t('Something that does not work')}
          </label>
        </div>
        <label className="field" style={{ marginTop: 10 }}>
          {kind === 'feature' ? t('What would you like OpenDiag to do?') : t('What happened, and what did you expect?')}
          <textarea rows={6} maxLength={2000} value={text} onChange={(e) => setText(e.target.value)} placeholder={t('The first line becomes the title.')} />
        </label>
        <p className="small muted">
          {t(
            'This is published on GitHub for everyone to see: do not include your name, email, VIN, number plate or other personal details. The app adds only its version and operating system.',
          )}
        </p>
        {inApp() ? (
          <button className="btn primary" disabled={send.busy || !long} onClick={() => send.run()}>
            {send.busy ? (
              <>
                <Spinner /> {t('Sending…')}
              </>
            ) : (
              t('Send')
            )}
          </button>
        ) : (
          <a
            className="btn primary"
            href={`${NEW_ISSUE}?labels=${kind === 'feature' ? 'feature+request' : 'bug'}&body=${encodeURIComponent(text)}`}
            target="_blank"
            rel="noreferrer"
          >
            {t('Continue on GitHub')}
          </a>
        )}
        {!long && text.length > 0 && <span className="small muted"> {t('A few more words, please (at least 10 characters).')}</span>}
        <ErrorBox error={send.error} />
        {done && (
          <p className="banner ok small">
            {t('Thank you! It was posted as issue #{n}.', { n: done.number })} <ProjectLink href={done.url}>{t('Open it on GitHub')}</ProjectLink>
          </p>
        )}
      </div>
      <p className="small">
        <ProjectLink href={LIST}>{t('See all feature requests')}</ProjectLink>
      </p>
    </>
  );
}
