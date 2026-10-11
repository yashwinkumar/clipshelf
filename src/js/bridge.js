// The only place that touches the Tauri globals. Everything else calls these helpers.

const tauri = window.__TAURI__;

export const available = Boolean(tauri && tauri.core && tauri.event);

export function invoke(command, args) {
  if (!available) {
    return Promise.reject(new Error('Tauri API not available'));
  }
  return tauri.core.invoke(command, args);
}

export function listen(event, handler) {
  if (!available) {
    return Promise.resolve(() => {});
  }
  return tauri.event.listen(event, handler);
}
