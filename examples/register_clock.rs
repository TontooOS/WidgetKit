//! Register a clock widget and keep pushing its content.
//!
//! Run against a live TontooOS session:
//!
//! ```sh
//! cargo run --example register_clock
//! ```

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use widgetkit::{Content, Registry, WidgetDescriptor, WidgetSize};

const APP: &str = "com.tontoo.clock";
const WIDGET: &str = "com.tontoo.clock.desktop";

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let descriptor = WidgetDescriptor::new(WIDGET, "Clock")
    .icon("clock")
    .sizes([WidgetSize::Small, WidgetSize::Medium, WidgetSize::Large])
    .default_size(WidgetSize::Medium)
    .refresh_interval(1)
    .summary("Current time and date");

  let registry = Registry::from_env();
  println!("socket: {}", registry.socket_path().display());

  let count = registry.register(APP, std::slice::from_ref(&descriptor))?;
  println!("registered {count} widget(s) for {APP}");

  // A real app would run its main loop here and push on every tick.
  for _ in 0..3 {
    registry.publish_content(WIDGET, &clock_tree())?;
    std::thread::sleep(Duration::from_secs(1));
  }

  let widgets = registry.list()?;
  println!("daemon now knows {} widget(s)", widgets.len());
  for entry in widgets {
    println!(" - {} [{}] {:?}", entry.id(), entry.app, entry.descriptor.sizes);
  }
  Ok(())
}

fn clock_tree() -> widgetkit::Node {
  let seconds = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map(|d| d.as_secs())
    .unwrap_or_default();
  let minutes_of_day = (seconds / 60) % (24 * 60);
  let text = format!("{:02}:{:02}", minutes_of_day / 60, minutes_of_day % 60);

  Content::column(2.0)
    .align(widgetkit::Align::Center)
    .child(Content::text(text).font_size(34.0))
    .child(Content::symbol("clock").symbol_size(16.0))
    .build()
}