use crux_http::http::convert::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LogEntry {
    pub level: LogLevel,
    pub message: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum LogLevel {
    Info,
    Warning,
    Error,
}

#[derive(Default)]
pub struct Logger {
    current: Vec<LogEntry>,
}

impl Logger {
    pub fn info(&mut self, message: String) {
        self.current.push(LogEntry {
            level: LogLevel::Info,
            message,
        });
    }

    pub fn warning(&mut self, message: String) {
        self.current.push(LogEntry {
            level: LogLevel::Warning,
            message,
        });
    }

    pub fn error(&mut self, message: String) {
        self.current.push(LogEntry {
            level: LogLevel::Error,
            message,
        });
    }

    pub fn pop_all(&mut self) -> Vec<LogEntry> {
        let entries = self.current.clone();
        self.current.clear();
        entries
    }
}