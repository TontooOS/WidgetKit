//! Client for the settings daemon widget registry.
//!
//! An app registers its [`WidgetDescriptor`]s once per start, then pushes
//! content trees with [`Registry::publish_content`] whenever it has something
//! new. The widget engine reads the list with [`Registry::list`] and stays up
//! to date through [`Registry::subscribe`].
//!
//! Wire format is newline-delimited JSON over the settings daemon unix
//! socket: `{"id": 1, "op": "...", "params": {...}}` answered with
//! `{"id": 1, "ok": true, "result": {...}}` or
//! `{"id": 1, "ok": false, "error": "..."}`.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use foundation::json::JsonValue;

use crate::descriptor::{RegisteredWidget, WidgetDescriptor};
use crate::error::{Result, WidgetError};
use crate::lang::t;
use crate::node::Node;

/// Settings daemon socket, mirrors the daemon default.
pub const DEFAULT_SOCKET_PATH: &str = "/run/tontoo-settings.sock";

/// Op that registers or replaces the widgets of one app.
pub const OP_REGISTER: &str = "widget_register";
/// Op that lists every registered widget.
pub const OP_LIST: &str = "widget_list";
/// Op that drops every widget of one app.
pub const OP_UNREGISTER: &str = "widget_unregister";
/// Op that pushes one widget content tree.
pub const OP_CONTENT: &str = "widget_content";
/// Op that returns the current content tree of every widget.
pub const OP_CONTENT_LIST: &str = "widget_content_list";

/// Event pushed when a widget content tree changed.
pub const EVENT_CONTENT_CHANGED: &str = "widget_content_changed";
/// Event pushed when the set of registered widgets changed.
pub const EVENT_WIDGETS_CHANGED: &str = "widgets_changed";

/// Socket read timeout per request.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Client handle for the widget registry in the settings daemon.
#[derive(Debug, Clone)]
pub struct Registry {
  socket_path: PathBuf,
}

impl Default for Registry {
  fn default() -> Self {
    Self::new()
  }
}

impl Registry {
  /// Client for the default socket path.
  pub fn new() -> Self {
    Self {
      socket_path: PathBuf::from(DEFAULT_SOCKET_PATH),
    }
  }

  /// Client for an explicit socket path, used by tests.
  pub fn with_socket(path: impl Into<PathBuf>) -> Self {
    Self {
      socket_path: path.into(),
    }
  }

  /// Build from `SETTINGS_SOCKET`, falling back to the default path.
  pub fn from_env() -> Self {
    let socket_path = std::env::var("SETTINGS_SOCKET")
      .map(PathBuf::from)
      .unwrap_or_else(|_| PathBuf::from(DEFAULT_SOCKET_PATH));
    Self { socket_path }
  }

  pub fn socket_path(&self) -> &Path {
    &self.socket_path
  }

  /// Register or replace the widgets of `app`.
  ///
  /// Every descriptor is validated first; nothing is sent when one is
  /// invalid. Returns how many widgets the daemon now holds for `app`.
  pub fn register(&self, app: &str, widgets: &[WidgetDescriptor]) -> Result<usize> {
    if widgets.is_empty() {
      return Err(WidgetError::InvalidDescriptor(t("no_descriptors")));
    }
    for descriptor in widgets {
      descriptor.validate()?;
    }
    let params = JsonValue::Object(vec![
      ("app".to_string(), JsonValue::Str(app.to_string())),
      (
        "widgets".to_string(),
        JsonValue::Array(widgets.iter().map(WidgetDescriptor::to_json).collect()),
      ),
    ]);
    let result = self.request(OP_REGISTER, params)?;
    Ok(result
      .get("registered")
      .and_then(|v| v.as_i64())
      .unwrap_or(widgets.len() as i64) as usize)
  }

  /// List every registered widget, sorted by app then id as the daemon
  /// returns them.
  ///
  /// Returns an empty list when the daemon holds none.
  pub fn list(&self) -> Result<Vec<RegisteredWidget>> {
    let result = self.request(OP_LIST, JsonValue::Object(Vec::new()))?;
    let widgets = match result.get("widgets") {
      Some(JsonValue::Array(items)) => items
        .iter()
        .filter_map(RegisteredWidget::from_json)
        .collect(),
      _ => Vec::new(),
    };
    Ok(widgets)
  }

  /// Drop every widget registered by `app`. Returns how many were removed.
  pub fn unregister(&self, app: &str) -> Result<usize> {
    let params = JsonValue::Object(vec![("app".to_string(), JsonValue::Str(app.to_string()))]);
    let result = self.request(OP_UNREGISTER, params)?;
    Ok(result
      .get("removed")
      .and_then(|v| v.as_i64())
      .unwrap_or(0) as usize)
  }

