const textarea = document.getElementById('text');

function dropHandler(ev) {
  ev.preventDefault();

  if (ev.dataTransfer.items) {
    const item = ev.dataTransfer.items[0];

    if (item.kind === 'file') {
      item.getAsFile().text().then((value) => textarea.value = value);
    }
  } else {
    const item = ev.dataTransfer.files[0];
    item.text().then((value) => textarea.value = value);
  }
}

textarea.addEventListener('drop', dropHandler);

function dragOverHandler(ev) {
  ev.preventDefault();
}

textarea.addEventListener('dragover', dragOverHandler);

function keyDownHandler(ev) {
  if (ev.ctrlKey && ev.key == 's') {
    ev.preventDefault();

    textarea.form.submit();
  }
}

textarea.addEventListener('keydown', keyDownHandler);

function openFile() {
  let input = document.createElement("input");
  input.type = "file";
  input.onchange = ev => {
    const item = ev.target.files[0];

    item.text().then((value) => textarea.value = value);
  };

  input.click();
}

const openbutton = document.getElementById('open');
openbutton.addEventListener('click', openFile);

const filter = document.getElementById("filter");

function filterLangs(ev) {
  ev.preventDefault();
  let langs = document.getElementById("langs");
  const term = filter.value.toLowerCase();

  for (option of langs) {
    if (option.innerText.toLowerCase().includes(term)) {
      option.style.display = "";
    }
    else {
      option.style.display = "none";
    }
  }
}

filter.addEventListener('change', filterLangs);
filter.addEventListener('keyup', filterLangs);
