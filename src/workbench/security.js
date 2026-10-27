"use strict";
const $ = id => document.getElementById(id);
let activeTool = "headers";
const tools = {
  headers: { title: "Security headers", description: "Review response headers for missing browser protections and configuration issues.", label: "Response headers", placeholder: "Paste response headers, one Name: value per line.", action: "Analyze headers", slug: "header-analyzer" },
  jwt: { title: "JWT inspector", description: "Decode a token and inspect its claims and expiry. Decoding does not verify its signature.", label: "JSON Web Token", placeholder: "Paste a JWT to inspect locally.", action: "Inspect token", slug: "jwt-deep" },
  diff: { title: "Response diff", description: "Compare two HTTP snapshots to inspect status, header, and body differences.", label: "First response (JSON)", placeholder: '{"status":200,"headers":{},"body":"response text"}', action: "Compare responses", slug: "response-diff" }
};
function selectTool(name) {
  activeTool = name;
  const tool = tools[name];
  $("tool-title").textContent = tool.title; $("tool-description").textContent = tool.description;
  $("input-label").textContent = tool.label; $("analysis-input").placeholder = tool.placeholder;
  $("analysis-input").value = ""; $("comparison-input").value = ""; $("result").textContent = "";
  $("analyze").textContent = tool.action; $("status").textContent = "Ready"; $("status").dataset.error = "false";
  $("second-input").hidden = name !== "diff"; $("comparison-input").required = name === "diff";
  $("analyzer").setAttribute("aria-labelledby", `tool-${name}`);
  for (const button of document.querySelectorAll("[data-tool]")) {
    const active = button.dataset.tool === name;
    button.setAttribute("aria-selected", String(active)); button.tabIndex = active ? 0 : -1;
  }
}
for (const button of document.querySelectorAll("[data-tool]")) {
  button.addEventListener("click", () => selectTool(button.dataset.tool));
  button.addEventListener("keydown", event => {
    const buttons = Array.from(document.querySelectorAll("[data-tool]"));
    let index = buttons.indexOf(button);
    if (["ArrowDown", "ArrowRight"].includes(event.key)) index = (index + 1) % buttons.length;
    else if (["ArrowUp", "ArrowLeft"].includes(event.key)) index = (index - 1 + buttons.length) % buttons.length;
    else if (event.key === "Home") index = 0;
    else if (event.key === "End") index = buttons.length - 1;
    else return;
    event.preventDefault(); selectTool(buttons[index].dataset.tool); buttons[index].focus();
  });
}
$("analysis-form").addEventListener("submit", async event => {
  event.preventDefault();
  const tool = tools[activeTool];
  $("analyze").disabled = true; $("status").dataset.error = "false"; $("status").textContent = "Analyzing…";
  try {
    const input = $("analysis-input").value.trim();
    let payload;
    if (activeTool === "jwt") payload = { jwt: input, now_unix: Math.floor(Date.now() / 1000) };
    else if (activeTool === "diff") payload = { a: JSON.parse(input), b: JSON.parse($("comparison-input").value) };
    else {
      const headers = Object.create(null);
      for (const [index, line] of input.split(/\r?\n/).entries()) {
        if (!line.trim() || /^HTTP\/\S+\s+\d+/.test(line)) continue;
        const split = line.indexOf(":");
        if (split < 1) throw new Error(`Header line ${index + 1} needs Name: value.`);
        headers[line.slice(0, split).trim()] = line.slice(split + 1).trim();
      }
      payload = { headers };
    }
    if (typeof window.cefQuery !== "function") throw new Error("Open this analyzer in Thor to connect to its Rust engine.");
    const result = await new Promise((resolve, reject) => window.cefQuery({request: JSON.stringify({ command: "security_analyze", slug: tool.slug, payload }), onSuccess: value => { try { resolve(JSON.parse(value)); } catch { reject(new Error("Invalid analyzer response.")); } }, onFailure: (_code, error) => reject(new Error(error))}));
    $("result").textContent = JSON.stringify(result.result, null, 2); $("status").textContent = "Analysis complete";
  } catch (error) { $("status").textContent = error.message; $("status").dataset.error = "true"; $("result").textContent = ""; }
  finally { $("analyze").disabled = false; }
});

function selectMenuTool() {
  const name = window.location.hash.slice(1);
  if (Object.hasOwn(tools, name) && name !== activeTool) selectTool(name);
}
window.addEventListener("hashchange", selectMenuTool);
selectMenuTool();