  /// Push the content tree of one widget.
  ///
  /// Returns `Err` when the daemon does not know `widget_id`, which happens
  /// when [`Registry::register`] has not run yet.
  pub fn publish_content(&self, widget_id: &str, content: &Node) -> Result<bool> {
    let params = JsonValue::Object(vec![
      ("widget".to_string(), JsonValue::Str(widget_id.to_string())),
      ("content".to_string(), content.to_json()),
    ]);
    let result = self.request(OP_CONTENT, params)?;
    Ok(result.get("accepted").and_then(|v| v.as_bool()).unwrap_or(false))
  }

  /// Read the current content tree of every widget, so a freshly started
  /// engine can paint without waiting for the next app tick.
  pub fn content_list(&self) -> Result<Vec<(String, Node)>> {
    let result = self.request(OP_CONTENT_LIST, JsonValue::Object(Vec::new()))?;
    let entries = match result.get("content") {
      Some(JsonValue::Array(items)) => items,
      _ => return Ok(Vec::new()),
    };
    Ok(
      entries
        .iter()
        .filter_map(|entry| {
          let id = entry.get("widget")?.as_str()?.to_string();
          let content = entry.get("content")?;
          Some((id, Node::from_json(content)))
        })
        .collect(),
    )
  }

  /// Send one request frame and return the `result` of a success reply.
  pub fn request(&self, op: &str, params: JsonValue) -> Result<JsonValue> {
    if !self.socket_path.exists() {
      return Err(WidgetError::SocketMissing(
        self.socket_path.to_string_lossy().to_string(),
      ));
    }
    let mut stream = UnixStream::connect(&self.socket_path).map_err(|err| {
      WidgetError::Connection(format!("{}: {err}", self.socket_path.display()))
    })?;
    let _ = stream.set_read_timeout(Some(REQUEST_TIMEOUT));
    let _ = stream.set_write_timeout(Some(REQUEST_TIMEOUT));

    let mut line = JsonValue::Object(vec![
      ("id".to_string(), JsonValue::Integer(1)),
      ("op".to_string(), JsonValue::Str(op.to_string())),
      ("params".to_string(), params),
    ])
    .stringify(false);
    line.push('\n');

    stream
      .write_all(line.as_bytes())
      .map_err(|err| WidgetError::Connection(err.to_string()))?;
    stream
      .flush()
      .map_err(|err| WidgetError::Connection(err.to_string()))?;

    let mut reply = String::new();
    BufReader::new(&stream)
      .read_line(&mut reply)
      .map_err(|err| WidgetError::Connection(err.to_string()))?;

    parse_reply(&reply)
  }

  /// Open a long lived subscription for change events.
  ///
  /// Pass an empty slice to receive every event. The caller owns the stream
  /// and should call [`Subscription::next_event`] on its own loop.
  pub fn subscribe(&self, events: &[&str]) -> Result<Subscription> {
    if !self.socket_path.exists() {
      return Err(WidgetError::SocketMissing(
        self.socket_path.to_string_lossy().to_string(),
      ));
    }
    let mut stream = UnixStream::connect(&self.socket_path).map_err(|err| {
      WidgetError::Connection(format!("{}: {err}", self.socket_path.display()))
    })?;
    let _ = stream.set_read_timeout(Some(REQUEST_TIMEOUT));

    let list = JsonValue::Array(
      events
        .iter()
        .map(|name| JsonValue::Str((*name).to_string()))
        .collect(),
    );
    let mut line = JsonValue::Object(vec![
      ("id".to_string(), JsonValue::Integer(1)),
      ("op".to_string(), JsonValue::Str("subscribe".to_string())),
      (
        "params".to_string(),
        JsonValue::Object(vec![("events".to_string(), list)]),
      ),
    ])
    .stringify(false);
    line.push('\n');

    stream
      .write_all(line.as_bytes())
      .map_err(|err| WidgetError::Connection(err.to_string()))?;
    stream
      .flush()
      .map_err(|err| WidgetError::Connection(err.to_string()))?;

    let mut reply = String::new();
    BufReader::new(&stream)
      .read_line(&mut reply)
      .map_err(|err| WidgetError::Connection(err.to_string()))?;
    parse_reply(&reply)?;

    Ok(Subscription { stream })
  }
}

/// Parse one daemon reply frame into its `result` payload.
fn parse_reply(reply: &str) -> Result<JsonValue> {
  let trimmed = reply.trim();
  if trimmed.is_empty() {
    return Err(WidgetError::Protocol("empty reply".to_string()));
  }
  let value = JsonValue::parse(trimmed)
    .map_err(|err| WidgetError::Protocol(format!("reply is not valid json: {err}")))?;
  match value.get("ok").and_then(|v| v.as_bool()) {
    Some(true) => Ok(value.get("result").cloned().unwrap_or(JsonValue::Null)),
    Some(false) => Err(WidgetError::Server(
      value
        .get("error")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown daemon error")
        .to_string(),
    )),
    None => Err(WidgetError::Protocol("reply has no ok field".to_string())),
  }
}

