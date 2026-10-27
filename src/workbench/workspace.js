"use strict";
document.getElementById("destination-form").addEventListener("submit", event => {
  event.preventDefault();
  const input = document.getElementById("destination");
  const error = document.getElementById("address-error");
  const value = input.value.trim();
  if (!value) { error.textContent = "Enter a website address to continue."; input.focus(); return; }
  try {
    const local = /^(localhost|127\.0\.0\.1|\[::1\])(?=[:/]|$)/i.test(value);
    const url = new URL(/^[a-z][a-z\d+.-]*:\/\//i.test(value) ? value : `${local ? "http" : "https"}://${value}`);
    if (!['http:', 'https:'].includes(url.protocol) || !url.hostname || /\s/.test(value)) throw new Error();
    window.location.assign(url.href);
  } catch { error.textContent = "Enter a valid HTTP or HTTPS address."; input.focus(); }
});
if (!/Mac/i.test(navigator.platform)) {
  for (const [id, key] of [["shortcut-address", "L"], ["shortcut-tab", "T"], ["shortcut-close", "W"]]) document.getElementById(id).textContent = `Ctrl ${key}`;
}

if (!/Mac|iPhone|iPad/.test(navigator.platform)) { document.getElementById("shortcut-sites").textContent="Ctrl K"; document.getElementById("shortcut-tools").textContent="Ctrl Shift K"; }
