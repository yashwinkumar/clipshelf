// Wiring: search, the card row, copy feedback and window events. No layout here.
import { available, invoke, listen } from './bridge.js';
import { createCard, refreshTimes } from './card.js';
import { showToast } from './feedback.js';

const LIMIT = 200;
const SEARCH_DEBOUNCE_MS = 150;
const TIME_REFRESH_MS = 30_000;

const search = document.getElementById('search');
const clearButton = document.getElementById('clear-search');
const hideButton = document.getElementById('hide');
const pauseButton = document.getElementById('pause');
const clearHistoryButton = document.getElementById('clear-history');
const clearDialog = document.getElementById('clear-dialog');
const confirmClear = document.getElementById('confirm-clear');
const cancelClear = document.getElementById('cancel-clear');
let capturePaused = false;
const list = document.getElementById('cards');
const empty = document.getElementById('empty');

let requestSeq = 0;
let searchTimer = null;
let selectedId = null;

function setEmpty(message) {
  empty.textContent = message || '';
  empty.hidden = !message;
}

function select(li) {
  for (const other of list.querySelectorAll('.card[aria-selected="true"]')) {
    other.setAttribute('aria-selected', 'false');
  }
  li.setAttribute('aria-selected', 'true');
  selectedId = li.dataset.id;
  li.scrollIntoView({ block: 'nearest', inline: 'nearest' });
}

async function copy(id) {
  try {
    const result = await invoke('copy_history_entry', { id });
    const message = `Copied ${result.chars.toLocaleString()} characters. Press Cmd+V to paste.`;
    showToast(result.acknowledged ? message : `${message} Another app changed the clipboard at the same time.`);
  } catch (err) {
    showToast(`Copy failed: ${err}`, true);
  }
}

function renderRows(rows, query) {
  const focusedCard = document.activeElement?.closest('.card');
  const restoreFocus = Boolean(focusedCard);
  list.replaceChildren();
  const actions = { onSelect: select, onCopy: copy };
  for (const row of rows) {
    list.append(createCard(row, actions));
  }
  const cards = Array.from(list.children);
  const active = cards.find(card => card.dataset.id === selectedId) || cards[0];
  if (active) {
    select(active);
    if (restoreFocus) active.focus({ preventScroll: true });
  } else {
    selectedId = null;
  }
  if (rows.length === 0) {
    setEmpty(query ? 'Nothing found' : 'Nothing copied yet. Copy some text to start.');
  } else {
    setEmpty('');
  }
}

async function load() {
  const seq = ++requestSeq;
  const query = search.value.trim();
  if (!available) {
    setEmpty('Tauri API not available.');
    return;
  }
  try {
    const rows = await invoke('search_history', { query: search.value, limit: LIMIT });
    if (seq !== requestSeq) return; // a newer search has started
    renderRows(rows, query);
  } catch (err) {
    if (seq !== requestSeq) return;
    list.replaceChildren();
    setEmpty(`History unavailable: ${err}`);
  }
}

function updateClear() {
  clearButton.hidden = search.value.length === 0;
}

search.addEventListener('input', () => {
  updateClear();
  clearTimeout(searchTimer);
  searchTimer = setTimeout(() => { searchTimer = null; load(); }, SEARCH_DEBOUNCE_MS);
});

clearButton.addEventListener('click', () => {
  search.value = '';
  updateClear();
  search.focus();
  load();
});

hideButton.addEventListener('click', () => {
  invoke('hide_panel').catch(() => {});
});

