use std::sync::Once;

use log::{Level, LevelFilter, Log, Metadata, Record};
use wasm_bindgen::JsValue;

static INSTALL: Once = Once::new();
static LOGGER: BrowserConsoleLogger = BrowserConsoleLogger;

struct BrowserConsoleLogger;

impl Log for BrowserConsoleLogger {
    fn enabled(&self, _metadata: &Metadata<'_>) -> bool {
        true
    }

    fn log(&self, record: &Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let message = format!("[{} {}] {}", record.level(), record.target(), record.args());
        match record.level() {
            Level::Error => error(&message),
            _ => info(&message),
        }
    }

    fn flush(&self) {}
}

pub(crate) fn install() {
    INSTALL.call_once(|| {
        if log::set_logger(&LOGGER).is_ok() {
            log::set_max_level(LevelFilter::Trace);
        }
        // Respect an application-installed tracing subscriber. Otherwise make
        // tracing events visible in the same browser console as `log` records.
        let _ = tracing_wasm::try_set_as_global_default();

        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |panic_info| {
            error(&format!("Fission application panic: {panic_info}"));
            previous(panic_info);
        }));
    });
}

pub(crate) fn info(message: &str) {
    web_sys::console::log_1(&JsValue::from_str(message));
}

pub(crate) fn error(message: &str) {
    web_sys::console::error_1(&JsValue::from_str(message));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_logger_accepts_every_log_level() {
        for level in [
            Level::Error,
            Level::Warn,
            Level::Info,
            Level::Debug,
            Level::Trace,
        ] {
            assert!(LOGGER.enabled(&Metadata::builder().level(level).build()));
        }
    }
}
