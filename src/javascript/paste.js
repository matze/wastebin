const BASE_PATH = document.getElementById('js_base_path').innerHTML;
const BASE_PATH_ID = document.getElementById('js_base_path_id').innerHTML;
const EXT = document.getElementById('js_ext').innerHTML;

document.addEventListener('keydown', onKey);

function onKey(e) {
  if (e.key == 'n') {
    window.location.href = BASE_PATH;
  }
  else if (e.key == 'r') {
    window.location.href = `${BASE_PATH_ID}?fmt=raw`;
  }
  else if (e.key == 'y') {
    navigator.clipboard.writeText(window.location.href);
  }
  else if (e.key == 'd') {
    window.location.href = `${BASE_PATH_ID}?dl=${EXT}`;
  }
  else if (e.key == 'q') {
    window.location.href = `${BASE_PATH_ID}?fmt=qr`;
  }
  else if (e.key == 'p') {
    window.location.href = `${BASE_PATH_ID}`;
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
