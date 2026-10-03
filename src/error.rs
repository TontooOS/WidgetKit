use crate::lang::t;

/// Error type for every WidgetKit operation.
#[derive(Debug, Clone, PartialEq)]
pub enum WidgetError {
  /// The settings daemon socket path does not exist (daemon not running).
  SocketMissing(String),
  /// Transport failure while talking to the daemon.
  Connection(String),
  /// Malformed frame from the daemon.
  Protocol(String),
  /// The daemon answered with `ok: false`.
  Server(String),
  /// A well-formed reply with unusable content.
  Parse(String),
  /// A descriptor failed validation before it was sent.
  InvalidDescriptor(String),
}

impl WidgetError {
  /// Translated, human readable message for this error.
  pub fn message(&self) -> String {
    match self {
      Self::SocketMissing(path) => format!("{}: {}", t("socket_missing"), path),
      Self::Connection(detail) => format!("{}: {}", t("connection_failed"), detail),
      Self::Protocol(detail) => format!("{}: {}", t("protocol_error"), detail),
      Self::Server(detail) => format!("{}: {}", t("server_error"), detail),
      Self::Parse(detail) => format!("{}: {}", t("parse_error"), detail),
      Self::InvalidDescriptor(detail) => t("parse_error").to_string() + ": " + detail,
    }
  }
}

/// Convenience alias used by every WidgetKit function.
pub type Result<T> = std::result::Result<T, WidgetError>;

impl std::fmt::Display for WidgetError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str(&self.message())
  }
}

impl std::error::Error for WidgetError {}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn display_mentions_path() {
    let err = WidgetError::SocketMissing("/run/tontoo-settings.sock".to_string());
    assert!(err.to_string().contains("/run/tontoo-settings.sock"));
  }

  #[test]
  fn messages_cover_every_variant() {
    for key in [
      "socket_missing",
      "connection_failed",
      "protocol_error",
      "server_error",
      "parse_error",
      "no_descriptors",
      "duplicate_id",
      "empty_id",
      "invalid_size",
      "missing_size",
    ] {
      assert_ne!(t(key), key, "missing translation for {key}");
    }
  }
}