/// A live event subscription held open against the daemon.
#[derive(Debug)]
pub struct Subscription {
  stream: UnixStream,
}

impl Subscription {
  /// Block until the next event arrives and return it.
  ///
  /// Returns [`WidgetError::Connection`] when the daemon closed the socket.
  pub fn next_event(&mut self) -> Result<(String, JsonValue)> {
    let mut line = String::new();
    let read = BufReader::new(&self.stream)
      .read_line(&mut line)
      .map_err(|err| WidgetError::Connection(err.to_string()))?;
    if read == 0 {
      return Err(WidgetError::Connection(
        "settings daemon closed the subscription".to_string(),
      ));
    }
    let value = JsonValue::parse(line.trim())
      .map_err(|err| WidgetError::Protocol(format!("event is not valid json: {err}")))?;
    let name = value
      .get("event")
      .and_then(|v| v.as_str())
      .ok_or_else(|| WidgetError::Protocol("event has no name".to_string()))?
      .to_string();
    let payload = value.get("result").cloned().unwrap_or(JsonValue::Null);
    Ok((name, payload))
  }
}

/// Register widgets at the default socket.
pub fn register(app: &str, widgets: &[WidgetDescriptor]) -> Result<usize> {
  Registry::new().register(app, widgets)
}

/// List every registered widget at the default socket.
pub fn list() -> Result<Vec<RegisteredWidget>> {
  Registry::new().list()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_success_reply_yields_the_result() {
    let reply = r#"{"id":1,"ok":true,"result":{"registered":2}}"#;
    let result = parse_reply(reply).unwrap();
    assert_eq!(result.get("registered").and_then(|v| v.as_i64()), Some(2));
  }

  #[test]
  fn a_failure_reply_carries_the_daemon_message() {
    let reply = r#"{"id":1,"ok":false,"error":"widget unknown"}"#;
    let err = parse_reply(reply).unwrap_err();
    assert!(matches!(err, WidgetError::Server(_)));
    assert!(err.to_string().contains("widget unknown"));
  }

  #[test]
  fn malformed_replies_are_protocol_errors() {
    assert!(matches!(
      parse_reply("").unwrap_err(),
      WidgetError::Protocol(_)
    ));
    assert!(matches!(
      parse_reply("not json").unwrap_err(),
      WidgetError::Protocol(_)
    ));
    assert!(matches!(
      parse_reply(r#"{"id":1}"#).unwrap_err(),
      WidgetError::Protocol(_)
    ));
  }

  #[test]
  fn a_missing_socket_surfaces_before_connecting() {
    let registry = Registry::with_socket("/nonexistent/tontoo-settings.sock");
    let err = registry.list().unwrap_err();
    assert!(matches!(err, WidgetError::SocketMissing(_)));
    assert!(err.to_string().contains("/nonexistent/tontoo-settings.sock"));
  }

  #[test]
  fn registering_nothing_is_rejected_locally() {
    let registry = Registry::with_socket("/nonexistent/tontoo-settings.sock");
    let err = registry.register("com.tontoo.app", &[]).unwrap_err();
    assert!(matches!(err, WidgetError::InvalidDescriptor(_)));
  }

  #[test]
  fn an_invalid_descriptor_is_rejected_before_the_socket() {
    let registry = Registry::with_socket("/nonexistent/tontoo-settings.sock");
    let bad = WidgetDescriptor::new("", "Broken");
    let err = registry.register("com.tontoo.app", &[bad]).unwrap_err();
    assert!(matches!(err, WidgetError::InvalidDescriptor(_)));
  }

  #[test]
  fn a_valid_descriptor_passes_validation_before_the_socket() {
    let registry = Registry::with_socket("/nonexistent/tontoo-settings.sock");
    let good = WidgetDescriptor::new("com.tontoo.a.b", "A");
    // Validation passes, so the failure comes from the missing socket.
    let err = registry.register("com.tontoo.app", &[good]).unwrap_err();
    assert!(matches!(err, WidgetError::SocketMissing(_)));
  }

  #[test]
  fn op_names_match_the_daemon_contract() {
    assert_eq!(OP_REGISTER, "widget_register");
    assert_eq!(OP_LIST, "widget_list");
    assert_eq!(OP_UNREGISTER, "widget_unregister");
    assert_eq!(OP_CONTENT, "widget_content");
    assert_eq!(OP_CONTENT_LIST, "widget_content_list");
  }

  #[test]
  fn default_socket_path_is_the_daemon_default() {
    assert_eq!(DEFAULT_SOCKET_PATH, "/run/tontoo-settings.sock");
    assert_eq!(Registry::new().socket_path(), Path::new(DEFAULT_SOCKET_PATH));
  }
}