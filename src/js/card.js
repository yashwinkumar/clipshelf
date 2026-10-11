// One clipboard card. Builds DOM with textContent only, so clipboard text is never parsed as HTML.
import { appIcon } from './icons.js';
import { characterLabel, previewText, relativeTime } from './format.js';

/** Muted source colours. Unknown apps use a muted fern green.
 * This is a Clipshelf palette, not Paste's unknown colour rule.
 */
export function tintFor(key) {
  const source = String(key || '').toLowerCase();
  if (source.includes('warp')) return { bg: '#536149', fg: '#FFFFFF' };
  if (source.includes('openai') || source.includes('chatgpt')) {
    return { bg: '#3d6558', fg: '#FFFFFF' };
  }
  return { bg: '#4b6945', fg: '#FFFFFF' };
}

function el(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

/**
 * @param row     history row from the backend (id, text, captured_at_ms, source_*)
 * @param actions { onSelect(li), onCopy(id) }
 */
export function createCard(row, actions) {
  const li = el('li', 'card');
  li.tabIndex = 0;
  li.setAttribute('role', 'option');
  li.setAttribute('aria-selected', 'false');
  li.dataset.id = String(row.id);
  li.dataset.at = String(row.captured_at_ms);

  const tint = tintFor(row.source_bundle_id || row.source_name);
  const header = el('div', 'card-header');
  header.style.background = tint.bg;
  header.style.color = tint.fg;
  header.append(
    el('div', 'card-title', 'Text'),
    el('div', 'card-time', relativeTime(row.captured_at_ms)),
  );

  const icon = el('div', 'card-icon');
  const initial = (row.source_name || row.source_bundle_id || '?').trim().charAt(0).toUpperCase();
  icon.textContent = initial || '?';
  header.append(icon);
  appIcon(row.source_bundle_id).then((src) => {
    if (!src) return;
    const img = el('img');
    img.alt = '';
    img.src = src;
    icon.replaceChildren(img);
  });

  const body = el('div', 'card-body');
  body.append(el('p', 'card-text', previewText(row.text)));

  const footer = el('div', 'card-footer', characterLabel(row.text));

  const copy = el('button', 'card-copy');
  copy.type = 'button';
  copy.title = 'Copy';
  copy.setAttribute('aria-label', 'Copy this item');
  copy.innerHTML =
    '<svg viewBox="0 0 16 16" aria-hidden="true"><rect x="5.5" y="5.5" width="8" height="8" rx="1.6" fill="none" stroke="currentColor" stroke-width="1.4"/><path d="M3.5 10.5h-.6A1.4 1.4 0 0 1 1.5 9.1V3.4A1.4 1.4 0 0 1 2.9 2h5.7a1.4 1.4 0 0 1 1.4 1.4v.6" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/></svg>';
  copy.addEventListener('click', (event) => {
    event.stopPropagation();
    actions.onCopy(row.id);
  });

  li.append(header, body, footer, copy);

  li.addEventListener('click', () => actions.onSelect(li));
  li.addEventListener('keydown', (event) => {
    if (event.target === li && event.key === ' ') {
      event.preventDefault();
      actions.onSelect(li);
    }
  });
  return li;
}

/** Refresh the relative times shown on cards. */
export function refreshTimes(list, now = Date.now()) {
  for (const li of list.querySelectorAll('.card')) {
    const time = li.querySelector('.card-time');
    if (time) time.textContent = relativeTime(Number(li.dataset.at), now);
  }
}
