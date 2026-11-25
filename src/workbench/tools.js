"use strict";
const $ = id => document.getElementById(id);
const choices = new Map();
let features = [], selected = null, selectionRevision = 0, busy = false;
const waiting = "Another feature is running. Wait to run this one.";

function query(command) {
  return new Promise((resolve, reject) => {
    if (typeof window.cefQuery !== "function") { reject(new Error("Open Developer Control Center in Thor to connect to its engine.")); return; }
    window.cefQuery({request: JSON.stringify(command), onSuccess: value => { try { resolve(JSON.parse(value)); } catch { reject(new Error("Invalid engine response.")); } }, onFailure: (_code, error) => reject(new Error(error))});
  });
}

function syncSelection() {
  for (const [slug, button] of choices) {
    button.setAttribute("aria-pressed", String(selected?.slug === slug));
  }
  $("feature-run").disabled = busy || !selected?.enabled;
}

function status(message, error = false) {
  $("feature-status").textContent = message;
  $("feature-status").dataset.error = String(error);
}

function readyStatus() {
  if (!selected) return "Choose a feature";
  if (!selected.enabled) return "Feature disabled";
  return busy ? waiting : "Ready";
}

function renderList() {
  const terms = $("feature-search").value.toLowerCase().trim().split(/\s+/);
  const matches = features.filter(feature => terms.every(term => `${feature.name} ${feature.slug} ${feature.category}`.toLowerCase().includes(term)));
  $("feature-count").textContent = `${matches.length} of ${features.length} registered features`;
  $("features").replaceChildren();
  choices.clear();
  for (const feature of matches) {
    const button = document.createElement("button"); button.type = "button"; button.className = "feature-choice";
    const title = document.createElement("span"); title.textContent = feature.name;
    const detail = document.createElement("small"); detail.textContent = `${feature.category} · ${feature.enabled ? feature.status : "Disabled"}`;
    button.append(title, detail); button.addEventListener("click", () => selectFeature(feature)); $("features").append(button);
    choices.set(feature.slug, button);
  }
  if (!matches.length) { const empty = document.createElement("p"); empty.textContent = "No features match. Try another name or domain."; $("features").append(empty); }
  syncSelection();
}

function selectFeature(feature) {
  if (selected?.slug === feature?.slug) return;
  selected = feature;
  selectionRevision += 1;
  $("feature-title").textContent = feature?.name ?? "Developer Control Center";
  $("feature-description").textContent = feature ? `${feature.category} · ${feature.slug}` : "Choose a registered feature to inspect captured evidence with Thor’s Rust engine.";
  $("feature-payload").value = "{}"; $("feature-result").textContent = "";
  status(readyStatus());
  // Keep the focused choice in the DOM instead of rebuilding the sidebar.
  syncSelection();
}

$("feature-search").addEventListener("input", renderList);
$("feature-form").addEventListener("submit", async event => {
  event.preventDefault();
  if (!selected?.enabled || busy) return;
  const { slug } = selected;
  const revision = selectionRevision;
  busy = true; syncSelection(); status("Running…");
  try {
    let payload;
    try { payload = JSON.parse($("feature-payload").value); }
    catch { throw new Error("Enter valid JSON. Property names and text values must use double quotes."); }
    if (!payload || typeof payload !== "object" || Array.isArray(payload)) throw new Error("Input must be a JSON object.");
    const response = await query({command:"feature_invoke", slug, payload});
    if (revision !== selectionRevision) return;
    $("feature-result").textContent = JSON.stringify(response, null, 2);
    status("Complete");
  } catch (error) {
    if (revision !== selectionRevision) return;
    status(error.message, true); $("feature-result").textContent = "";
  } finally {
    busy = false;
    syncSelection();
    if (revision !== selectionRevision && $("feature-status").textContent === waiting) status(readyStatus());
  }
});

function selectMenuFeature() {
  const slug = window.location.hash.slice(1);
  if (!slug) return;
  const feature = features.find(feature => feature.slug === slug);
  if (feature) {
    selectFeature(feature);
    choices.get(slug)?.scrollIntoView({block:"nearest"});
  } else {
    selectFeature(null);
    status("Feature unavailable. Choose a registered feature.", true);
  }
}

window.addEventListener("hashchange", selectMenuFeature);
query({command:"feature_list"}).then(result => { if (!Array.isArray(result)) throw new Error("Invalid feature registry."); features = result; renderList(); selectMenuFeature(); }).catch(error => { $("feature-count").textContent = error.message; $("feature-count").dataset.error = "true"; });
