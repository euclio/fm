use gtk::glib;
use relm4::gtk;
use sourceview5::glib::{LogField, LogWriterOutput};
use log::{self, Record};

trait AsLog {
    fn into_log(self) -> log::Level;
}

impl AsLog for glib::LogLevel {
    fn into_log(self) -> log::Level {
        match self {
            glib::LogLevel::Info => log::Level::Info,
            glib::LogLevel::Error => log::Level::Error,
            glib::LogLevel::Debug => log::Level::Debug,
            glib::LogLevel::Warning => log::Level::Warn,
            glib::LogLevel::Message => log::Level::Info,
            glib::LogLevel::Critical => log::Level::Error,
        }
    }
}

pub fn tracing_writer_func(level: glib::LogLevel, fields: &[LogField<'_>]) -> LogWriterOutput {
    let mut message = "";
    let mut target = "";
    let mut file = None;
    let mut line = None;

    for field in fields {
        match field.key() {
            "MESSAGE" => message = field.value_str().unwrap_or(""),
            "GLIB_DOMAIN" => target = field.value_str().unwrap_or(""),
            "CODE_FILE" => file = field.value_str(),
            "CODE_LINE" => line = field.value_str().and_then(|line| line.parse().ok()),
            _ => (),
        }
    }

    log::logger().log(&Record::builder()
        .level(level.into_log())
        .args(format_args!("{}", message))
        .target(target)
        .file(file)
        .line(line)
        .build());

    LogWriterOutput::Handled
}
