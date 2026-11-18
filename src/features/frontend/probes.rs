use super::FrontendEngine;

impl FrontendEngine {
    /// Probe script: collect focusable nodes in document order for
    /// [`parse_focus_order_probe`] → [`focus_order_overlay`].
    /// Probe React DevTools global hook when present (no extension required).
    pub fn react_devtools_probe_script() -> &'static str {
        r#"(function(){
  var hook = window.__REACT_DEVTOOLS_GLOBAL_HOOK__;
  if (!hook) return JSON.stringify({ present:false, renderers:0, roots:[] });
  var renderers = 0;
  try { renderers = hook.renderers ? hook.renderers.size : 0; } catch(e) {}
  var roots = [];
  try {
    if (hook.renderers) {
      hook.renderers.forEach(function(r){
        try {
          var version = (r && r.version) || (r && r.reconcilerVersion) || '?';
          roots.push({ version: String(version) });
        } catch(e2){}
      });
    }
  } catch(e3){}
  return JSON.stringify({ present:true, renderers: renderers, roots: roots, supportsFiber: !!hook.supportsFiber });
})()"#
    }

    /// Collect buffered Layout Shift entries for CLS heatmap (`from_active_tab`).
    pub fn cls_entries_probe_script() -> &'static str {
        r#"(function(){
  var entries = [];
  try {
    var list = performance.getEntriesByType('layout-shift') || [];
    for (var i = 0; i < list.length; i++) {
      var e = list[i];
      if (e.hadRecentInput) continue;
      var node = 'unknown';
      try {
        if (e.sources && e.sources[0] && e.sources[0].node) {
          var n = e.sources[0].node;
          node = n.id ? ('#' + n.id) : (n.tagName || 'node').toLowerCase();
        }
      } catch (err) {}
      entries.push({ node: node, score: e.value || 0, had_recent_input: !!e.hadRecentInput });
    }
  } catch (e2) {}
  return JSON.stringify(entries);
})()"#
    }

    /// Best-effort accessibility DOM probe for `from_active_tab`.
    pub fn a11y_dom_probe_script() -> &'static str {
        r#"(function(){
  function walk(el, depth){
    if(!el || depth>10) return null;
    var tag=(el.tagName||'').toLowerCase();
    if(!tag || tag==='script' || tag==='style' || tag==='noscript') return null;
    var attrs={};
    if(el.attributes){
      for(var i=0;i<el.attributes.length;i++){
        attrs[el.attributes[i].name]=el.attributes[i].value;
      }
    }
    var kids=[];
    var children=el.children||[];
    for(var j=0;j<children.length;j++){
      var n=walk(children[j], depth+1);
      if(n) kids.push(n);
    }
    var text='';
    var nodes=el.childNodes||[];
    for(var k=0;k<nodes.length;k++){
      if(nodes[k].nodeType===3) text+=nodes[k].textContent||'';
    }
    text=text.replace(/\s+/g,' ').trim();
    return {tag:tag, attributes:attrs, text:text||null, children:kids};
  }
  var root=walk(document.body, 0);
  return JSON.stringify(root ? [root] : []);
})()"#
    }

    /// Probe script: collect focusable nodes in document order for
    /// [`parse_focus_order_probe`] → [`focus_order_overlay`].
    pub fn focus_order_probe_script() -> &'static str {
        r#"(() => {
  const FOCUSABLE = 'a[href],button,input,select,textarea,summary,[tabindex],[contenteditable="true"]';
  const nodes = [];
  let hint = 0;
  document.querySelectorAll(FOCUSABLE).forEach((el) => {
    const style = window.getComputedStyle(el);
    const rect = el.getBoundingClientRect();
    const hidden = style.visibility === 'hidden' || style.display === 'none'
      || (rect.width === 0 && rect.height === 0);
    let tab = el.tabIndex;
    if (tab === undefined || tab === null) tab = 0;
    const sel = el.id ? ('#' + el.id)
      : (el.getAttribute('name') ? (el.tagName.toLowerCase() + '[name="' + el.getAttribute('name') + '"]')
      : (el.tagName.toLowerCase() + ':nth-of-type(' + ([].indexOf.call(el.parentNode ? el.parentNode.children : [], el) + 1) + ')'));
    nodes.push({
      selector: sel,
      tab_index: tab,
      visible: !hidden,
      disabled: !!(el.disabled || el.getAttribute('aria-disabled') === 'true'),
      order_hint: hint++
    });
  });
  return JSON.stringify(nodes);
})()"#
    }

    /// Best-effort CSS rule coverage probe for `from_active_tab` (stylesheet walk).
    /// Prefer CDP `CSS.takeCoverageDelta` JSON when available.
    pub fn css_coverage_probe_script() -> &'static str {
        r#"(() => {
  const rules = [];
  for (const sheet of document.styleSheets) {
    let list;
    try { list = sheet.cssRules; } catch (e) { continue; }
    if (!list) continue;
    for (const rule of list) {
      if (!rule.selectorText) continue;
      const sel = rule.selectorText;
      let used = false;
      try {
        const first = sel.split(',')[0].trim();
        if (first) used = !!document.querySelector(first);
      } catch (e2) {}
      rules.push({
        selector: sel,
        used: used,
        bytes: (rule.cssText || '').length
      });
    }
  }
  return JSON.stringify(rules);
})()"#
    }
}
