"use strict";
const $ = id => document.getElementById(id);
let features = [], selected = null, busy = false;
function query(command) {
  return new Promise((resolve, reject) => {
    if (typeof window.cefQuery !== "function") { reject(new Error("Open Developer Control Center in Thor to connect to its engine.")); return; }
    window.cefQuery({request: JSON.stringify(command), onSuccess: value => {try {resolve(JSON.parse(value));} catch {reject(new Error("Invalid engine response."));}}, onFailure: (_code, error) => reject(new Error(error))});
  });
}
function render() {
  const terms = $("feature-search").value.toLowerCase().trim().split(/\s+/);
  const matches = features.filter(feature => terms.every(term => `${feature.name} ${feature.slug} ${feature.category}`.toLowerCase().includes(term)));
  $("feature-count").textContent = `${matches.length} of ${features.length} registered features`;
  $("features").replaceChildren();
  for (const feature of matches) {
    const button = document.createElement("button"); button.type = "button"; button.className = "feature-choice";
    button.setAttribute("aria-pressed", String(selected?.slug === feature.slug));
    button.disabled = busy;
    const title = document.createElement("span"); title.textContent = feature.name;
    const detail = document.createElement("small"); detail.textContent = `${feature.category} · ${feature.enabled ? feature.status : "Disabled"}`;
    button.append(title, detail); button.addEventListener("click", () => selectFeature(feature)); $("features").append(button);
  }
  if (!matches.length) { const empty = document.createElement("p"); empty.textContent = "No features match. Try another name or domain."; $("features").append(empty); }
}
function selectFeature(feature) {
  selected = feature; $("feature-title").textContent = feature.name;
  $("feature-description").textContent = `${feature.category} · ${feature.slug}`;
  $("feature-payload").value = "{}"; $("feature-result").textContent = "";
  $("feature-run").disabled = !feature.enabled; $("feature-status").dataset.error = "false";
  $("feature-status").textContent = feature.enabled ? "Ready" : "Feature disabled"; render();
}
$("feature-search").addEventListener("input", render);
$("feature-form").addEventListener("submit", async event => {
  event.preventDefault(); if (!selected || busy) return;
  busy = true; $("feature-run").disabled = true; $("feature-status").dataset.error = "false"; $("feature-status").textContent = "Running…"; render();
  try {
    const payload = JSON.parse($("feature-payload").value);
    if (!payload || typeof payload !== "object" || Array.isArray(payload)) throw new Error("Input must be a JSON object.");
    const response = await query({command:"feature_invoke", slug:selected.slug, payload});
    $("feature-result").textContent = JSON.stringify(response, null, 2); $("feature-status").textContent = "Complete";
  } catch (error) { $("feature-status").textContent = error.message; $("feature-status").dataset.error = "true"; $("feature-result").textContent = ""; }
  finally { busy = false; $("feature-run").disabled = !selected.enabled; render(); }
});
function selectMenuFeature() {
  const slug = window.location.hash.slice(1);
  if (!slug) return;
  const feature = features.find(feature => feature.slug === slug);
  if (feature) {
    if (selected?.slug !== slug) selectFeature(feature);
    $("features").querySelector('[aria-pressed="true"]')?.scrollIntoView({block:"nearest"});
  } else {
    $("feature-status").textContent = "Feature unavailable. Choose a registered feature.";
    $("feature-status").dataset.error = "true";
  }
}
window.addEventListener("hashchange", selectMenuFeature);
query({command:"feature_list"}).then(result => {if (!Array.isArray(result)) throw new Error("Invalid feature registry."); features = result; render(); selectMenuFeature();}).catch(error => {$("feature-count").textContent = error.message; $("feature-count").dataset.error = "true";});
