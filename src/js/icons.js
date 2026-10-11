// Source-app icons, fetched once per bundle identifier through the backend.
// The icons are the installed apps' own icons, not Paste assets.
import { invoke } from './bridge.js';

const cache = new Map();

/** Resolves to a PNG data URL, or null when the app is unknown or not installed. */
export function appIcon(bundleId) {
  if (!bundleId) return Promise.resolve(null);
  if (!cache.has(bundleId)) {
    cache.set(
      bundleId,
      invoke('app_icon', { bundleId }).catch(() => null),
    );
  }
  return cache.get(bundleId);
}
