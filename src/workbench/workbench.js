"use strict";
const $ = id => document.getElementById(id);
const state = { document: null, auth: { type: "none" }, response: null, pending: null };
function query(command) {
  let id;
  const promise = new Promise((resolve, reject) => {
    if (typeof window.cefQuery !== "function") { reject(new Error("Thor tool connection unavailable. Reopen this tool in Thor.")); return; }
    id = window.cefQuery({ request: JSON.stringify(command), onSuccess: text => {
      try { resolve(JSON.parse(text)); } catch { reject(new Error("Invalid tool response")); }
    }, onFailure: (_code, message) => reject(new Error(message)) });
  });
  return { promise, cancel: () => { if (id !== undefined) window.cefQueryCancel(id); } };
}
async function call(command) { return query(command).promise; }
function message(text, error = false) { $("message").textContent = text; $("message").dataset.error = String(error); }
function pairs(text, separator) {
  return text.split(/\r?\n/).filter(line => line.trim()).map((line, index) => {
    const split = line.indexOf(separator);
    if (split < 1) throw new Error(`Line ${index + 1} needs a name${separator}value pair`);
    return { name: line.slice(0, split).trim(), value: line.slice(split + 1).trim() };
  });
}
async function authentication() {
  if ($("auth-type").value === "none") return { type: "none" };
  if ($("token").value) {
    const result = await call({ command: "secret_store", value: $("token").value, persistent: $("persist-token").checked });
    state.auth = { type: "bearer", token: result.reference };
    $("token").value = "";
    $("auth-status").textContent = $("persist-token").checked ? "Credential reference stored in Keychain." : "Temporary credential available for this session.";
  }
  if (state.auth.type !== "bearer") throw new Error("Enter a bearer token before sending or saving");
  return state.auth;
}
async function requestSpec() {
  const type = $("body-type").value, text = $("body").value;
  let body = { type: "empty" };
  if (type === "json") body = { type, value: JSON.parse(text) };
  if (type === "text") body = { type, text, content_type: "text/plain" };
  if (type === "form") body = { type, entries: pairs(text, "=") };
  if (type === "binary") body = { type, data: text, content_type: "application/octet-stream" };
  return { method: $("method").value, url: $("url").value, headers: pairs($("headers").value, ":"), query: pairs($("params").value, "="), body, auth: await authentication() };
}
function variables() {
  const values = JSON.parse($("variables").value);
  if (!values || Array.isArray(values) || typeof values !== "object" || Object.values(values).some(value => typeof value !== "string")) throw new Error("Run variables must be a JSON object of string values");
  return { overrides: values };
}
function renderResponse(response) {
  state.response = response;
  $("response-summary").textContent = `${response.status} · ${response.total_ms.toFixed(1)} ms · ${response.body_bytes.toLocaleString()} bytes`;
  $("response-header-text").textContent = response.headers.map(header => `${header.name}: ${header.value}`).join("\n");
  $("response-headers").hidden = false;
  let body = response.body.data;
  if (response.body.encoding === "utf8") { try { body = JSON.stringify(JSON.parse(body), null, 2); } catch {} }
  $("response-body").textContent = body;
  $("copy-response").disabled = false;
  message(response.body.encoding === "base64" ? "Binary response shown as lossless base64." : "Request completed.");
}
async function send(event) {
  event.preventDefault();
  if (state.pending) return;
  $("send").disabled = true;
  $("response-summary").textContent = "Sending…";
  message("Sending request…");
  try {
    const request = await requestSpec();
    const operation = query({ command: "api_send", request, variables: variables() });
    state.pending = operation;
    $("cancel").disabled = false;
    renderResponse(await operation.promise);
  } catch (error) { message(error.message, true); $("response-summary").textContent = "Request failed or cancelled"; }
  finally { state.pending = null; $("send").disabled = false; $("cancel").disabled = true; }
}
async function projects(selected) {
  const values = await call({ command: "project_list" });
  $("project").replaceChildren(new Option("Select a project", ""));
  for (const project of values) $("project").add(new Option(project.name, project.id));
  if (selected) $("project").value = selected;
  else if (values.length) $("project").value = values[0].id;
  await requests();
}
async function requests() {
  $("requests").replaceChildren();
  if (!$("project").value) { $("requests").textContent = "Create a project to keep requests."; return; }
  const values = await call({ command: "request_list", project: $("project").value });
  if (!values.length) $("requests").textContent = "Save your first request here.";
  for (const item of values) {
    const button = document.createElement("button");
    button.type = "button"; button.textContent = item.title; button.title = item.title;
    button.setAttribute("aria-current", String(state.document?.id === item.id));
    button.addEventListener("click", () => openRequest(item.id).catch(error => message(error.message, true)));
    $("requests").append(button);
  }
}
async function openRequest(id) {
  const item = await call({ command: "request_open", id });
  state.document = item; state.auth = item.request.auth;
  $("method").value = item.request.method; $("url").value = item.request.url; $("request-title").value = item.title;
  $("headers").value = item.request.headers.map(header => `${header.name}: ${header.value}`).join("\n");
  $("params").value = item.request.query.map(entry => `${entry.name}=${entry.value}`).join("\n");
  const body = item.request.body;
  $("body-type").value = body.type; $("body").disabled = body.type === "empty";
  $("body").value = body.type === "json" ? JSON.stringify(body.value, null, 2) : body.type === "form" ? body.entries.map(entry => `${entry.name}=${entry.value}`).join("\n") : body.text || body.data || "";
  $("auth-type").value = state.auth.type === "bearer" ? "bearer" : "none"; $("token").value = ""; $("token").disabled = $("auth-type").value === "none";
  $("auth-status").textContent = state.auth.type === "bearer" ? "Saved credential reference selected. Enter a token to replace it." : "No saved authentication.";
  $("dirty").textContent = "Saved"; $("save-status").textContent = "";
  await requests();
}
$("request-form").addEventListener("submit", send);
$("cancel").addEventListener("click", () => { state.pending?.cancel(); });
$("project").addEventListener("change", () => { state.document = null; requests().catch(error => message(error.message, true)); });
$("project-form").addEventListener("submit", async event => {
  event.preventDefault();
  try { const project = await call({ command: "project_create", name: $("project-name").value }); $("project-name").value = ""; await projects(project.id); }
  catch (error) { message(error.message, true); }
});
$("save").addEventListener("click", async () => {
  $("save").disabled = true;
  try {
    if (!$("project").value) throw new Error("Create or select a project first");
    if (!$("request-title").value.trim()) throw new Error("Name this request before saving");
    const command = { command: "request_save", project: $("project").value, title: $("request-title").value, request: await requestSpec() };
    if (state.document) { command.id = state.document.id; command.revision = state.document.revision; }
    const result = await call(command); state.document = result;
    $("dirty").textContent = "Saved"; $("save-status").textContent = "Saved to project";
    await requests();
  } catch (error) { message(error.message, true); }
  finally { $("save").disabled = false; }
});
$("new-request").addEventListener("click", () => {
  state.document = null; state.auth = { type: "none" }; $("request-form").reset(); $("method").value = "GET"; $("variables").value = "{}";
  $("token").disabled = true; $("body").disabled = true; $("request-title").value = ""; $("dirty").textContent = "Unsaved"; $("save-status").textContent = ""; $("url").focus();
});
$("body-type").addEventListener("change", () => { $("body").disabled = $("body-type").value === "empty"; });
$("auth-type").addEventListener("change", () => { state.auth = { type: "none" }; $("token").disabled = $("auth-type").value === "none"; $("token").value = ""; });
$("request-form").addEventListener("input", () => { $("dirty").textContent = "Unsaved changes"; });
for (const tab of document.querySelectorAll("[data-panel]")) tab.addEventListener("click", () => {
  for (const item of document.querySelectorAll("[data-panel]")) { const active = item === tab; item.setAttribute("aria-selected", String(active)); $("panel-" + item.dataset.panel).hidden = !active; }
});
$("copy-response").addEventListener("click", async () => {
  try { await navigator.clipboard.writeText(state.response.body.data); message("Response body copied."); } catch { message("Copy unavailable. Select the response body and copy it.", true); }
});
document.addEventListener("keydown", event => { if ((event.metaKey || event.ctrlKey) && event.key === "Enter") { event.preventDefault(); $("request-form").requestSubmit(); } });
projects().catch(error => message(error.message, true));
