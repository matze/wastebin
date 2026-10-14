function $(id) {
  return document.getElementById(id);
}

const lineNumbers = $("line-numbers");
const textarea = $("text");

const mirror = document.createElement("div");
mirror.id = "line-mirror";
mirror.setAttribute("aria-hidden", "true");
document.body.appendChild(mirror);

// Properties that influence where the text wraps, copied from the textarea.
const MIRROR_STYLE = [
  "font",
  "fontFeatureSettings",
  "fontKerning",
  "letterSpacing",
  "wordBreak",
  "wordSpacing",
  "tabSize",
  "whiteSpace",
  "overflowWrap",
];

const advanceContext = document.createElement("canvas").getContext("2d");

// Upper bound of the width a single code unit adds to a line.
function unitAdvance(font) {
  advanceContext.font = font;
  // Wide characters such as CJK, emoji and tab stops advance by at most twice
  // the step of the font, which covers them with a single bound per code unit.
  return 2 * advanceContext.measureText("M").width;
}

// Number of visual rows each of `lines` occupies when wrapped as in the textarea.
function measureRows(lines) {
  const style = getComputedStyle(textarea);

  const scrollbar =
    textarea.offsetWidth - textarea.clientWidth -
    parseFloat(style.borderLeftWidth) - parseFloat(style.borderRightWidth);
  const width = Math.max(0,
    textarea.getBoundingClientRect().width -
    parseFloat(style.borderLeftWidth) - parseFloat(style.borderRightWidth) -
    parseFloat(style.paddingLeft) - parseFloat(style.paddingRight) - scrollbar);

  const rows = new Array(lines.length).fill(1);

  // A line that cannot be wider than the text area cannot wrap either. Only
  // lay out the remaining ones, so that typing in a long text stays cheap.
  const advance = unitAdvance(style.font);
  const candidateLines = [];
  for (let i = 0; i < lines.length; i++) {
    if (lines[i].length * advance > width) {
      candidateLines.push(i);
    }
  }
  if (candidateLines.length === 0) {
    return rows;
  }

  for (const property of MIRROR_STYLE) {
    mirror.style[property] = style[property];
  }
  mirror.style.width = width + "px";

  // The lines are laid out together, so a single layout pass suffices.
  const spans = [];
  const fragment = document.createDocumentFragment();
  candidateLines.forEach(function(index, i) {
    if (i > 0) fragment.appendChild(document.createTextNode("\n"));
    const span = document.createElement("span");
    span.textContent = lines[index];
    fragment.appendChild(span);
    spans.push(span);
  });
  mirror.replaceChildren(fragment);

  spans.forEach(function(span, i) {
    // A space hanging at the end of a wrapped row forms a box of its own, so
    // count the distinct line boxes rather than the client rects.
    const boxes = new Set();
    for (const rect of span.getClientRects()) {
      boxes.add(Math.round(rect.top));
    }
    rows[candidateLines[i]] = Math.max(1, boxes.size);
  });

  return rows;
}

function updateLineNumbers() {
  const rows = measureRows(textarea.value.split("\n"));
  let html = "";
  for (let i = 0; i < rows.length; i++) {
    html += "<div>" + (i + 1) + "</div>";
  }
  lineNumbers.innerHTML = html;

  // Set through the CSSOM: the page's CSP (`style-src 'self' data:`) rejects
  // `style` attributes coming from markup, which would drop `--rows` again.
  const cells = lineNumbers.children;
  for (let i = 0; i < rows.length; i++) {
    if (rows[i] > 1) cells[i].style.setProperty("--rows", rows[i]);
  }

  syncScroll();
}

function syncScroll() {
  if (lineNumbers.scrollTop !== textarea.scrollTop) {
    lineNumbers.scrollTop = textarea.scrollTop;
  }
}

textarea.addEventListener("input", updateLineNumbers);
textarea.addEventListener("scroll", syncScroll);
updateLineNumbers();

// Wrapping changes with the available width and with the font metrics.
if (window.ResizeObserver) {
  new ResizeObserver(updateLineNumbers).observe(textarea);
}
if (document.fonts && document.fonts.ready) {
  document.fonts.ready.then(updateLineNumbers);
}

const stats = $("stats");
const MAX_BYTES = parseInt(stats.dataset.maxBytes, 10) || 1024 * 1024;
const UNIT_KB = stats.dataset.unitKb;
const UNIT_MB = stats.dataset.unitMb;
const LABEL_LIMIT = stats.dataset.labelLimit;

function formatSize(bytes) {
  if (bytes >= 1024 * 1024) return (bytes / (1024 * 1024)).toFixed(1) + " " + UNIT_MB;
  return (bytes / 1024).toFixed(0) + " " + UNIT_KB;
}

$("progress-limit").textContent = LABEL_LIMIT + " " + formatSize(MAX_BYTES);

