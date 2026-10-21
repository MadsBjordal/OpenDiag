import type { ReactNode } from 'react';
import { inApp } from './api';

export const CHANGELOG = 'https://github.com/MadsBjordal/OpenDiag/blob/main/CHANGELOG.md';

/** In the app, links open in the browser through the shell; in a browser they are plain links. */
export function ProjectLink({ href, children }: { href: string; children: ReactNode }) {
  return (
    <a
      href={href}
      target="_blank"
      rel="noreferrer"
      onClick={async (e) => {
        if (!inApp()) return;
        e.preventDefault();
        const { invoke } = await import('@tauri-apps/api/core');
        await invoke('open_project_page', { url: href });
      }}
    >
      {children}
    </a>
  );
}
