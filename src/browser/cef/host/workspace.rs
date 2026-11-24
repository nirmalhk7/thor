use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use ::cef::wrapper::byte_read_handler::{ByteReadHandler, ByteStream};
use ::cef::wrapper::message_router::*;
use ::cef::wrapper::stream_resource_handler::StreamResourceHandler;
use ::cef::*;

use crate::api::{CancellationToken, HttpService};
use crate::workbench::{self, Workbench, WorkbenchCommand};

type Pending = Arc<Mutex<HashMap<i64, (CancellationToken, Arc<Mutex<dyn BrowserSideCallback>>)>>>;

pub(super) fn start() -> anyhow::Result<Arc<BrowserSideRouter>> {
    let router = BrowserSideRouter::new(MessageRouterConfig::default());
    let handler = ToolHandler {
        http: HttpService::new()?,
        pending: Arc::default(),
    };
    router
        .add_handler(Arc::new(handler), false)
        .ok_or_else(|| anyhow::anyhow!("CEF workspace handler unavailable"))?;
    Ok(router)
}

pub(super) fn register_context(context: &RequestContext) -> anyhow::Result<()> {
    let mut factory = WorkspaceFactory::new();
    if context.register_scheme_handler_factory(
        Some(&CefString::from("thor")),
        Some(&CefString::from("workbench")),
        Some(&mut factory),
    ) != 1
    {
        anyhow::bail!("CEF workspace scheme unavailable");
    }
    Ok(())
}

struct ToolHandler {
    http: HttpService,
    pending: Pending,
}

impl BrowserSideHandler for ToolHandler {
    // Approved Qlty function-parameters exception for this callback only:
    // CEF's BrowserSideHandler trait requires all six parameters.
    fn on_query_str(
        &self,
        _browser: Option<Browser>,
        frame: Option<Frame>,
        query_id: i64,
        request: &str,
        persistent: bool,
        callback: Arc<Mutex<dyn BrowserSideCallback>>,
    ) -> bool {
        let allowed = frame.as_ref().is_some_and(|frame| {
            workbench::is_tool_document(
                &CefString::from(&frame.url()).to_string(),
                frame.is_main() == 1,
            )
        });
        if !allowed || persistent || request.len() > 8 * 1024 * 1024 {
            callback
                .lock()
                .unwrap()
                .failure(403, "Workspace request rejected");
            return true;
        }
        let command = match serde_json::from_str::<WorkbenchCommand>(request) {
            Ok(command)
                if frame.as_ref().is_some_and(|frame| {
                    workbench::command_allowed(
                        &CefString::from(&frame.url()).to_string(),
                        frame.is_main() == 1,
                        &command,
                    )
                }) =>
            {
                command
            }
            _ => {
                callback
                    .lock()
                    .unwrap()
                    .failure(400, "Unsupported workspace command");
                return true;
            }
        };
        let cancel = CancellationToken::new();
        self.pending
            .lock()
            .unwrap()
            .insert(query_id, (cancel.clone(), callback));
        let pending = self.pending.clone();
        let http = self.http.clone();
        tokio::spawn(async move {
            let result = match crate::AppState::global().projects() {
                Ok(projects) => {
                    Workbench::new(&http, projects)
                        .execute_cancellable(command, &cancel)
                        .await
                }
                Err(error) => Err(error),
            };
            let mut queries = pending.lock().unwrap();
            if let Some((_, callback)) = queries.remove(&query_id) {
                let callback = callback.lock().unwrap();
                match result {
                    Ok(value) => callback.success_str(&value.to_string()),
                    Err(error) => callback.failure(400, &error),
                }
            }
        });
        true
    }

    fn on_query_canceled(&self, _browser: Option<Browser>, _frame: Option<Frame>, query_id: i64) {
        if let Some((cancel, _)) = self.pending.lock().unwrap().remove(&query_id) {
            cancel.cancel();
        }
    }
}

wrap_scheme_handler_factory! {
    struct WorkspaceFactory;
    impl SchemeHandlerFactory {
        fn create(&self, _browser: Option<&mut Browser>, _frame: Option<&mut Frame>, _scheme_name: Option<&CefString>, request: Option<&mut Request>) -> Option<ResourceHandler> {
            let request = request?;
            if CefString::from(&request.method()).to_string() != "GET" { return None; }
            let asset = workbench::asset(&CefString::from(&request.url()).to_string())?;
            let mut reader = ByteReadHandler::new(Arc::new(Mutex::new(ByteStream::new(asset.bytes.to_vec()))));
            let stream = stream_reader_create_for_handler(Some(&mut reader))?;
            let mut headers = CefStringMultimap::new();
            headers.append("Content-Security-Policy", workbench::CSP);
            headers.append("Cache-Control", "no-store");
            headers.append("X-Content-Type-Options", "nosniff");
            Some(StreamResourceHandler::new(200, "OK".into(), asset.mime.into(), Some(headers), Some(stream)))
        }
    }
}

thread_local! {
    static RENDERER: Arc<RendererSideRouter> = RendererSideRouter::new(MessageRouterConfig::default());
}

wrap_render_process_handler! {
    pub(super) struct WorkspaceRenderer;
    impl RenderProcessHandler {
        fn on_context_created(&self, browser: Option<&mut Browser>, frame: Option<&mut Frame>, context: Option<&mut V8Context>) {
            if let (Some(frame), Some(context)) = (frame.as_ref(), context.as_ref()) {
                super::page_activity::install(frame, context);
            }
            if frame.as_ref().is_some_and(|frame| workbench::is_tool_document(&CefString::from(&frame.url()).to_string(), frame.is_main() == 1)) {
                RENDERER.with(|router| router.on_context_created(browser.cloned(), frame.cloned(), context.cloned()));
            }
        }
        fn on_context_released(&self, browser: Option<&mut Browser>, frame: Option<&mut Frame>, context: Option<&mut V8Context>) {
            if let Some(frame) = frame.as_ref() {
                super::page_activity::report(frame, false);
            }
            RENDERER.with(|router| router.on_context_released(browser.cloned(), frame.cloned(), context.cloned()));
        }
        fn on_process_message_received(&self, browser: Option<&mut Browser>, frame: Option<&mut Frame>, source_process: ProcessId, message: Option<&mut ProcessMessage>) -> i32 {
            RENDERER.with(|router| i32::from(router.on_process_message_received(browser.cloned(), frame.cloned(), Some(source_process), message.cloned())))
        }
    }
}
