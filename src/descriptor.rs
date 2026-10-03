//! Widget descriptors: what an app registers with the settings daemon.
//!
//! A descriptor is metadata only. The content tree is separate and is pushed
//! by the app whenever it has something new to show, so registering is cheap
//! and can happen on every app start.

use foundation::json::JsonValue;

use crate::error::{Result, WidgetError};
use crate::lang::t;
use crate::size::WidgetSize;

/// One widget an app offers on the desktop.
///
/// ```
/// use widgetkit::{WidgetDescriptor, WidgetSize};
///
/// let descriptor = WidgetDescriptor::new("com.tontoo.battery.level", "Battery")
///   .icon("battery.100")
///   .sizes([WidgetSize::Small, WidgetSize::Medium, WidgetSize::Large])
///   .refresh_interval(60);
/// assert!(descriptor.validate().is_ok());
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct WidgetDescriptor {
  /// Globally unique id, normally `<bundle id>.<name>`.
  pub id: String,
  /// Display name, already localized by the app.
  pub name: String,
  /// SF Symbol name or an absolute icon path.
  pub icon: String,
  /// Sizes this widget supports, in menu order.
  pub sizes: Vec<WidgetSize>,
  /// Size a freshly placed widget starts at. Must be in `sizes`.
  pub default_size: WidgetSize,
  /// Seconds between content pushes. `0` means the app only pushes on change.
  pub refresh_interval: u64,
  /// One line description shown in the edit panel.
  pub summary: String,
}

impl WidgetDescriptor {
  /// A descriptor with the small and medium sizes and no refresh timer.
  pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
    Self {
      id: id.into(),
      name: name.into(),
      icon: "square.grid.2x2".to_string(),
      sizes: vec![WidgetSize::Small, WidgetSize::Medium],
      default_size: WidgetSize::Medium,
      refresh_interval: 0,
      summary: String::new(),
    }
  }

  /// SF Symbol name or absolute icon path.
  pub fn icon(mut self, icon: impl Into<String>) -> Self {
    self.icon = icon.into();
    self
  }

  /// Replace the supported sizes.
  pub fn sizes<I: IntoIterator<Item = WidgetSize>>(mut self, sizes: I) -> Self {
    self.sizes = sizes.into_iter().collect();
    self
  }

  /// Add one supported size.
  pub fn add_size(mut self, size: WidgetSize) -> Self {
    if !self.sizes.contains(&size) {
      self.sizes.push(size);
    }
    self
  }

  /// Size a freshly placed widget starts at.
  pub fn default_size(mut self, size: WidgetSize) -> Self {
    self.default_size = size;
    self
  }

  /// Seconds between content pushes, `0` for change driven only.
  pub fn refresh_interval(mut self, seconds: u64) -> Self {
    self.refresh_interval = seconds;
    self
  }

  /// One line description for the edit panel.
  pub fn summary(mut self, summary: impl Into<String>) -> Self {
    self.summary = summary.into();
    self
  }

  /// Check the descriptor before it is sent.
  ///
  /// Returns [`WidgetError::InvalidDescriptor`] when the id is empty or the
  /// size list is empty.
  pub fn validate(&self) -> Result<()> {
    if self.id.trim().is_empty() {
      return Err(WidgetError::InvalidDescriptor(t("empty_id")));
    }
    if self.sizes.is_empty() {
      return Err(WidgetError::InvalidDescriptor(t("missing_size")));
    }
    Ok(())
  }

  /// Encode as JSON, without the owning app id.
  pub fn to_json(&self) -> JsonValue {
    JsonValue::Object(vec![
      ("id".to_string(), JsonValue::Str(self.id.clone())),
      ("name".to_string(), JsonValue::Str(self.name.clone())),
      ("icon".to_string(), JsonValue::Str(self.icon.clone())),
      (
        "sizes".to_string(),
        JsonValue::Array(
          self
            .sizes
            .iter()
            .map(|size| JsonValue::Str(size.as_str().to_string()))
            .collect(),
        ),
      ),
      (
        "default_size".to_string(),
        JsonValue::Str(self.default_size.as_str().to_string()),
      ),
      (
        "refresh_interval".to_string(),
        JsonValue::Integer(self.refresh_interval as i64),
      ),
      ("summary".to_string(), JsonValue::Str(self.summary.clone())),
    ])
  }

  /// Decode from JSON. Returns `None` when the id is missing or empty.
  pub fn from_json(value: &JsonValue) -> Option<Self> {
    let id = value.get("id").and_then(|v| v.as_str()).unwrap_or("").trim();
    if id.is_empty() {
      return None;
    }
    let sizes: Vec<WidgetSize> = value
      .get("sizes")
      .and_then(|v| v.as_array())
      .map(|items| {
        items
          .iter()
          .filter_map(|item| item.as_str())
          .filter_map(|raw| WidgetSize::from_str(raw).ok())
          .collect()
      })
      .unwrap_or_default();
    if sizes.is_empty() {
      return None;
    }
    let default_size = value
      .get("default_size")
      .and_then(|v| v.as_str())
      .and_then(|raw| WidgetSize::from_str(raw).ok())
      .filter(|size| sizes.contains(size))
      .unwrap_or(sizes[0]);

    Some(Self {
      id: id.to_string(),
      name: value
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or(id)
        .to_string(),
      icon: value
        .get("icon")
        .and_then(|v| v.as_str())
        .unwrap_or("square.grid.2x2")
        .to_string(),
      sizes,
      default_size,
      refresh_interval: value
        .get("refresh_interval")
        .and_then(|v| v.as_u64())
        .unwrap_or(0),
      summary: value
        .get("summary")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string(),
    })
  }
}

