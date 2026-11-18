use super::perf_features;
use super::{json, ok, opt, req, reqv, FeatureEngine, Value};

pub(in crate::features) fn perf_budgets(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    let budgets: crate::load::PerformanceBudgets =
        if let Some(b) = opt::<crate::load::PerformanceBudgets>(p, "budgets") {
            b
        } else {
            crate::load::PerformanceBudgets::core_web_vitals()
        };
    let vitals: Option<crate::load::BrowserMetrics> = if opt::<bool>(p, "from_active_tab")
        .unwrap_or(false)
    {
        let _ = f.evaluate_on_active(
            crate::features::cdp_bridge::CdpBridge::web_vitals_injection_script(),
        );
        let raw = f.evaluate_on_active(
                            "(function(){ return (typeof window.__thorCollectWebVitals==='function') ? window.__thorCollectWebVitals() : '{}'; })()",
                        )?;
        crate::features::cdp_bridge::CdpBridge::parse_web_vitals(&raw)
    } else {
        opt(p, "vitals")
    };
    let load: Option<crate::load::LoadMetrics> = opt(p, "load");
    let resources: Option<perf_features::ResourceBudgetInputs> = opt(p, "resources");
    ok(f.perf().check_budgets_with_resources(
        &budgets,
        vitals.as_ref(),
        load.as_ref(),
        resources.as_ref(),
    ))
}

pub(in crate::features) fn rum_lab(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    ok(f.perf().rum_vs_lab(&req(p, "lab")?, &req(p, "rum")?))
}

pub(in crate::features) fn inp_attribution(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    if opt::<bool>(p, "from_active_tab").unwrap_or(false) {
        let raw = f.evaluate_on_active(
                        r#"(function(){
  var out=[];
  try {
    var list=performance.getEntriesByType('event')||[];
    for (var i=0;i<list.length;i++){
      var e=list[i];
      if (!e || (e.name!=='click' && e.name!=='keydown' && e.name!=='pointerdown')) continue;
      var target=(e.target&& (e.target.id?('#'+e.target.id):(e.target.tagName||'node').toLowerCase()))||'unknown';
      var processing=(e.processingEnd&&e.processingStart)?(e.processingEnd-e.processingStart):0;
      var inputDelay=(e.processingStart&&e.startTime)?(e.processingStart-e.startTime):0;
      var presentation=(e.duration||0)-processing-inputDelay;
      if (presentation<0) presentation=0;
      out.push({target:target, input_delay_ms:inputDelay, processing_ms:processing, presentation_ms:presentation});
    }
  } catch(err){}
  return JSON.stringify(out);
})()"#,
                    )?;
        let interactions: Vec<perf_features::InteractionEvent> =
            serde_json::from_str(&raw).map_err(|e| format!("invalid INP probe JSON: {e}"))?;
        ok(f.perf().attribute_inp(&interactions))
    } else {
        ok(f.perf().attribute_inp(&reqv(p, "interactions")?))
    }
}

pub(in crate::features) fn vitals_under_load(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    ok(json!({
        "knee_vus": f.perf().vitals_knee(
            &req::<Vec<(u32, f64)>>(p, "samples")?,
            opt(p, "lcp_budget_ms").unwrap_or(2500.0),
        )
    }))
}

pub(in crate::features) fn lcp_element_forensics(
    f: &FeatureEngine,
    p: &Value,
) -> Result<Value, String> {
    ok(f.perf().lcp_element_forensics(&req(p, "payload")?))
}

pub(in crate::features) fn soft_nav_vitals(f: &FeatureEngine, p: &Value) -> Result<Value, String> {
    let records = if opt::<bool>(p, "from_active_tab").unwrap_or(false) {
        let _ = f.evaluate_on_active(
            crate::features::cdp_bridge::CdpBridge::soft_nav_and_speculation_script(),
        );
        let raw = f.evaluate_on_active(
                        "(function(){ return (typeof window.__thorCollectSoftNavs==='function') ? window.__thorCollectSoftNavs() : '[]'; })()",
                    )?;
        crate::features::cdp_bridge::CdpBridge::parse_soft_navs(&raw)
    } else {
        reqv(p, "records")?
    };
    ok(f.perf().soft_nav_vitals(&records)?)
}
