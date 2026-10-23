use std::time::Duration;

use base64::Engine;
use serde_json::{json, Value};
use tokio::sync::oneshot;

use crate::browser::session::TabRef;
use crate::engine::engine_coordinator::{
    EngineCommand, EngineError, EngineHandle, IdentityContextId, ScreenshotOptions,
};
use crate::engine::tab_session::TabId;

pub(super) async fn call(
    engine: &EngineHandle,
    name: &str,
    args: &Value,
) -> Result<Vec<Value>, String> {
    match name {
        "thor_tabs_list" => Ok(list_tabs(engine)),
        "thor_tabs_create" => create_tab(engine, args).await,
        _ => call_on_target(engine, name, args).await,
    }
}

fn list_tabs(engine: &EngineHandle) -> Vec<Value> {
    let snapshot = engine.snapshot_rx.borrow().clone();
    text(json!({ "active_target": snapshot.active_target, "tabs": snapshot.tab_details, "runtime_status": snapshot.runtime_status }).to_string())
}

async fn create_tab(engine: &EngineHandle, args: &Value) -> Result<Vec<Value>, String> {
    let url = args["url"].as_str().unwrap_or("about:blank").to_owned();
    let identity_id = IdentityContextId(args["identity_id"].as_u64().unwrap_or(0));
    let tab = request(engine, None, |reply| EngineCommand::CreateTab {
        identity_id,
        url,
        reply,
    })
    .await?;
    Ok(text(json!({ "id": tab.0 }).to_string()))
}

fn resolve_target(engine: &EngineHandle, args: &Value) -> Result<TabRef, String> {
    let snapshot = engine.snapshot_rx.borrow();
    let requested = args.get("id").and_then(Value::as_u64);
    let active = snapshot
        .active_target
        .filter(|target| requested.is_none_or(|id| target.id == id));
    let target = active
        .or_else(|| {
            snapshot
                .tab_details
                .iter()
                .find(|tab| Some(tab.target.id) == requested)
                .map(|tab| tab.target)
        })
        .ok_or("Selected tab unavailable")?;
    if args
        .get("generation")
        .and_then(Value::as_u64)
        .is_some_and(|generation| generation != target.generation)
    {
        return Err("Stale target".into());
    }
    Ok(target)
}

async fn call_on_target(
    engine: &EngineHandle,
    name: &str,
    args: &Value,
) -> Result<Vec<Value>, String> {
    let target = resolve_target(engine, args)?;
    if name == "thor_get_screenshot" {
        return screenshot(engine, target).await;
    }
    if let Some(command) = navigation_command(name, target.id, args) {
        request(engine, Some(target), command).await?;
        return Ok(text("Browser operation accepted".into()));
    }
    let script = action_script(name, args)?;
    let value = request(engine, Some(target), |reply| EngineCommand::Evaluate {
        tab_id: TabId(target.id),
        generation: target.generation,
        script,
        reply,
    })
    .await?;
    if matches!(
        name,
        "thor_click" | "thor_type" | "thor_hover" | "thor_scroll"
    ) {
        let result: Value =
            serde_json::from_str(&value).map_err(|_| "Invalid browser action result")?;
        if result["ok"] != true {
            return Err("Browser element action failed".into());
        }
    }
    Ok(text(value))
}

fn navigation_command(
    name: &str,
    id: u64,
    args: &Value,
) -> Option<impl FnOnce(oneshot::Sender<Result<(), EngineError>>) -> EngineCommand> {
    let action = match name {
        "thor_navigate" => 0,
        "thor_tabs_activate" => 1,
        "thor_tabs_close" => 2,
        "thor_back" => 3,
        "thor_forward" => 4,
        "thor_reload" => 5,
        _ => return None,
    };
    let url = args["url"].as_str().unwrap_or_default().to_owned();
    Some(move |reply| {
        let tab_id = TabId(id);
        match action {
            0 => EngineCommand::Navigate { tab_id, url, reply },
            1 => EngineCommand::ActivateTab { tab_id, reply },
            2 => EngineCommand::CloseTab { tab_id, reply },
            3 => EngineCommand::Back { tab_id, reply },
            4 => EngineCommand::Forward { tab_id, reply },
            _ => EngineCommand::Reload { tab_id, reply },
        }
    })
}

