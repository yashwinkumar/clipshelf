// Short status message shown over the cards. Used for copy success and failure only.
const TIMEOUT_MS = 2600;
let timer = null;

export function showToast(message, isError = false) {
  const toast = document.getElementById('toast');
  toast.textContent = message;
  toast.classList.toggle('error', isError);
  toast.hidden = false;
  clearTimeout(timer);
  timer = setTimeout(() => {
    toast.hidden = true;
  }, TIMEOUT_MS);
}
