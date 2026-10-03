//! WidgetKit lets a TontooOS app put widgets on the desktop.
//!
//! An app describes a widget with a [`WidgetDescriptor`] and registers it
//! with the settings daemon on start. The desktop widget engine reads that
//! list and renders the widget content the app pushes as a [`Node`] tree.
//!
//! # Quick Start
//!
//! ```
//! use widgetkit::{Align, Content, WidgetDescriptor, WidgetSize};
//!
//! let descriptor = WidgetDescriptor::new("com.tontoo.clock.desktop", "Clock")
//!   .icon("clock")
//!   .sizes([WidgetSize::Small, WidgetSize::Medium])
//!   .refresh_interval(1);
//!
//! let tree = Content::column(4.0)
//!   .align(Align::Center)
//!   .child(Content::text("12:34").font_size(32.0))
//!   .child(Content::text("Tuesday").font_size(11.0))
//!   .build();
//!
//! assert!(descriptor.validate().is_ok());
//! assert_eq!(tree.node_count(), 3);
//! ```
//!
//! Sending both to the daemon needs a running settings daemon:
//!
//! ```no_run
//! use widgetkit::{Registry, WidgetDescriptor};
//!
//! let registry = Registry::from_env();
//! let descriptor = WidgetDescriptor::new("com.tontoo.clock.desktop", "Clock");
//! registry.register("com.tontoo.clock", &[descriptor])?;
//! # Ok::<(), widgetkit::WidgetError>(())
//! ```

pub mod descriptor;
pub mod error;
pub mod ffi;
pub mod lang;
pub mod node;
pub mod registry;
pub mod size;

pub use descriptor::{RegisteredWidget, WidgetDescriptor};
pub use error::{Result, WidgetError};
pub use node::{Align, Content, Node, Paint, TextAlign, TextStyle, Weight};
pub use registry::{Registry, Subscription};
pub use size::{GridSpan, WidgetSize};

/// WidgetKit version, mirrors `CARGO_PKG_VERSION`.
pub const WIDGETKIT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Bundle id of the built in clock widget the engine starts with.
pub const DEFAULT_WIDGET_ID: &str = "com.tontoo.widgetengine.clock";

/// Bundle id of the built in clock widget's owning app.
pub const DEFAULT_WIDGET_APP: &str = "com.tontoo.widgetengine";

/// Translate a key from the built in language files.
pub fn t(key: &str) -> String {
  lang::t(key)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn version_is_the_package_version() {
    assert_eq!(WIDGETKIT_VERSION, "27.0.0");
  }

  #[test]
  fn the_default_widget_is_a_clock_owned_by_the_engine() {
    assert!(DEFAULT_WIDGET_ID.starts_with(DEFAULT_WIDGET_APP));
    assert!(DEFAULT_WIDGET_ID.ends_with("clock"));
  }

  #[test]
  fn the_documented_quick_start_builds() {
    let descriptor = WidgetDescriptor::new("com.tontoo.clock.desktop", "Clock")
      .icon("clock")
      .sizes([WidgetSize::Small, WidgetSize::Medium])
      .refresh_interval(1);
    let tree = Content::column(4.0)
      .align(Align::Center)
      .child(Content::text("12:34").font_size(32.0))
      .child(Content::text("Tuesday").font_size(11.0))
      .build();

    assert!(descriptor.validate().is_ok());
    assert_eq!(tree.node_count(), 3);
  }

  #[test]
  fn translations_are_available() {
    assert_ne!(t("socket_missing"), "socket_missing");
  }
}