// Central keyboard handling preserves text editing inside the search field.
document.getElementById('search-form').addEventListener('submit', event => event.preventDefault());
document.addEventListener('keydown', event => {
  if (event.isComposing || clearDialog.open) return;
  const command = event.metaKey || event.ctrlKey;
  const inSearch = event.target === search;
  const cards = Array.from(list.children);
  const current = cards.findIndex(card => card.dataset.id === selectedId);
  if (command && event.key.toLowerCase() === 'f') {
    event.preventDefault();
    search.focus();
    search.select();
    return;
  }
  if (event.key === 'Escape') {
    event.preventDefault();
    invoke('hide_panel').catch(() => {});
    return;
  }
  // Let input cursor movement, selection and Cmd+C retain their normal meaning.
  if (inSearch && !['ArrowDown', 'Enter'].includes(event.key)) return;
  if (event.altKey || event.shiftKey) return;
  let next = null;
  if (!command && (event.key === 'ArrowRight' || event.key === 'ArrowDown')) {
    next = inSearch ? Math.max(0, current) : Math.min(cards.length - 1, current + 1);
  } else if (!command && (event.key === 'ArrowLeft' || event.key === 'ArrowUp')) {
    next = Math.max(0, current - 1);
  } else if (!command && event.key === 'Home') {
    next = 0;
  } else if (!command && event.key === 'End') {
    next = cards.length - 1;
  }
  if (next !== null) {
    event.preventDefault();
    if (cards[next]) { select(cards[next]); cards[next].focus({ preventScroll: true }); }
    return;
  }
  const onButton = event.target.closest?.('button');
  if (onButton) return; // native button activation must not also copy a card
  if (event.key === 'Enter' || (command && event.key.toLowerCase() === 'c')) {
    event.preventDefault();
    if (event.repeat) return;
    // Flush a pending search before copying so Enter cannot copy a stale result.
    clearTimeout(searchTimer);
    searchTimer = null;
    const ready = inSearch ? load() : Promise.resolve();
    ready.then(() => {
      const card = list.querySelector('.card[aria-selected="true"]');
      if (card) copy(Number(card.dataset.id));
    });
  }
});

function updatePause(paused) {
  capturePaused = paused;
  const label = paused ? 'Resume capture' : 'Pause capture';
  pauseButton.setAttribute('aria-pressed', String(paused));
  pauseButton.setAttribute('aria-label', label);
  pauseButton.title = label;
  pauseButton.innerHTML = paused
    ? '<svg viewBox="0 0 16 16" aria-hidden="true"><path d="m5 3 8 5-8 5Z" fill="currentColor"/></svg>'
    : '<svg viewBox="0 0 16 16" aria-hidden="true"><path d="M5 3v10M11 3v10" stroke="currentColor" stroke-width="2" stroke-linecap="round"/></svg>';
}
pauseButton.addEventListener('click', async () => {
  pauseButton.disabled = true;
  try {
    const paused = !capturePaused;
    await invoke('set_capture_paused', { paused });
    updatePause(paused);
    showToast(paused ? 'Capture paused.' : 'Capture resumed.');
  } catch (_) {
    showToast('Capture state could not be changed. Try again.', true);
  } finally { pauseButton.disabled = false; }
});
listen('capture-paused', event => updatePause(event.payload));
invoke('capture_paused').then(updatePause).catch(() => {});

// New captures refresh the list only when they were stored.
listen('clipboard-event', (event) => {
  if (event.payload.stored === 'stored') load();
  if (event.payload.stored === 'storage_error' || event.payload.stored === 'unavailable') {
    showToast('History could not be saved. Check available disk space and restart Clipshelf.', true);
  }
  if (event.payload.skip_reason === 'read_failed') {
    showToast('Copied text could not be read. Check macOS clipboard access and copy again.', true);
  }
});

// The window is reused between shows: focus the search and reload each time.
window.addEventListener('focus', () => {
  search.focus();
  load();
});

setInterval(() => refreshTimes(list), TIME_REFRESH_MS);
updateClear();
load();

clearHistoryButton.addEventListener('click', () => {
  document.getElementById('clear-error').hidden = true;
  clearDialog.showModal();
});
clearDialog.addEventListener('cancel', event => {
  if (confirmClear.disabled) event.preventDefault();
});
cancelClear.addEventListener('click', () => clearDialog.close());
clearDialog.addEventListener('close', () => clearHistoryButton.focus());
confirmClear.addEventListener('click', async () => {
  confirmClear.disabled = true;
  cancelClear.disabled = true;
  try {
    const count = await invoke('clear_history', { confirmed: true });
    clearDialog.close();
    showToast(`Cleared ${count.toLocaleString()} ${count === 1 ? 'item' : 'items'}.`);
  } catch (_) {
    const error = document.getElementById('clear-error');
    error.textContent = 'History could not be cleared. Try again.';
    error.hidden = false;
  } finally {
    confirmClear.disabled = false;
    cancelClear.disabled = false;
  }
});
listen('history-cleared', () => {
  clearTimeout(searchTimer);
  searchTimer = null;
  ++requestSeq;
  selectedId = null;
  search.value = '';
  updateClear();
  list.replaceChildren();
  load();
});
