function $(id) {
  return document.getElementById(id);
}

document.addEventListener('keydown', onKey);
$("copy-button").addEventListener("click", copy);

function highlightLines(scroll) {
  document.querySelectorAll('.line-highlight').forEach(el => {
    el.classList.remove('line-highlight');
  });

  const match = window.location.hash.match(/^#L(\d+)(?:-L(\d+))?$/);
  if (!match) return;

  const a = parseInt(match[1], 10);
  const b = match[2] ? parseInt(match[2], 10) : a;
  const from = Math.min(a, b);
  const to = Math.max(a, b);

  for (let i = from; i <= to; i++) {
    const lnDiv = document.getElementById('L' + i);
    if (lnDiv) lnDiv.classList.add('line-highlight');
    const lcDiv = document.getElementById('LC' + i);
    if (lcDiv) lcDiv.classList.add('line-highlight');
  }

  if (scroll && match[2]) {
    const firstLn = document.getElementById('L' + from);
    if (firstLn) firstLn.scrollIntoView({ block: 'center' });
  }
}

window.addEventListener('hashchange', () => highlightLines(true));
highlightLines(true);

document.querySelectorAll('#line-numbers a').forEach(a => {
  a.addEventListener('click', (e) => {
    if (!e.shiftKey) return;
    const m = a.getAttribute('href').match(/^#L(\d+)$/);
    const current = window.location.hash.match(/^#L(\d+)(?:-L\d+)?$/);
    if (!m || !current) return;
    e.preventDefault();
    const clicked = parseInt(m[1], 10);
    const base = parseInt(current[1], 10);
    const from = Math.min(base, clicked);
    const to = Math.max(base, clicked);
    history.replaceState(null, '', from === to ? '#L' + from : '#L' + from + '-L' + to);
    highlightLines(false);
  });
});

function showToast(text, timeout) {
  let toast = $("toast");

  toast.innerText = text;
  toast.classList.toggle("hidden");
  toast.classList.toggle("shown");

  setTimeout(() => {
    toast.classList.toggle("hidden");
    toast.classList.toggle("shown");
  }, timeout);
}

function copy() {
  const code = document.querySelector('.src-code code');
  if (!code) return;
  const content = code.textContent.trim();

  navigator.clipboard.writeText(content)
    .then(() => {
      showToast("Copied content", 1500);
    }, function(err) {
      console.error("failed to copy content", err);
    });
}

function onKey(e) {
  if (e.keyCode == 27) {
    const overlay = document.getElementById("overlay");
    if (overlay && overlay.style.display == "block") {
      overlay.style.display = "none";
    }
    return;
  }

  if (e.ctrlKey || e.metaKey) {
    return;
  }

  const pasteId = document.body.dataset.pasteId;

  if (e.key == 'n') {
    window.location.href = "/";
  }
  else if (e.key == 'r' && pasteId) {
    window.location.href = "/raw/" + pasteId;
  }
  else if (e.key == 'y') {
    navigator.clipboard.writeText(window.location.href);
    showToast("Copied URL", 1500);
  }
  else if (e.key == 'd' && pasteId) {
    window.location.href = "/dl/" + pasteId;
  }
  else if (e.key == 'q' && pasteId) {
    window.location.href = "/qr/" + pasteId;
  }
  else if (e.key == 'p') {
    window.location.href = window.location.href.split("?")[0];
  }
  else if (e.key == 'c') {
    copy();
  }
  else if (e.key == 'w') {
    document.body.classList.toggle('line-wrap');
  }
  else if (e.key == 'm') {
    const toggle = document.getElementById('view-toggle');
    if (toggle) window.location.href = toggle.href;
  }
  else if (e.key == '?') {
    toggleOverlay();
  }
}

function buildOverlay() {
  const rows = [
    ['n', 'Go home'],
    ['p', 'Go here'],
    ['y', 'Copy URL'],
    ['c', 'Copy content'],
    ['d', 'Download'],
    ['q', 'Show QR code'],
    ['w', 'Toggle line wrapping'],
  ];
  if (document.getElementById('view-toggle')) {
    rows.push(['m', 'Toggle rendered view']);
  }
  rows.push(['?', 'Toggle help']);

  const overlay = document.createElement('div');
  overlay.id = 'overlay';
  const content = document.createElement('div');
  content.id = 'overlay-content';
  const table = document.createElement('table');
  for (const [key, label] of rows) {
    const tr = document.createElement('tr');
    const tdKey = document.createElement('td');
    const kbd = document.createElement('kbd');
    kbd.textContent = key;
    tdKey.appendChild(kbd);
    const tdLabel = document.createElement('td');
    tdLabel.textContent = label;
    tr.appendChild(tdKey);
    tr.appendChild(tdLabel);
    table.appendChild(tr);
  }
  content.appendChild(table);
  overlay.appendChild(content);
  overlay.addEventListener('click', () => { overlay.style.display = 'none'; });
  document.body.appendChild(overlay);
  return overlay;
}

function toggleOverlay() {
  const overlay = document.getElementById('overlay') || buildOverlay();
  overlay.style.display = overlay.style.display != 'block' ? 'block' : 'none';
}
