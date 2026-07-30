//! Production-grade structured logging engine for Thor.
//!
//! Provides colorized console logging, non-blocking rotating log files
//! (`thor_data_dir()/logs/thor.log`), an in-memory ring buffer for DevCC/IPC log inspection,
//! dynamic level filtering (`THOR_LOG` / `RUST_LOG`), and panic capture hooks.

use std::collections::VecDeque;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::SystemTime;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use crate::paths::thor_data_dir;

/// Logging severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Trace = 0,
    Debug = 1,
    Info = 2,
    Warn = 3,
    Error = 4,
}

impl LogLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Trace => "TRACE",
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERROR",
        }
    }

    pub fn ansi_color(&self) -> &'static str {
        match self {
            LogLevel::Trace => "\x1b[90m", // Dim Gray
            LogLevel::Debug => "\x1b[35m", // Magenta
            LogLevel::Info => "\x1b[36m",  // Cyan
            LogLevel::Warn => "\x1b[33m",  // Yellow
            LogLevel::Error => "\x1b[31;1m", // Bold Red
        }
    }
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for LogLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "trace" => Ok(LogLevel::Trace),
            "debug" => Ok(LogLevel::Debug),
            "info" => Ok(LogLevel::Info),
            "warn" | "warning" => Ok(LogLevel::Warn),
            "error" | "err" => Ok(LogLevel::Error),
            other => Err(format!("Unknown log level: {other}")),
        }
    }
}

/// A structured single log event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogRecord {
    pub timestamp_rfc3339: String,
    pub level: LogLevel,
    pub target: String,
    pub message: String,
    pub file: Option<String>,
    pub line: Option<u32>,
}

impl LogRecord {
    pub fn new(
        level: LogLevel,
        target: impl Into<String>,
        message: impl Into<String>,
        file: Option<&str>,
        line: Option<u32>,
    ) -> Self {
        let now = SystemTime::now();
        let timestamp_rfc3339 = humantime_format_rfc3339(now);
        Self {
            timestamp_rfc3339,
            level,
            target: target.into(),
            message: message.into(),
            file: file.map(|s| s.to_string()),
            line,
        }
    }

    pub fn format_console(&self, use_color: bool) -> String {
        let file_info = match (&self.file, self.line) {
            (Some(f), Some(l)) => format!(" [{}:{}]", f, l),
            (Some(f), None) => format!(" [{}]", f),
            _ => String::new(),
        };

        if use_color {
            format!(
                "{}\x1b[90m[{}]\x1b[0m {}{:5}\x1b[0m \x1b[32m[{}]\x1b[0m {}{}\x1b[0m",
                LogLevel::Trace.ansi_color(),
                self.timestamp_rfc3339,
                self.level.ansi_color(),
                self.level.as_str(),
                self.target,
                self.message,
                file_info
            )
        } else {
            format!(
                "[{}] {:5} [{}] {}{}",
                self.timestamp_rfc3339,
                self.level.as_str(),
                self.target,
                self.message,
                file_info
            )
        }
    }
}

fn humantime_format_rfc3339(time: SystemTime) -> String {
    let duration = time
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();
    let millis = duration.subsec_millis();

    // Standard UTC time calculation
    let days = secs / 86400;
    let rem_secs = secs % 86400;
    let hours = rem_secs / 3600;
    let mins = (rem_secs % 3600) / 60;
    let s = rem_secs % 60;

    // Epoch (1970-01-01) day counts
    let (year, month, day) = days_to_date(days);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        year, month, day, hours, mins, s, millis
    )
}

fn days_to_date(days: u64) -> (u64, u64, u64) {
    // Simplified Days-to-YMD calculation for UTC timestamps post 1970
    let z = days + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

/// Thread-safe in-memory ring buffer for storing recent log records.
pub struct LogBuffer {
    capacity: usize,
    buffer: RwLock<VecDeque<LogRecord>>,
}

impl LogBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            buffer: RwLock::new(VecDeque::with_capacity(capacity)),
        }
    }

    pub fn push(&self, record: LogRecord) {
        let mut guard = self.buffer.write();
        if guard.len() >= self.capacity {
            guard.pop_front();
        }
        guard.push_back(record);
    }

    pub fn query(
        &self,
        min_level: Option<LogLevel>,
        target_filter: Option<&str>,
        limit: usize,
    ) -> Vec<LogRecord> {
        let guard = self.buffer.read();
        guard
            .iter()
            .rev()
            .filter(|r| {
                if let Some(min_lvl) = min_level {
                    if r.level < min_lvl {
                        return false;
                    }
                }
                if let Some(target) = target_filter {
                    if !r.target.contains(target) {
                        return false;
                    }
                }
                true
            })
            .take(limit)
            .cloned()
            .collect()
    }

    pub fn clear(&self) {
        self.buffer.write().clear();
    }

    pub fn len(&self) -> usize {
        self.buffer.read().len()
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.read().is_empty()
    }
}