async fn screenshot(engine: &EngineHandle, target: TabRef) -> Result<Vec<Value>, String> {
    let bytes = request(engine, Some(target), |reply| {
        EngineCommand::CaptureScreenshot {
            tab_id: TabId(target.id),
            options: ScreenshotOptions {
                format: "png".into(),
                quality: 100,
            },
            reply,
        }
    })
    .await?;
    if bytes.is_empty() {
        return Err("Empty screenshot".into());
    }
    Ok(vec![
        json!({ "type": "image", "data": base64::engine::general_purpose::STANDARD.encode(bytes), "mimeType": "image/png" }),
    ])
}

async fn request<T>(
    engine: &EngineHandle,
    target: Option<TabRef>,
    command: impl FnOnce(oneshot::Sender<Result<T, EngineError>>) -> EngineCommand,
) -> Result<T, String> {
    let (reply, receiver) = oneshot::channel();
    let command = command(reply);
    let command = match target {
        Some(target) => EngineCommand::Targeted {
            target,
            command: Box::new(command),
        },
        None => command,
    };
    engine.send(command).map_err(|error| error.to_string())?;
    tokio::time::timeout(Duration::from_secs(35), receiver)
        .await
        .map_err(|_| "Browser operation timed out")?
        .map_err(|_| "Browser engine disconnected")?
        .map_err(|error| error.to_string())
}

fn action_script(name: &str, args: &Value) -> Result<String, String> {
    if name == "thor_eval_js" {
        return Ok(args["script"].as_str().ok_or("script required")?.into());
    }
    if name == "thor_get_dom" {
        let limit = args["max_chars"].as_u64().unwrap_or(200_000);
        return Ok(format!(
            "document.documentElement.outerHTML.slice(0, {limit})"
        ));
    }
    let selector = args["selector"].as_str().unwrap_or_default();
    let operation = element_operation(name, args)?;
    Ok(format!("(() => {{ const selector = {}; const el = selector ? document.querySelector(selector) : null; {} }})()", json!(selector), operation))
}

fn element_operation(name: &str, args: &Value) -> Result<String, String> {
    let action = match name {
        "thor_click" => "el.scrollIntoView({block:'center'}); if (typeof el.click === 'function') el.click(); else el.dispatchEvent(new MouseEvent('click', {bubbles:true}));".into(),
        "thor_hover" => "el.dispatchEvent(new MouseEvent('mouseover', {bubbles:true}));".into(),
        "thor_type" => format!("const text = {}; if (el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement) {{ const proto = el instanceof HTMLTextAreaElement ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype; Object.getOwnPropertyDescriptor(proto, 'value').set.call(el, text); }} else if (el.isContentEditable) el.textContent = text; else return JSON.stringify({{ok:false}}); el.dispatchEvent(new Event('input', {{bubbles:true}})); el.dispatchEvent(new Event('change', {{bubbles:true}}));", args["text"]),
        "thor_scroll" => return Ok(scroll_script(args)),
        _ => return Err("Unknown tool".into()),
    };
    Ok(format!("if (!el) return JSON.stringify({{ok:false}}); {action} return JSON.stringify({{ok:true}});"))
}

fn scroll_script(args: &Value) -> String {
    if args["selector"].as_str().unwrap_or_default().is_empty() {
        let y = args["y"].as_f64().unwrap_or(0.0);
        format!("window.scrollTo(0, {y}); return JSON.stringify({{ok:true}});")
    } else {
        "if (!el) return JSON.stringify({ok:false}); el.scrollIntoView({block:'center'}); return JSON.stringify({ok:true});".into()
    }
}

fn text(value: String) -> Vec<Value> {
    vec![json!({ "type": "text", "text": value })]
}
