use std::fmt as std_fmt;
use tracing::Level;
use tracing_subscriber::fmt::FormatEvent;
use tracing_subscriber::fmt::{self, format::Writer};
use tracing_subscriber::{fmt::format::FormatFields, registry::LookupSpan};

pub struct ColorFormatter;

impl<S, N> FormatEvent<S, N> for ColorFormatter
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &fmt::FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &tracing::Event<'_>,
    ) -> std_fmt::Result {
        let metadata = event.metadata();
        let level = metadata.level();

        // Determine color based on level
        let color = match *level {
            Level::ERROR => "\x1b[91m", // Red
            Level::WARN => "\x1b[93m",  // Yellow
            Level::INFO => "",          // Nothing
            Level::DEBUG => "\x1b[94m", // Blue
            Level::TRACE => "\x1b[95m", // Magenta
        };

        ctx.field_format();

        // Reset color
        let reset = "\x1b[0m";

        let mut message = String::new();
        ctx.format_fields(Writer::new(&mut message), event)?;

        // Write the entire log line
        writeln!(writer, "{color}{message}{reset}")
    }
}
