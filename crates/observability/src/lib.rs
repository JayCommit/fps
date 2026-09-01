use std::str::FromStr;

use tracing_subscriber::fmt;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogFormat {
    Pretty,
    Json,
}

impl FromStr for LogFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pretty" => Ok(Self::Pretty),
            "json" => Ok(Self::Json),
            other => Err(format!("unknown log format '{other}'")),
        }
    }
}

/// Install a global tracing subscriber. Safe to call once per process.
pub fn init_tracing(format: LogFormat, default_filter: &str) {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter));

    match format {
        LogFormat::Pretty => {
            tracing_subscriber::registry()
                .with(filter)
                .with(fmt::layer().with_target(true).with_writer(std::io::stderr))
                .init();
        }
        LogFormat::Json => {
            tracing_subscriber::registry()
                .with(filter)
                .with(
                    fmt::layer()
                        .json()
                        .flatten_event(true)
                        .with_span_list(true)
                        .with_writer(std::io::stderr),
                )
                .init();
        }
    }
}

pub fn redact(value: &str) -> &'static str {
    if value.is_empty() {
        ""
    } else {
        "[redacted]"
    }
}

pub fn looks_secret(key: &str) -> bool {
    let k = key.to_ascii_lowercase();
    [
        "password",
        "secret",
        "token",
        "key",
        "authorization",
        "cookie",
    ]
    .iter()
    .any(|needle| k.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_keys_are_detected() {
        assert!(looks_secret("DATABASE_PASSWORD"));
        assert!(looks_secret("node_token"));
        assert!(!looks_secret("hostname"));
    }
}
