use crate::proc::get_hartid;
use spin::Mutex;
use log::{self, Level, LevelFilter, Log, Metadata, Record};

pub fn init() {
    log::set_logger(&LOGGER).unwrap();
    log::set_max_level(LevelFilter::Trace);
}

struct SimplerLogger(Mutex::<()>);
static LOGGER: SimplerLogger = SimplerLogger(Mutex::new(()));

impl Log for SimplerLogger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let hold_lock = LOGGER.0.lock();
            print!("\x1b[{}m", level_to_color_code(record.level()));
            println!("[{}] [cpu{}]: {}", record.level(), get_hartid(), record.args());
            print!("\x1b[0m");
            drop(hold_lock);
        }
    }

    fn flush(&self) {}
}


fn level_to_color_code(level: Level) -> u8 {
    match level {
        Level::Error => 31, // Red
        Level::Warn => 93,  // BrightYellow
        Level::Info => 34,  // Blue
        Level::Debug => 32, // Green
        Level::Trace => 90, // BrightBlack
    }
}