/// Rotating file appender.
pub struct RotatingFileAppender {
    log_dir: PathBuf,
    file_path: PathBuf,
    max_size_bytes: u64,
    max_files: usize,
    file_writer: RwLock<Option<File>>,
}

impl RotatingFileAppender {
    pub fn new(max_size_bytes: u64, max_files: usize) -> Self {
        let log_dir = thor_data_dir().join("logs");
        let _ = fs::create_dir_all(&log_dir);
        let file_path = log_dir.join("thor.log");

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_path)
            .ok();

        Self {
            log_dir,
            file_path,
            max_size_bytes,
            max_files,
            file_writer: RwLock::new(file),
        }
    }

    pub fn append(&self, record: &LogRecord) {
        let line = record.format_console(false) + "\n";
        let bytes = line.as_bytes();

        let mut guard = self.file_writer.write();
        if let Some(file) = guard.as_mut() {
            if let Ok(metadata) = file.metadata() {
                if metadata.len() + bytes.len() as u64 > self.max_size_bytes {
                    // Perform file rotation
                    drop(guard);
                    self.rotate();
                    guard = self.file_writer.write();
                }
            }
            if let Some(f) = guard.as_mut() {
                let _ = f.write_all(bytes);
                let _ = f.flush();
            }
        }
    }

    fn rotate(&self) {
        // Shift existing rotated files: thor.log.4 -> thor.log.5, ... thor.log -> thor.log.1
        for i in (1..self.max_files).rev() {
            let src = self.log_dir.join(format!("thor.log.{}", i));
            let dst = self.log_dir.join(format!("thor.log.{}", i + 1));
            if src.exists() {
                let _ = fs::rename(&src, &dst);
            }
        }

        let first_backup = self.log_dir.join("thor.log.1");
        if self.file_path.exists() {
            let _ = fs::rename(&self.file_path, &first_backup);
        }

        // Open new file
        let new_file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&self.file_path)
            .ok();

        *self.file_writer.write() = new_file;
    }
}

/// Global Logger state.
pub struct Logger {
    buffer: Arc<LogBuffer>,
    file_appender: Option<RotatingFileAppender>,
    min_level: LogLevel,
    is_tty: bool,
}

use std::sync::OnceLock;

static GLOBAL_LOGGER: OnceLock<Arc<Logger>> = OnceLock::new();

impl Logger {
    pub fn global() -> Arc<Logger> {
        GLOBAL_LOGGER
            .get_or_init(|| {
                let env_level = std::env::var("THOR_LOG")
                    .or_else(|_| std::env::var("RUST_LOG"))
                    .ok()
                    .and_then(|s| s.parse::<LogLevel>().ok())
                    .unwrap_or(LogLevel::Info);

                let is_tty = libc_isatty();
                let file_appender = Some(RotatingFileAppender::new(10 * 1024 * 1024, 5));
                let buffer = Arc::new(LogBuffer::new(1000));

                Arc::new(Logger {
                    buffer,
                    file_appender,
                    min_level: env_level,
                    is_tty,
                })
            })
            .clone()
    }

    pub fn buffer(&self) -> Arc<LogBuffer> {
        self.buffer.clone()
    }

    pub fn log(&self, record: LogRecord) {
        if record.level < self.min_level {
            return;
        }

        // 1. In-Memory Ring Buffer
        self.buffer.push(record.clone());

        // 2. Console Output
        let formatted = record.format_console(self.is_tty);
        if record.level >= LogLevel::Error {
            eprintln!("{}", formatted);
        } else {
            println!("{}", formatted);
        }

        // 3. File Appender
        if let Some(appender) = &self.file_appender {
            appender.append(&record);
        }
    }
}

/// Initialize Thor professional logging system.
pub fn init_logging() -> Arc<Logger> {
    static PANIC_HOOK_SET: AtomicBool = AtomicBool::new(false);

    let logger = Logger::global();

    if !PANIC_HOOK_SET.swap(true, Ordering::SeqCst) {
        init_panic_hook();
    }

    logger
}

fn libc_isatty() -> bool {
    #[cfg(unix)]
    unsafe {
        libc::isatty(libc::STDOUT_FILENO) != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// Install custom panic hook to capture crash stack traces in Thor logs.
pub fn init_panic_hook() {
    let next_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "Box<Any> panic payload".to_string()
        };

        let location = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_else(|| "unknown".to_string());

        let record = LogRecord::new(
            LogLevel::Error,
            "thor::panic",
            format!("CRITICAL PANIC at {location}: {payload}"),
            info.location().map(|l| l.file()),
            info.location().map(|l| l.line()),
        );

        Logger::global().log(record);
        next_hook(info);
    }));
}

