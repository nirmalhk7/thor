use std::future::Future;
use std::sync::OnceLock;

use tokio::runtime::Runtime;

/// Compatibility for synchronous feature handlers. Native workbench uses async
/// execution directly; this runtime survives individual calls and pool reuse.
pub(crate) fn run<F>(future: F) -> Result<F::Output, String>
where
    F: Future + Send,
    F::Output: Send,
{
    static RUNTIME: OnceLock<Result<Runtime, std::io::Error>> = OnceLock::new();
    let runtime = RUNTIME.get_or_init(|| tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2).thread_name("thor-api").enable_all().build())
        .as_ref().map_err(|_| "API execution runtime unavailable".to_string())?;
    std::thread::scope(|scope| scope.spawn(|| runtime.block_on(future)).join())
        .map_err(|_| "API execution worker failed".into())
}
