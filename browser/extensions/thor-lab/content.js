// Detect common frameworks on the page and post a summary to the SW.
(function () {
  try {
    const react =
      !!(window.__REACT_DEVTOOLS_GLOBAL_HOOK__ && window.__REACT_DEVTOOLS_GLOBAL_HOOK__.renderers &&
        window.__REACT_DEVTOOLS_GLOBAL_HOOK__.renderers.size) ||
      !!document.querySelector("[data-reactroot],[data-reactid]") ||
      !!(window.React && window.React.version);
    const vue = !!(window.__VUE__ || window.Vue || document.querySelector("[data-v-]"));
    const next = !!(window.__NEXT_DATA__);
    const nuxt = !!(window.__NUXT__);
    chrome.runtime.sendMessage({
      type: "thor-stack",
      payload: {
        url: location.href,
        react: !!react,
        vue: !!vue,
        next: !!next,
        nuxt: !!nuxt,
        title: document.title || "",
      },
    });
  } catch (e) {
    /* ignore cross-origin / CSP */
  }
})();
