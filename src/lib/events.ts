import { listen } from "@tauri-apps/api/event";

export function subscribe(event: string, callback: () => void): () => void {
  let disposed = false;
  let unlisten: (() => void) | undefined;
  listen(event, callback).then(fn => {
    if (disposed) fn(); else unlisten = fn;
  }).catch(error => console.error(`Cannot subscribe to ${event}`, error));
  return () => { disposed = true; unlisten?.(); };
}
