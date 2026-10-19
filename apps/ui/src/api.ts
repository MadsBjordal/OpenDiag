// The only way the UI talks to the engine.
// In the app (Windows, Android, …) it goes through Tauri; in a browser it goes to
// the local development bridge (`pnpm bridge`), which runs the same engine.

type Result<T> = { ok: true; data: T } | { ok: false; error: string; kind: string };

export class ApiError extends Error {
  constructor(
    message: string,
    public kind: string,
  ) {
    super(message);
  }
}

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

export const inApp = (): boolean => typeof window !== 'undefined' && !!window.__TAURI_INTERNALS__;

const bridgeUrl = (): string => (import.meta.env.VITE_BRIDGE as string | undefined) ?? (location.port === '5173' ? 'http://127.0.0.1:7878/api' : '/api');

export async function call<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
  const request = { cmd, ...args };
  let res: Result<T>;
  if (inApp()) {
    const { invoke } = await import('@tauri-apps/api/core');
    res = await invoke<Result<T>>('od_call', { request });
  } else {
    let r: Response;
    try {
      r = await fetch(bridgeUrl(), { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(request) });
    } catch {
      throw new ApiError('The OpenDiag engine is not running. Start it with "pnpm bridge" (development) or use the app.', 'bridge');
    }
    res = (await r.json()) as Result<T>;
  }
  if (!res.ok) throw new ApiError(res.error, res.kind);
  return res.data;
}

/** Subscribes to engine events (scan progress). Returns an unsubscribe function. */
export async function listen<T>(event: string, handler: (payload: T) => void): Promise<() => void> {
  if (inApp()) {
    const { listen } = await import('@tauri-apps/api/event');
    return listen<T>(event, (e) => handler(e.payload));
  }
  // The bridge has no push channel: poll the progress while a scan runs.
  if (event !== 'scan_progress') return () => {};
  let stop = false;
  const tick = async () => {
    while (!stop) {
      try {
        const p = await call<T | null>('progress');
        if (p) handler(p);
      } catch {
        /* ignore */
      }
      await new Promise((r) => setTimeout(r, 250));
    }
  };
  void tick();
  return () => {
    stop = true;
  };
}

/** Saves an exported file: in the app to the exports folder, in a browser as a download. */
export async function saveFile(file: { filename: string; base64: string }): Promise<string> {
  if (inApp()) {
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke<string>('save_export', { filename: file.filename, base64: file.base64 });
  }
  const bytes = Uint8Array.from(atob(file.base64), (c) => c.charCodeAt(0));
  const url = URL.createObjectURL(new Blob([bytes]));
  const a = document.createElement('a');
  a.href = url;
  a.download = file.filename;
  a.click();
  URL.revokeObjectURL(url);
  return file.filename;
}

export function textFile(filename: string, text: string): { filename: string; base64: string } {
  const bytes = new TextEncoder().encode(text);
  let bin = '';
  bytes.forEach((b) => (bin += String.fromCharCode(b)));
  return { filename, base64: btoa(bin) };
}