function updateStats() {
  const text = textarea.value;
  const lines = Math.max(1, text.split("\n").length);
  const chars = text.length;
  let bytes;
  try { bytes = new Blob([text]).size; } catch(e) { bytes = text.length; }

  $("stat-lines").textContent = lines;
  $("stat-chars").textContent = chars;
  $("stat-bytes").textContent = bytes.toLocaleString();

  const pct = Math.min(100, (bytes / MAX_BYTES) * 100);
  const fill = $("progress-fill");
  fill.style.width = pct + "%";
  if (pct > 85) {
    fill.classList.add("warn");
  } else {
    fill.classList.remove("warn");
  }
  $("progress-kb").textContent = (bytes / 1024).toFixed(1) + " " + UNIT_KB;
}

textarea.addEventListener("input", updateStats);
updateStats();

const langSelect = $("langs");
const langFilter = $("filter");

langFilter.addEventListener("input", function() {
  const term = langFilter.value.toLowerCase();
  for (const opt of langSelect.options) {
    const name = opt.text.toLowerCase();
    const ext = opt.value.toLowerCase();
    opt.hidden = !(name.includes(term) || ext.includes(term));
  }
});

const encryptToggle = $("encrypt-toggle");
const passwordGroup = $("password-group");

if (encryptToggle && passwordGroup) {
  encryptToggle.addEventListener("change", function() {
    if (encryptToggle.checked) {
      passwordGroup.classList.add("shown");
      $("password").focus();
    } else {
      passwordGroup.classList.remove("shown");
      $("password").value = "";
    }
  });
}

$("burn-after-reading").addEventListener("change", function() {
  const disabled = this.checked;
  const radios = document.querySelectorAll('#expiry-list input[type="radio"]');
  for (const radio of radios) {
    radio.disabled = disabled;
  }
});

const overlay = $("drop-overlay");
let dragCounter = 0;

const editorWrap = $("editor-wrap");

editorWrap.addEventListener("dragenter", function(e) {
  e.preventDefault();
  dragCounter++;
  overlay.classList.add("active");
});

editorWrap.addEventListener("dragleave", function(e) {
  e.preventDefault();
  dragCounter--;
  if (dragCounter <= 0) {
    dragCounter = 0;
    overlay.classList.remove("active");
  }
});

editorWrap.addEventListener("dragover", function(e) {
  e.preventDefault();
});

editorWrap.addEventListener("drop", function(e) {
  e.preventDefault();
  dragCounter = 0;
  overlay.classList.remove("active");

  const files = e.dataTransfer && e.dataTransfer.files;
  if (files && files.length > 0) {
    loadFile(files[0]);
  }
});

function loadFile(file) {
  file.text().then(function(value) {
    textarea.value = value.replace(/\n$/, "");
    updateLineNumbers();
    updateStats();

    // Infer extension from filename
    const name = file.name || "";
    const dot = name.lastIndexOf(".");
    if (dot > 0) {
      const ext = name.slice(dot + 1).toLowerCase();
      for (let i = 0; i < langSelect.options.length; i++) {
        if (langSelect.options[i].value === ext) {
          langSelect.options[i].selected = true;
          break;
        }
      }
      langFilter.value = "";
      langFilter.dispatchEvent(new Event("input"));
    }

    // Set title to filename
    $("title").value = file.name;
  });
}

$("open").addEventListener("click", function() {
  const input = document.createElement("input");
  input.type = "file";
  input.onchange = function(e) {
    const file = e.target.files[0];
    if (file) loadFile(file);
  };
  input.click();
});

const INDENT = "    ";

function insertIndent() {
  // Prefer `execCommand` so the edit stays in the native undo history.
  if (!document.execCommand("insertText", false, INDENT)) {
    textarea.setRangeText(INDENT, textarea.selectionStart, textarea.selectionEnd, "end");
  }
}

function removeIndent() {
  const lineStart = textarea.value.lastIndexOf("\n", textarea.selectionStart - 1) + 1;
  const indent = textarea.value.slice(lineStart, lineStart + INDENT.length).match(/^ +/);

  if (!indent) {
    return;
  }

  const width = indent[0].length;
  const start = textarea.selectionStart;
  const end = textarea.selectionEnd;

  textarea.setSelectionRange(lineStart, lineStart + width);

  if (!document.execCommand("delete")) {
    textarea.setRangeText("", lineStart, lineStart + width, "end");
  }

  textarea.setSelectionRange(Math.max(lineStart, start - width), Math.max(lineStart, end - width));
}

textarea.addEventListener("keydown", function(e) {
  if ((e.ctrlKey || e.metaKey) && e.key === "s") {
    e.preventDefault();
    $("form").submit();
    return;
  }

  if ((e.ctrlKey || e.metaKey) && e.key === "Enter") {
    e.preventDefault();
    $("form").submit();
    return;
  }

  if (e.key === "Tab" && !e.ctrlKey && !e.metaKey && !e.altKey) {
    e.preventDefault();

    if (e.shiftKey) {
      removeIndent();
    } else {
      insertIndent();
    }

    updateLineNumbers();
    updateStats();
  }
});
