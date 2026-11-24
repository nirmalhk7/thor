"use strict";
const $ = id => document.getElementById(id);
let busy = false;
let summary = "";
const limit = 256 * 1024;

function status(message, error = false) {
  $("status").textContent = message;
  $("status").dataset.error = String(error);
}

function clearResult() {
  summary = "";
  $("result").textContent = "";
  $("copy").hidden = true;
}

function inspectRequest(body) {
  return new Promise((resolve, reject) => {
    if (typeof window.cefQuery !== "function") {
      reject(new Error("Open this inspector in Thor to analyze the request."));
      return;
    }
    window.cefQuery({
      request: JSON.stringify({command:"ai_inspect_request", body}),
      onSuccess: value => { try { resolve(JSON.parse(value)); } catch { reject(new Error("Could not read the inspection result. Try again.")); } },
      onFailure: (_code, error) => reject(new Error(error)),
    });
  });
}

let inputRevision = 0;
$("request-body").addEventListener("input", () => {
  inputRevision += 1;
  clearResult();
  status(busy ? "Request changed. Inspect again when the current check finishes." : "Ready");
});

$("request-form").addEventListener("submit", async event => {
  event.preventDefault();
  if (busy) return;
  const input = $("request-body").value.trim();
  const revision = inputRevision;
  clearResult();
  if (new Blob([input]).size > limit) {
    status("Use request JSON no larger than 256 KiB.", true);
    return;
  }
  let body;
  try { body = JSON.parse(input); }
  catch { status("Enter valid JSON. Put property names and text values in double quotes.", true); return; }
  busy = true;
  $("inspect").disabled = true;
  status("Inspecting…");
  try {
    const result = await inspectRequest(body);
    if (revision !== inputRevision) return;
    summary = JSON.stringify(result, null, 2);
    $("result").textContent = summary;
    $("copy").hidden = false;
    status("Summary ready");
  } catch (error) {
    if (revision === inputRevision) status(error.message, true);
  } finally {
    busy = false;
    $("inspect").disabled = false;
    if (revision !== inputRevision) status("Ready");
  }
});

$("copy").addEventListener("click", async () => {
  if (!summary) return;
  const revision = inputRevision;
  try {
    await navigator.clipboard.writeText(summary);
    if (revision === inputRevision) status("Summary copied");
  } catch {
    if (revision === inputRevision) {
      $("result").focus();
      status("Clipboard unavailable. Select the summary and copy it.", true);
    }
  }
});
