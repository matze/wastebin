document.addEventListener("DOMContentLoaded", function() {
  const toggle = document.getElementById("password-toggle");
  const input = document.getElementById("password");
  if (!toggle || !input) return;

  toggle.addEventListener("click", function() {
    const show = input.type === "password";
    input.type = show ? "text" : "password";
    toggle.classList.toggle("shown", show);
    const label = show ? toggle.dataset.hideLabel : toggle.dataset.showLabel;
    toggle.title = label;
    toggle.setAttribute("aria-label", label);
  });
});
