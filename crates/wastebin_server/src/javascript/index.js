function $(id) {
  return document.getElementById(id);
}

const lineNumbers = $("line-numbers");
const textarea = $("text");

function updateLineNumbers() {
  const count = Math.max(1, textarea.value.split("\n").length);
  let html = "";
  for (let i = 1; i <= count; i++) {
    html += "<div>" + i + "</div>";
  }
  lineNumbers.innerHTML = html;
}

function syncScroll() {
  lineNumbers.scrollTop = textarea.scrollTop;
}

textarea.addEventListener("input", updateLineNumbers);
textarea.addEventListener("scroll", syncScroll);
updateLineNumbers();

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

textarea.addEventListener("keydown", function(e) {
  if ((e.ctrlKey || e.metaKey) && e.key === "s") {
    e.preventDefault();
    $("form").submit();
  }
  if ((e.ctrlKey || e.metaKey) && e.key === "Enter") {
    e.preventDefault();
    $("form").submit();
  }
});
