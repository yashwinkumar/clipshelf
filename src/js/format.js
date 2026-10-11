// Pure display helpers. No DOM, no backend calls.

/** Characters shown in a card before the text is cut. Storage keeps the full text. */
export const PREVIEW_CHARS = 2000;

export function previewText(text) {
  return Array.from(text).slice(0, PREVIEW_CHARS).join('');
}

export function characterLabel(text) {
  const n = Array.from(text).length;
  return `${n.toLocaleString()} ${n === 1 ? 'character' : 'characters'}`;
}

export function relativeTime(ms, now = Date.now()) {
  const seconds = Math.max(0, Math.round((now - ms) / 1000));
  if (seconds < 60) return seconds === 1 ? '1 second ago' : `${seconds} seconds ago`;
  const minutes = Math.round(seconds / 60);
  if (minutes < 60) return minutes === 1 ? '1 minute ago' : `${minutes} minutes ago`;
  const hours = Math.round(minutes / 60);
  if (hours < 24) return hours === 1 ? '1 hour ago' : `${hours} hours ago`;
  const days = Math.round(hours / 24);
  return days === 1 ? '1 day ago' : `${days} days ago`;
}
