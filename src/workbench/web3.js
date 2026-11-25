"use strict";
const $ = id => document.getElementById(id);
const input = $("call-request");
const limit = 64 * 1024;
let busy = false, revision = 0;

function status(message, error = false) {
  $("status").textContent = message;
  $("status").dataset.error = String(error);
}

function inspectCall(request) {
  return new Promise((resolve, reject) => {
    if (typeof window.cefQuery !== "function") {
      reject(new Error("Open this inspector in Thor to decode the call."));
      return;
    }
    window.cefQuery({
      request: JSON.stringify({command: "web3_inspect_call", request}),
      onSuccess: value => {
        try { resolve(JSON.parse(value)); }
        catch { reject(new Error("Could not read the inspection result. Try again.")); }
      },
      onFailure: (_code, error) => reject(new Error(error)),
    });
  });
}

input.addEventListener("input", () => {
  revision += 1;
  $("result").textContent = "";
  status(busy ? "Request changed. Inspect again when the current check finishes." : "Ready");
});

$("call-form").addEventListener("submit", async event => {
  event.preventDefault();
  if (busy) return;
  const started = revision;
  const raw = input.value.trim();
  $("result").textContent = "";
  if (new Blob([raw]).size > limit) {
    status("Use an RPC request no larger than 64 KiB.", true);
    return;
  }
  let request;
  try { request = JSON.parse(raw); }
  catch { status("Enter valid JSON for a single eth_call request.", true); return; }

  busy = true;
  $("inspect").disabled = true;
  status("Inspecting…");
  try {
    const result = await inspectCall(request);
    if (started !== revision) return;
    $("result").textContent = JSON.stringify(result, null, 2);
    status(result.state === "decoded" ? "Call decoded" : "Selector only");
  } catch (error) {
    if (started === revision) status(error.message, true);
  } finally {
    busy = false;
    $("inspect").disabled = false;
    if (started !== revision) status("Ready");
  }
});
