function $(id) {
  return document.getElementById(id);
}

window.onload = function() {
  // Read cookie first and check the preference.
  const value = document.cookie.match('(^|;)\\s*pref\\s*=\\s*([^;]+)')?.pop() || '';

  if (value == "dark") {
    $("dark-switch").style.display = "none";
    $("light-switch").style.display = "block";
    return;
  }

  if (value == "light") {
    $("dark-switch").style.display = "block";
    $("light-switch").style.display = "none";
    return;
  }

  // We have no cookie, so check the system preference which _should_ match what
  // we see at the moment.
  if (window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches) {
    $("dark-switch").style.display = "none";
    $("light-switch").style.display = "block";
  }
  else {
    $("dark-switch").style.display = "block";
    $("light-switch").style.display = "none";
  }
};
