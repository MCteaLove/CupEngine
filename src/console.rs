//! Консоль логов: перехватывает записи `log` и хранит их для окна Console в редакторе.

use std::sync::Mutex;

use log::{Level, LevelFilter, Log, Metadata, Record};

#[derive(Clone)]
pub struct LogEntry {
    pub level: Level,
    pub target: String,
    pub message: String,
}

static LOGS: Mutex<Vec<LogEntry>> = Mutex::new(Vec::new());
const MAX_LINES: usize = 2000;

struct ConsoleLogger;
static LOGGER: ConsoleLogger = ConsoleLogger;

impl Log for ConsoleLogger {
    fn enabled(&self, m: &Metadata) -> bool {
        // свои сообщения — от Info, чужие крейты (wgpu и т.д.) — только предупреждения и ошибки
        m.level() <= Level::Warn || m.target().starts_with("engine")
    }

    fn log(&self, r: &Record) {
        if !self.enabled(r.metadata()) {
            return;
        }
        eprintln!("[{}] {}: {}", r.level(), r.target(), r.args());
        if let Ok(mut v) = LOGS.lock() {
            v.push(LogEntry {
                level: r.level(),
                target: r.target().to_string(),
                message: r.args().to_string(),
            });
            if v.len() > MAX_LINES {
                let extra = v.len() - MAX_LINES;
                v.drain(0..extra);
            }
        }
    }

    fn flush(&self) {}
}

pub fn init() {
    let _ = log::set_logger(&LOGGER);
    log::set_max_level(LevelFilter::Info);
}

pub fn snapshot() -> Vec<LogEntry> {
    LOGS.lock().map(|v| v.clone()).unwrap_or_default()
}

pub fn clear() {
    if let Ok(mut v) = LOGS.lock() {
        v.clear();
    }
}
