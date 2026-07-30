const last = { stack: null };

chrome.runtime.onMessage.addListener((msg, _sender, sendResponse) => {
  if (msg && msg.type === "thor-stack") {
    last.stack = msg.payload;
    chrome.storage.session.set({ thor_stack: msg.payload }).catch(() => {});
    sendResponse({ ok: true });
    return true;
  }
  if (msg && msg.type === "thor-stack-get") {
    sendResponse({ ok: true, stack: last.stack });
    return true;
  }
  return false;
});

chrome.action.onClicked.addListener(async (tab) => {
  const stack = last.stack || (await chrome.storage.session.get("thor_stack")).thor_stack;
  const summary = stack
    ? `Thor Lab — ${stack.url}\nreact=${stack.react} vue=${stack.vue} next=${stack.next}`
    : "Thor Lab — no stack detected yet (navigate a page first)";
  if (tab?.id != null) {
    chrome.scripting
      .executeScript({
        target: { tabId: tab.id },
        func: (text) => console.info(text),
        args: [summary],
      })
      .catch(() => {});
  }
});
