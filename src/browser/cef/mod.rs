//! CEF owns embedded renderers; Thor owns the window, controls, and tab session.

mod host;
#[cfg(target_os = "macos")]
mod mac;

use std::path::{Path, PathBuf};

use ::cef::{args::Args, *};

/// Dispatch subprocesses before initializing logging, Tokio, or application UI.
pub fn run() -> anyhow::Result<()> {
    let executable = std::env::current_exe()?;
    let helper = std::env::args().any(|arg| arg.starts_with("--type="));
    let _framework = load_framework(&executable, helper)?;
    let _ = api_hash(sys::CEF_API_VERSION_LAST, 0);
    let args = Args::new();
    #[cfg(target_os = "macos")]
    if !helper {
        mac::initialize()?;
    }
    #[cfg(target_os = "macos")]
    let _sandbox = helper.then(|| {
        let mut sandbox = sandbox::Sandbox::new();
        sandbox.initialize(args.as_main_args());
        sandbox
    });
    let status = execute_process(Some(args.as_main_args()), None, std::ptr::null_mut());
    if status >= 0 {
        if status != 0 {
            anyhow::bail!("CEF subprocess exited with {status}");
        }
        return Ok(());
    }
    let _logger = crate::logging::init_logging();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let _guard = runtime.enter();
    let root = crate::paths::thor_data_dir().join("CEF");
    std::fs::create_dir_all(&root)?;
    host::prepare(root.clone());
    let settings = Settings {
        root_cache_path: CefString::from(root.to_string_lossy().as_ref()),
        cache_path: CefString::from(root.join("Default").to_string_lossy().as_ref()),
        persist_session_cookies: 1,
        log_severity: LogSeverity::DISABLE,
        user_agent_product: CefString::from("Thor"),
        background_color: 0xff121318,
        ..Default::default()
    };
    let mut app = host::ThorApp::new();
    if initialize(
        Some(args.as_main_args()),
        Some(&settings),
        Some(&mut app),
        std::ptr::null_mut(),
    ) != 1
    {
        host::release();
        anyhow::bail!("CEF initialization failed");
    }
    run_message_loop();
    host::release();
    shutdown();
    Ok(())
}

#[cfg(target_os = "macos")]
struct Framework;

#[cfg(target_os = "macos")]
impl Drop for Framework {
    fn drop(&mut self) {
        unload_library();
    }
}

#[cfg(target_os = "macos")]
fn load_framework(executable: &Path, helper: bool) -> anyhow::Result<Framework> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let directory = executable
        .parent()
        .ok_or_else(|| anyhow::anyhow!("CEF executable has no parent"))?;
    let relative = if helper { "../../.." } else { "../Frameworks" };
    let bundled = directory
        .join(relative)
        .join("Chromium Embedded Framework.framework/Chromium Embedded Framework");
    let path = framework_path(bundled)?;
    let path = CString::new(path.as_os_str().as_bytes())?;
    if load_library(Some(unsafe { &*path.as_ptr() })) != 1 {
        anyhow::bail!("CEF framework could not be loaded");
    }
    Ok(Framework)
}

#[cfg(not(target_os = "macos"))]
fn load_framework(_executable: &Path, _helper: bool) -> anyhow::Result<()> {
    Ok(())
}

fn framework_path(bundled: PathBuf) -> anyhow::Result<PathBuf> {
    let path = if bundled.is_file() {
        bundled
    } else if let Some(root) = std::env::var_os("CEF_PATH") {
        PathBuf::from(root)
            .join("Chromium Embedded Framework.framework/Chromium Embedded Framework")
    } else {
        anyhow::bail!("CEF framework missing; run the packaged Thor app or set CEF_PATH to the pinned CEF distribution");
    };
    Ok(path.canonicalize()?)
}
