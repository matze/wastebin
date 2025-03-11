function $(id) {
  return document.getElementById(id);
}

document.addEventListener('keydown', onKey);
$("copy-button").addEventListener("click", copy);

function copy() {
  const lines = document.querySelectorAll('td.line');
  const content = Array.from(lines)
    .map(line => line.textContent)
    .join('')
    .trim();

  navigator.clipboard.writeText(content)
    .then(() => {
        let toast = $("toast");

        toast.classList.toggle("hidden");
        toast.classList.toggle("shown");

        setTimeout(() => {
          toast.classList.toggle("hidden");
          toast.classList.toggle("shown");
        }, 1500);
    }, function(err) {
      console.error("failed to copy content", err);
    });
}

function onKey(e) {
  if (e.key == 'n') {
    window.location.href = "/";
  }
  else if (e.key == 'r') {
    window.location.href = "/raw" + window.location.pathname;
  }
  else if (e.key == 'y') {
    navigator.clipboard.writeText(window.location.href);
  }
  else if (e.key == 'd') {
    window.location.href = "/dl" + window.location.pathname;
  }
  else if (e.key == 'q') {
    window.location.href = "/qr" + window.location.pathname;
  }
  else if (e.key == 'p') {
    window.location.href = window.location.href.split("?")[0];
  }
  else if (e.key == 'c') {
    copy();
  }
  else if (e.key == '?') {
    var overlay = document.getElementById("overlay");

    overlay.style.display = overlay.style.display != "block" ? "block" : "none";
    overlay.onclick = function() {
      if (overlay.style.display == "block") {
        overlay.style.display = "none";
      }
    };
  }

  if (e.keyCode == 27) {
    var overlay = document.getElementById("overlay");

    if (overlay.style.display == "block") {
      overlay.style.display = "none";
    }
  }
}
