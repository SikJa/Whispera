import { listen, type EventCallback, type UnlistenFn } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';

// Global listeners receive even events emitted to a different window in Tauri.
// Every capture surface must subscribe to its own native window explicitly.
export function listenCapture<T>(event: string, handler: EventCallback<T>): Promise<UnlistenFn> {
  return listen(event, handler, { target: getCurrentWindow().label });
}