/// A descriptor as the daemon reports it back, together with its owning app.
#[derive(Debug, Clone, PartialEq)]
pub struct RegisteredWidget {
  /// Bundle id of the app that registered the widget.
  pub app: String,
  /// The descriptor itself.
  pub descriptor: WidgetDescriptor,
  /// Unix seconds when the app last registered.
  pub registered_at: i64,
}

impl RegisteredWidget {
  /// Convenience accessor for the widget id.
  pub fn id(&self) -> &str {
    &self.descriptor.id
  }

  /// Convenience accessor for the display name.
  pub fn name(&self) -> &str {
    &self.descriptor.name
  }

  /// Encode as JSON, including the owning app id.
  pub fn to_json(&self) -> JsonValue {
    let mut value = self.descriptor.to_json();
    if let JsonValue::Object(entries) = &mut value {
      entries.insert(0, ("app".to_string(), JsonValue::Str(self.app.clone())));
      entries.push((
        "registered_at".to_string(),
        JsonValue::Integer(self.registered_at),
      ));
    }
    value
  }

  /// Decode from JSON, dropping entries without an app id.
  pub fn from_json(value: &JsonValue) -> Option<Self> {
    let app = value.get("app").and_then(|v| v.as_str()).unwrap_or("");
    if app.is_empty() {
      return None;
    }
    Some(Self {
      app: app.to_string(),
      descriptor: WidgetDescriptor::from_json(value)?,
      registered_at: value.get("registered_at").and_then(|v| v.as_i64()).unwrap_or(0),
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn sample() -> WidgetDescriptor {
    WidgetDescriptor::new("com.tontoo.battery.level", "Battery")
      .icon("battery.100")
      .sizes(WidgetSize::ALL)
      .default_size(WidgetSize::Medium)
      .refresh_interval(60)
      .summary("Charge level of the laptop")
  }

  #[test]
  fn descriptor_round_trips() {
    let descriptor = sample();
    let decoded = WidgetDescriptor::from_json(&descriptor.to_json()).unwrap();
    assert_eq!(decoded, descriptor);
  }

  #[test]
  fn a_new_descriptor_validates() {
    assert!(WidgetDescriptor::new("a.b", "A").validate().is_ok());
  }

  #[test]
  fn an_empty_id_is_rejected() {
    let err = WidgetDescriptor::new("   ", "A").validate().unwrap_err();
    assert!(matches!(err, WidgetError::InvalidDescriptor(_)));
  }

  #[test]
  fn an_empty_size_list_is_rejected() {
    let err = WidgetDescriptor::new("a.b", "A")
      .sizes(Vec::new())
      .validate()
      .unwrap_err();
    // Assert the variant, not the text: the message is localized.
    assert!(matches!(err, WidgetError::InvalidDescriptor(_)));
  }

  #[test]
  fn add_size_does_not_duplicate() {
    // `new` already starts with small and medium.
    let descriptor = WidgetDescriptor::new("a.b", "A").add_size(WidgetSize::Small);
    assert_eq!(descriptor.sizes, vec![WidgetSize::Small, WidgetSize::Medium]);

    let descriptor = descriptor.add_size(WidgetSize::Large);
    assert_eq!(
      descriptor.sizes,
      vec![WidgetSize::Small, WidgetSize::Medium, WidgetSize::Large]
    );
  }

  #[test]
  fn a_descriptor_without_an_id_does_not_decode() {
    let raw = JsonValue::Object(vec![("name".to_string(), JsonValue::Str("x".to_string()))]);
    assert!(WidgetDescriptor::from_json(&raw).is_none());
  }

  #[test]
  fn a_descriptor_with_an_unknown_size_does_not_decode() {
    let raw = JsonValue::Object(vec![
      ("id".to_string(), JsonValue::Str("a.b".to_string())),
      (
        "sizes".to_string(),
        JsonValue::Array(vec![JsonValue::Str("huge".to_string())]),
      ),
    ]);
    assert!(WidgetDescriptor::from_json(&raw).is_none());
  }

  #[test]
  fn a_default_size_outside_the_list_falls_back_to_the_first() {
    let raw = JsonValue::Object(vec![
      ("id".to_string(), JsonValue::Str("a.b".to_string())),
      (
        "sizes".to_string(),
        JsonValue::Array(vec![JsonValue::Str("small".to_string())]),
      ),
      (
        "default_size".to_string(),
        JsonValue::Str("large".to_string()),
      ),
    ]);
    let decoded = WidgetDescriptor::from_json(&raw).unwrap();
    assert_eq!(decoded.default_size, WidgetSize::Small);
  }

  #[test]
  fn registered_widget_round_trips_with_its_app() {
    let entry = RegisteredWidget {
      app: "com.tontodo.batteries".to_string(),
      descriptor: sample(),
      registered_at: 1_760_000_000,
    };
    let decoded = RegisteredWidget::from_json(&entry.to_json()).unwrap();
    assert_eq!(decoded, entry);
    assert_eq!(decoded.id(), "com.tontoo.battery.level");
    assert_eq!(decoded.name(), "Battery");
  }

  #[test]
  fn a_registered_widget_without_an_app_does_not_decode() {
    let mut value = sample().to_json();
    if let JsonValue::Object(entries) = &mut value {
      entries.retain(|(key, _)| key != "app");
    }
    assert!(RegisteredWidget::from_json(&value).is_none());
  }
}