#[macro_export]
macro_rules! thor_info {
    ($target:expr, $($arg:tt)+) => {
        $crate::logging::Logger::global().log(
            $crate::logging::LogRecord::new(
                $crate::logging::LogLevel::Info,
                $target,
                format!($($arg)+),
                Some(file!()),
                Some(line!()),
            )
        )
    };
    ($($arg:tt)+) => {
        $crate::thor_info!("thor", $($arg)+)
    };
}

#[macro_export]
macro_rules! thor_warn {
    ($target:expr, $($arg:tt)+) => {
        $crate::logging::Logger::global().log(
            $crate::logging::LogRecord::new(
                $crate::logging::LogLevel::Warn,
                $target,
                format!($($arg)+),
                Some(file!()),
                Some(line!()),
            )
        )
    };
    ($($arg:tt)+) => {
        $crate::thor_warn!("thor", $($arg)+)
    };
}

#[macro_export]
macro_rules! thor_err {
    ($target:expr, $($arg:tt)+) => {
        $crate::logging::Logger::global().log(
            $crate::logging::LogRecord::new(
                $crate::logging::LogLevel::Error,
                $target,
                format!($($arg)+),
                Some(file!()),
                Some(line!()),
            )
        )
    };
    ($($arg:tt)+) => {
        $crate::thor_err!("thor", $($arg)+)
    };
}

#[macro_export]
macro_rules! thor_debug {
    ($target:expr, $($arg:tt)+) => {
        $crate::logging::Logger::global().log(
            $crate::logging::LogRecord::new(
                $crate::logging::LogLevel::Debug,
                $target,
                format!($($arg)+),
                Some(file!()),
                Some(line!()),
            )
        )
    };
    ($($arg:tt)+) => {
        $crate::thor_debug!("thor", $($arg)+)
    };
}

#[macro_export]
macro_rules! thor_trace {
    ($target:expr, $($arg:tt)+) => {
        $crate::logging::Logger::global().log(
            $crate::logging::LogRecord::new(
                $crate::logging::LogLevel::Trace,
                $target,
                format!($($arg)+),
                Some(file!()),
                Some(line!()),
            )
        )
    };
    ($($arg:tt)+) => {
        $crate::thor_trace!("thor", $($arg)+)
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_level_parsing_and_ordering() {
        assert_eq!("info".parse::<LogLevel>().unwrap(), LogLevel::Info);
        assert_eq!("DEBUG".parse::<LogLevel>().unwrap(), LogLevel::Debug);
        assert_eq!("error".parse::<LogLevel>().unwrap(), LogLevel::Error);
        assert!(LogLevel::Error > LogLevel::Warn);
        assert!(LogLevel::Warn > LogLevel::Info);
        assert!(LogLevel::Info > LogLevel::Debug);
    }

    #[test]
    fn test_ring_buffer_eviction() {
        let buffer = LogBuffer::new(3);
        buffer.push(LogRecord::new(LogLevel::Info, "test", "msg 1", None, None));
        buffer.push(LogRecord::new(LogLevel::Warn, "test", "msg 2", None, None));
        buffer.push(LogRecord::new(LogLevel::Error, "test", "msg 3", None, None));
        assert_eq!(buffer.len(), 3);

        buffer.push(LogRecord::new(LogLevel::Info, "test", "msg 4", None, None));
        assert_eq!(buffer.len(), 3);

        let records = buffer.query(None, None, 10);
        assert_eq!(records.len(), 3);
        assert_eq!(records[0].message, "msg 4");
        assert_eq!(records[2].message, "msg 2");
    }

    #[test]
    fn test_log_buffer_filtering() {
        let buffer = LogBuffer::new(10);
        buffer.push(LogRecord::new(LogLevel::Debug, "thor::cdp", "debug cdp", None, None));
        buffer.push(LogRecord::new(LogLevel::Info, "thor::shell", "info shell", None, None));
        buffer.push(LogRecord::new(LogLevel::Error, "thor::net", "error net", None, None));

        let errors = buffer.query(Some(LogLevel::Error), None, 10);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].message, "error net");

        let cdp_logs = buffer.query(None, Some("cdp"), 10);
        assert_eq!(cdp_logs.len(), 1);
        assert_eq!(cdp_logs[0].message, "debug cdp");
    }

    #[test]
    fn test_log_record_formatting() {
        let record = LogRecord::new(LogLevel::Info, "thor::shell", "Booting...", Some("main.rs"), Some(42));
        let formatted = record.format_console(false);
        assert!(formatted.contains("INFO"));
        assert!(formatted.contains("[thor::shell]"));
        assert!(formatted.contains("Booting..."));
        assert!(formatted.contains("[main.rs:42]"));
    }
}
