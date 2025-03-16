function $(id) {
  return document.getElementById(id);
}

function dropHandler(ev) {
  ev.preventDefault();

  if (ev.dataTransfer.items) {
    const item = ev.dataTransfer.items[0];

    if (item.kind === 'file') {
      item.getAsFile().text().then((value) => $("text").value = value);
    }
  } else {
    const item = ev.dataTransfer.files[0];
    item.text().then((value) => $("text").value = value);
  }
}

function dragOverHandler(ev) {
  ev.preventDefault();
}

function keyDownHandler(ev) {
  if (ev.ctrlKey && ev.key == 's') {
    ev.preventDefault();

    $("text").form.submit();
  }
}

function openFile() {
  let input = document.createElement("input");
  input.type = "file";
  input.onchange = ev => {
    const item = ev.target.files[0];
    let titleInput = $("title");

    // Iterate through the `langs` <select> and
    // try to match the value with the extension. If we have one, select it.
    const extension = item.name.split(".").pop().toLowerCase();
    const langSelect = $("langs");

    for (i = 0; i < langSelect.length; i++) {
      if (langSelect[i].value == extension) {
        langSelect[i].selected = true;
        break;
      }
    }

    // Set title to the filename.
    titleInput.value = item.name;

    // Set <textarea> to file content.
    item.text().then((value) => $("text").value = value);
  };

  input.click();
}

function filterLangs(ev) {
  ev.preventDefault();
  let langs = $("langs");
  const term = $("filter").value.toLowerCase();

  for (option of langs) {
    if (option.innerText.toLowerCase().includes(term)) {
      option.style.display = "";
    }
    else {
      option.style.display = "none";
    }
  }
}

function burnCheckboxHandler() {
  $("expiration-list").disabled = $("burn-after-reading").checked;
}

$("text").addEventListener("drop", dropHandler);
$("text").addEventListener("dragover", dragOverHandler);
$("text").addEventListener("keydown", keyDownHandler);
$("open").addEventListener("click", openFile);
$("filter").addEventListener("change", filterLangs);
$("filter").addEventListener("keyup", filterLangs);
$("burn-after-reading").addEventListener("click", burnCheckboxHandler);
