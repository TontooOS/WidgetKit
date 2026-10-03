//! The declarative widget content tree.
//!
//! An app does not draw a widget, it builds a [`Node`] tree and the widget
//! engine turns it into TontooUI elements. [`Content`] is the builder an app
//! uses; [`Node`] is the serializable wire type that travels over the socket.
//!
//! Node names on the wire are short (`"row"`, `"text"`) to keep IPC frames
//! small. Every decode is total: an unknown name or a wrong field type falls
//! back to a safe default instead of failing, so one bad app can never take
//! the engine down.

use foundation::json::JsonValue;

use crate::error::{Result, WidgetError};

/// Cross-axis alignment inside a stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Align {
  #[default]
  Leading,
  Center,
  Trailing,
}

impl Align {
  pub fn as_str(self) -> &'static str {
    match self {
      Self::Leading => "leading",
      Self::Center => "center",
      Self::Trailing => "trailing",
    }
  }

  pub fn from_str(raw: &str) -> Self {
    match raw {
      "center" => Self::Center,
      "trailing" => Self::Trailing,
      _ => Self::Leading,
    }
  }
}

/// Text weight, mapped to the matching TontooUI / SF Pro weight.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Weight {
  #[default]
  Regular,
  Medium,
  Semibold,
  Bold,
}

impl Weight {
  pub fn as_str(self) -> &'static str {
    match self {
      Self::Regular => "regular",
      Self::Medium => "medium",
      Self::Semibold => "semibold",
      Self::Bold => "bold",
    }
  }

  pub fn from_str(raw: &str) -> Self {
    match raw {
      "medium" => Self::Medium,
      "semibold" => Self::Semibold,
      "bold" => Self::Bold,
      _ => Self::Regular,
    }
  }
}

/// Color request from an app.
///
/// The system chrome of a widget (frame, badge, menu) only ever uses
/// `Background` and `Text`, per the TontooOS color rule. `Accent` follows the
/// system accent so it stays consistent across apps. `Rgba` is an escape hatch
/// for content that genuinely needs its own color, such as a battery level.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Paint {
  Background,
  Text,
  Accent,
  Rgba(u8, u8, u8, u8),
}

impl Default for Paint {
  fn default() -> Self {
    Self::Text
  }
}

impl Paint {
  pub fn to_json(self) -> JsonValue {
    match self {
      Self::Background => JsonValue::Str("bg".to_string()),
      Self::Text => JsonValue::Str("text".to_string()),
      Self::Accent => JsonValue::Str("accent".to_string()),
      Self::Rgba(r, g, b, a) => JsonValue::Str(format!("#{r:02x}{g:02x}{b:02x}{a:02x}")),
    }
  }

  pub fn from_json(value: Option<&JsonValue>) -> Self {
    let Some(raw) = value.and_then(|v| v.as_str()) else {
      return Self::Text;
    };
    match raw {
      "bg" => Self::Background,
      "text" => Self::Text,
      "accent" => Self::Accent,
      other => {
        let hex = other.strip_prefix('#').unwrap_or(other);
        if hex.len() == 8 {
          let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).unwrap_or(0);
          Self::Rgba(byte(0), byte(2), byte(4), byte(6))
        } else {
          Self::Text
        }
      }
    }
  }
}

/// Horizontal text alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextAlign {
  #[default]
  Leading,
  Center,
  Trailing,
}

impl TextAlign {
  pub fn as_str(self) -> &'static str {
    match self {
      Self::Leading => "leading",
      Self::Center => "center",
      Self::Trailing => "trailing",
    }
  }

  pub fn from_str(raw: &str) -> Self {
    match raw {
      "center" => Self::Center,
      "trailing" => Self::Trailing,
      _ => Self::Leading,
    }
  }
}

/// Style of a [`Node::Text`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextStyle {
  pub size: f32,
  pub weight: Weight,
  pub color: Paint,
  pub align: TextAlign,
}

impl Default for TextStyle {
  fn default() -> Self {
    Self {
      size: 13.0,
      weight: Weight::Regular,
      color: Paint::Text,
      align: TextAlign::Leading,
    }
  }
}

impl TextStyle {
  fn to_json(self) -> JsonValue {
    JsonValue::Object(vec![
      ("size".to_string(), JsonValue::Float(self.size as f64)),
      ("weight".to_string(), JsonValue::Str(self.weight.as_str().to_string())),
      ("color".to_string(), self.color.to_json()),
      ("align".to_string(), JsonValue::Str(self.align.as_str().to_string())),
    ])
  }

  fn from_json(value: Option<&JsonValue>) -> Self {
    let mut style = Self::default();
    let Some(entry) = value else { return style };
    if let Some(size) = entry.get("size").and_then(|v| v.as_f64()) {
      style.size = clamp_size(size as f32);
    }
    if let Some(weight) = entry.get("weight").and_then(|v| v.as_str()) {
      style.weight = Weight::from_str(weight);
    }
    style.color = Paint::from_json(entry.get("color"));
    if let Some(align) = entry.get("align").and_then(|v| v.as_str()) {
      style.align = TextAlign::from_str(align);
    }
    style
  }
}

/// Smallest and largest font a widget may ask for, in logical px.
pub const MIN_FONT_SIZE: f32 = 8.0;
/// See [`MIN_FONT_SIZE`].
pub const MAX_FONT_SIZE: f32 = 96.0;

fn clamp_size(size: f32) -> f32 {
  if size.is_nan() {
    return TextStyle::default().size;
  }
  size.clamp(MIN_FONT_SIZE, MAX_FONT_SIZE)
}

fn clamp_spacing(spacing: f32) -> f32 {
  if spacing.is_nan() {
    0.0
  } else {
    spacing.clamp(0.0, 64.0)
  }
}

fn clamp_len(value: f32, max: f32) -> f32 {
  if value.is_nan() {
    0.0
  } else {
    value.clamp(0.0, max)
  }
}

/// One node of the widget content tree.
#[derive(Debug, Clone, PartialEq)]
pub enum Node {
  /// Horizontal stack.
  Row {
    spacing: f32,
    align: Align,
    children: Vec<Node>,
  },
  /// Vertical stack.
  Column {
    spacing: f32,
    align: Align,
    children: Vec<Node>,
  },
  /// Overlay stack, every child shares the same rect.
  Stack {
    align: Align,
    children: Vec<Node>,
  },
  /// Flexible empty space.
  Spacer {
    min: f32,
    factor: f32,
  },
  /// Inset around a single child.
  Padding {
    all: f32,
    child: Box<Node>,
  },
  /// A text run.
  Text {
    text: String,
    style: TextStyle,
  },
  /// An SF Symbol by name.
  Symbol {
    name: String,
    size: f32,
    tint: Paint,
  },
  /// An image file, decoded by the engine.
  Image {
    path: String,
    width: f32,
    height: f32,
  },
  /// Horizontal progress bar, `value` of `total`.
  Progress {
    value: f32,
    total: f32,
    thickness: f32,
    tint: Paint,
  },
  /// Circular progress ring, `value` of `total`.
  Gauge {
    value: f32,
    total: f32,
    diameter: f32,
    tint: Paint,
  },
  /// Filled circle, used for accessory style indicators.
  Circle {
    diameter: f32,
    fill: Paint,
  },
  /// Rounded bar, used as a separator or a track.
  Bar {
    height: f32,
    radius: f32,
    fill: Paint,
  },
}

impl Node {
  /// Direct children of this node, empty for leaves.
  ///
  /// [`Node::Padding`] reports its single wrapped child, so tree walks such as
  /// [`Node::node_count`] descend through insets.
  pub fn children(&self) -> &[Node] {
    match self {
      Self::Row { children, .. }
      | Self::Column { children, .. }
      | Self::Stack { children, .. } => children,
      Self::Padding { child, .. } => std::slice::from_ref(&**child),
      _ => &[],
    }
  }

  /// Total number of nodes in this subtree, including this one.
  pub fn node_count(&self) -> usize {
    1 + self.children().iter().map(Node::node_count).sum::<usize>()
  }

  /// Encode as JSON.
  pub fn to_json(&self) -> JsonValue {
    match self {
      Self::Row {
        spacing,
        align,
        children,
      } => JsonValue::Object(vec![
        ("n".to_string(), JsonValue::Str("row".to_string())),
        ("spacing".to_string(), JsonValue::Float(*spacing as f64)),
        ("align".to_string(), JsonValue::Str(align.as_str().to_string())),
        ("children".to_string(), nodes_to_json(children)),
      ]),
      Self::Column {
        spacing,
        align,
        children,
      } => JsonValue::Object(vec![
        ("n".to_string(), JsonValue::Str("column".to_string())),
        ("spacing".to_string(), JsonValue::Float(*spacing as f64)),
        ("align".to_string(), JsonValue::Str(align.as_str().to_string())),
        ("children".to_string(), nodes_to_json(children)),
      ]),
      Self::Stack { align, children } => JsonValue::Object(vec![
        ("n".to_string(), JsonValue::Str("stack".to_string())),
        ("align".to_string(), JsonValue::Str(align.as_str().to_string())),
        ("children".to_string(), nodes_to_json(children)),
      ]),
      Self::Spacer { min, factor } => JsonValue::Object(vec![
        ("n".to_string(), JsonValue::Str("spacer".to_string())),
        ("min".to_string(), JsonValue::Float(*min as f64)),
        ("factor".to_string(), JsonValue::Float(*factor as f64)),
      ]),
      Self::Padding { all, child } => JsonValue::Object(vec![
        ("n".to_string(), JsonValue::Str("padding".to_string())),
        ("all".to_string(), JsonValue::Float(*all as f64)),
        ("child".to_string(), child.to_json()),
      ]),
      Self::Text { text, style } => JsonValue::Object(vec![
        ("n".to_string(), JsonValue::Str("text".to_string())),
        ("text".to_string(), JsonValue::Str(text.clone())),
        ("style".to_string(), style.to_json()),
      ]),
      Self::Symbol { name, size, tint } => JsonValue::Object(vec![
        ("n".to_string(), JsonValue::Str("symbol".to_string())),
        ("name".to_string(), JsonValue::Str(name.clone())),
        ("size".to_string(), JsonValue::Float(*size as f64)),
        ("tint".to_string(), tint.to_json()),
      ]),
      Self::Image {
        path,
        width,
        height,
      } => JsonValue::Object(vec![
        ("n".to_string(), JsonValue::Str("image".to_string())),
        ("path".to_string(), JsonValue::Str(path.clone())),
        ("w".to_string(), JsonValue::Float(*width as f64)),
        ("h".to_string(), JsonValue::Float(*height as f64)),
      ]),
      Self::Progress {
        value,
        total,
        thickness,
        tint,
      } => JsonValue::Object(vec![
        ("n".to_string(), JsonValue::Str("progress".to_string())),
        ("value".to_string(), JsonValue::Float(*value as f64)),
        ("total".to_string(), JsonValue::Float(*total as f64)),
        ("thickness".to_string(), JsonValue::Float(*thickness as f64)),
        ("tint".to_string(), tint.to_json()),
      ]),
      Self::Gauge {
        value,
        total,
        diameter,
        tint,
      } => JsonValue::Object(vec![
        ("n".to_string(), JsonValue::Str("gauge".to_string())),
        ("value".to_string(), JsonValue::Float(*value as f64)),
        ("total".to_string(), JsonValue::Float(*total as f64)),
        ("diameter".to_string(), JsonValue::Float(*diameter as f64)),
        ("tint".to_string(), tint.to_json()),
      ]),
      Self::Circle { diameter, fill } => JsonValue::Object(vec![
        ("n".to_string(), JsonValue::Str("circle".to_string())),
        ("diameter".to_string(), JsonValue::Float(*diameter as f64)),
        ("fill".to_string(), fill.to_json()),
      ]),
      Self::Bar {
        height,
        radius,
        fill,
      } => JsonValue::Object(vec![
        ("n".to_string(), JsonValue::Str("bar".to_string())),
        ("height".to_string(), JsonValue::Float(*height as f64)),
        ("radius".to_string(), JsonValue::Float(*radius as f64)),
        ("fill".to_string(), fill.to_json()),
      ]),
    }
  }

  /// Decode from JSON. Unknown names and wrong field types fall back to safe
  /// defaults rather than failing.
  pub fn from_json(value: &JsonValue) -> Node {
    let name = value.get("n").and_then(|v| v.as_str()).unwrap_or("");
    let children = || match value.get("children") {
      Some(JsonValue::Array(items)) => items.iter().map(Node::from_json).collect(),
      _ => Vec::new(),
    };
    let num = |key: &str, fallback: f32| {
      value
        .get(key)
        .and_then(|v| v.as_f64())
        .map(|v| v as f32)
        .unwrap_or(fallback)
    };

    match name {
      "row" => Node::Row {
        spacing: clamp_spacing(num("spacing", 8.0)),
        align: Align::from_str(value.get("align").and_then(|v| v.as_str()).unwrap_or("")),
        children: children(),
      },
      "column" => Node::Column {
        spacing: clamp_spacing(num("spacing", 8.0)),
        align: Align::from_str(value.get("align").and_then(|v| v.as_str()).unwrap_or("")),
        children: children(),
      },
      "stack" => Node::Stack {
        align: Align::from_str(value.get("align").and_then(|v| v.as_str()).unwrap_or("")),
        children: children(),
      },
      "spacer" => Node::Spacer {
        min: clamp_len(num("min", 0.0), 512.0),
        factor: clamp_len(num("factor", 1.0), 64.0),
      },
      "padding" => {
        let child = value
          .get("child")
          .map(Node::from_json)
          .unwrap_or(Node::Spacer {
            min: 0.0,
            factor: 0.0,
          });
        Node::Padding {
          all: clamp_len(num("all", 0.0), 128.0),
          child: Box::new(child),
        }
      }
      "text" => Node::Text {
        text: value
          .get("text")
          .and_then(|v| v.as_str())
          .unwrap_or_default()
          .to_string(),
        style: TextStyle::from_json(value.get("style")),
      },
      "symbol" => Node::Symbol {
        name: value
          .get("name")
          .and_then(|v| v.as_str())
          .unwrap_or("circle")
          .to_string(),
        size: clamp_len(num("size", 16.0), MAX_FONT_SIZE),
        tint: Paint::from_json(value.get("tint")),
      },
      "image" => Node::Image {
        path: value
          .get("path")
          .and_then(|v| v.as_str())
          .unwrap_or_default()
          .to_string(),
        width: clamp_len(num("w", 0.0), 4096.0),
        height: clamp_len(num("h", 0.0), 4096.0),
      },
      "progress" => Node::Progress {
        value: num("value", 0.0),
        total: num("total", 1.0).max(f32::EPSILON),
        thickness: clamp_len(num("thickness", 4.0), 64.0),
        tint: Paint::from_json(value.get("tint")),
      },
      "gauge" => Node::Gauge {
        value: num("value", 0.0),
        total: num("total", 1.0).max(f32::EPSILON),
        diameter: clamp_len(num("diameter", 32.0), 4096.0),
        tint: Paint::from_json(value.get("tint")),
      },
      "circle" => Node::Circle {
        diameter: clamp_len(num("diameter", 24.0), 4096.0),
        fill: Paint::from_json(value.get("fill")),
      },
      "bar" => Node::Bar {
        height: clamp_len(num("height", 1.0), 4096.0),
        radius: clamp_len(num("radius", 0.5), 4096.0),
        fill: Paint::from_json(value.get("fill")),
      },
      _ => Node::Spacer {
        min: 0.0,
        factor: 0.0,
      },
    }
  }

  /// Decode from JSON text.
  pub fn parse(text: &str) -> Result<Node> {
    let value = JsonValue::parse(text)
      .map_err(|err| WidgetError::Parse(format!("widget content is not valid json: {err}")))?;
    Ok(Node::from_json(&value))
  }

  /// Encode as compact JSON text.
  pub fn to_text(&self) -> String {
    self.to_json().stringify(false)
  }
}

fn nodes_to_json(nodes: &[Node]) -> JsonValue {
  JsonValue::Array(nodes.iter().map(Node::to_json).collect())
}

/// Builder an app uses to assemble a [`Node`] tree.
///
/// ```
/// use widgetkit::{Content, Weight};
///
/// let tree = Content::column(10.0)
///   .align(widgetkit::Align::Center)
///   .child(Content::text("12:34").font_size(28.0).weight(Weight::Semibold))
///   .child(Content::symbol("clock").tint(widgetkit::Paint::Accent))
///   .build();
/// assert_eq!(tree.node_count(), 3);
/// ```
#[derive(Debug, Clone)]
pub struct Content {
  node: Node,
}

impl Content {
  fn wrap(node: Node) -> Self {
    Self { node }
  }

  /// Wrap an already built tree, e.g. one decoded from JSON, so it can be
  /// used as a child or as a root.
  ///
  /// ```
  /// use widgetkit::{Content, Node};
  ///
  /// let decoded = Node::parse(r#"{"n":"text","text":"hi"}"#).unwrap();
  /// assert_eq!(Content::from_node(decoded).build().node_count(), 1);
  /// ```
  pub fn from_node(node: Node) -> Self {
    Self { node }
  }

  /// Horizontal stack.
  pub fn row(spacing: f32) -> Self {
    Self::wrap(Node::Row {
      spacing: clamp_spacing(spacing),
      align: Align::Leading,
      children: Vec::new(),
    })
  }

  /// Vertical stack.
  pub fn column(spacing: f32) -> Self {
    Self::wrap(Node::Column {
      spacing: clamp_spacing(spacing),
      align: Align::Leading,
      children: Vec::new(),
    })
  }

  /// Overlay stack, every child shares the same rect.
  pub fn stack() -> Self {
    Self::wrap(Node::Stack {
      align: Align::Leading,
      children: Vec::new(),
    })
  }

  /// Flexible empty space.
  pub fn spacer(factor: f32) -> Self {
    Self::wrap(Node::Spacer {
      min: 0.0,
      factor: clamp_len(factor, 64.0),
    })
  }

  /// A text run.
  pub fn text(text: impl Into<String>) -> Self {
    Self::wrap(Node::Text {
      text: text.into(),
      style: TextStyle::default(),
    })
  }

  /// An SF Symbol by name.
  pub fn symbol(name: impl Into<String>) -> Self {
    Self::wrap(Node::Symbol {
      name: name.into(),
      size: 16.0,
      tint: Paint::Text,
    })
  }

  /// An image file.
  pub fn image(path: impl Into<String>) -> Self {
    Self::wrap(Node::Image {
      path: path.into(),
      width: 0.0,
      height: 0.0,
    })
  }

  /// Horizontal progress bar.
  pub fn progress(value: f32, total: f32) -> Self {
    Self::wrap(Node::Progress {
      value,
      total: total.max(f32::EPSILON),
      thickness: 4.0,
      tint: Paint::Accent,
    })
  }

  /// Circular progress ring.
  pub fn gauge(value: f32, total: f32) -> Self {
    Self::wrap(Node::Gauge {
      value,
      total: total.max(f32::EPSILON),
      diameter: 32.0,
      tint: Paint::Accent,
    })
  }

  /// Filled circle.
  pub fn circle(diameter: f32) -> Self {
    Self::wrap(Node::Circle {
      diameter: clamp_len(diameter, 4096.0),
      fill: Paint::Accent,
    })
  }

  /// Rounded bar, used as a separator or a track.
  pub fn bar(height: f32) -> Self {
    Self::wrap(Node::Bar {
      height: clamp_len(height, 4096.0),
      radius: height / 2.0,
      fill: Paint::Background,
    })
  }

  /// Cross-axis alignment of a stack.
  pub fn align(mut self, align: Align) -> Self {
    match &mut self.node {
      Node::Row { align: slot, .. }
      | Node::Column { align: slot, .. }
      | Node::Stack { align: slot, .. } => *slot = align,
      _ => {}
    }
    self
  }

  /// Spacing between children of a stack.
  pub fn spacing(mut self, spacing: f32) -> Self {
    if let Node::Row { spacing: slot, .. } | Node::Column { spacing: slot, .. } = &mut self.node {
      *slot = clamp_spacing(spacing);
    }
    self
  }

  /// Font size in logical px, clamped to
  /// [`MIN_FONT_SIZE`]..=[`MAX_FONT_SIZE`].
  pub fn font_size(mut self, size: f32) -> Self {
    if let Node::Text { style, .. } = &mut self.node {
      style.size = clamp_size(size);
    }
    self
  }

  /// Font weight of a text node.
  pub fn weight(mut self, weight: Weight) -> Self {
    if let Node::Text { style, .. } = &mut self.node {
      style.weight = weight;
    }
    self
  }

  /// Horizontal alignment of a text node.
  pub fn text_align(mut self, align: TextAlign) -> Self {
    if let Node::Text { style, .. } = &mut self.node {
      style.align = align;
    }
    self
  }

  /// Tint of a symbol, progress, gauge or circle.
  pub fn tint(mut self, paint: Paint) -> Self {
    match &mut self.node {
      Node::Symbol { tint: slot, .. }
      | Node::Progress { tint: slot, .. }
      | Node::Gauge { tint: slot, .. }
      | Node::Circle { fill: slot, .. } => *slot = paint,
      Node::Text { style, .. } => style.color = paint,
      Node::Bar { fill: slot, .. } => *slot = paint,
      _ => {}
    }
    self
  }

  /// Text color of a text node.
  pub fn color(self, paint: Paint) -> Self {
    self.tint(paint)
  }

  /// Symbol size in logical px.
  pub fn symbol_size(mut self, size: f32) -> Self {
    if let Node::Symbol { size: slot, .. } = &mut self.node {
      *slot = clamp_len(size, MAX_FONT_SIZE);
    }
    self
  }

  /// Explicit image size in logical px. Zero means intrinsic size.
  pub fn image_size(mut self, width: f32, height: f32) -> Self {
    if let Node::Image {
      width: w, height: h, ..
    } = &mut self.node
    {
      *w = clamp_len(width, 4096.0);
      *h = clamp_len(height, 4096.0);
    }
    self
  }

  /// Ring or bar thickness in logical px.
  pub fn thickness(mut self, thickness: f32) -> Self {
    match &mut self.node {
      Node::Progress { thickness: slot, .. } => *slot = clamp_len(thickness, 64.0),
      Node::Gauge { diameter, .. } => *diameter = clamp_len(thickness, 4096.0),
      _ => {}
    }
    self
  }

  /// Minimum extent of a spacer in logical px.
  pub fn min(mut self, min: f32) -> Self {
    if let Node::Spacer { min: slot, .. } = &mut self.node {
      *slot = clamp_len(min, 512.0);
    }
    self
  }

  /// Inset around this node and its subtree.
  pub fn padding(self, all: f32) -> Self {
    let all = clamp_len(all, 128.0);
    Self::wrap(Node::Padding {
      all,
      child: Box::new(self.node),
    })
  }

  /// Append a child to a stack. Ignored on leaves.
  pub fn child(mut self, child: Content) -> Self {
    if let Node::Row { children, .. }
    | Node::Column { children, .. }
    | Node::Stack { children, .. } = &mut self.node
    {
      children.push(child.node);
    }
    self
  }

  /// Append several children to a stack.
  pub fn children<I: IntoIterator<Item = Content>>(mut self, children: I) -> Self {
    for child in children {
      self = self.child(child);
    }
    self
  }

  /// Finish the builder.
  pub fn build(self) -> Node {
    self.node
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn sample() -> Node {
    Content::column(10.0)
      .align(Align::Center)
      .child(Content::text("12:34").font_size(28.0).weight(Weight::Semibold))
      .child(
        Content::row(6.0)
          .child(Content::symbol("battery.100").symbol_size(14.0))
          .child(Content::text("100 %")),
      )
      .child(Content::progress(3.0, 4.0).thickness(6.0))
      .child(Content::gauge(1.0, 2.0))
      .child(Content::circle(24.0))
      .child(Content::bar(1.0))
      .child(Content::image("/tmp/a.png").image_size(32.0, 32.0))
      .child(Content::stack().child(Content::spacer(1.0).min(4.0)))
      .padding(12.0)
      .build()
  }

  #[test]
  fn every_node_kind_round_trips() {
    let tree = sample();
    let text = tree.to_text();
    let decoded = Node::parse(&text).unwrap();
    assert_eq!(decoded, tree);
  }

  #[test]
  fn node_count_walks_the_whole_subtree() {
    let tree = sample();
    let counted = tree.node_count();
    let decoded = Node::parse(&tree.to_text()).unwrap();
    assert_eq!(decoded.node_count(), counted);
    assert!(counted > 10, "sample tree should be large, got {counted}");
  }

  #[test]
  fn unknown_node_name_degrades_to_an_empty_spacer() {
    let raw = r#"{"n":"hyperdrive"}"#;
    let node = Node::parse(raw).unwrap();
    assert_eq!(node, Node::Spacer { min: 0.0, factor: 0.0 });
  }

  #[test]
  fn wrong_field_types_fall_back_to_defaults() {
    let raw = r#"{"n":"text","text":"hi","style":{"size":"huge","weight":"nope"}}"#;
    let node = Node::parse(raw).unwrap();
    match node {
      Node::Text { text, style } => {
        assert_eq!(text, "hi");
        assert_eq!(style.size, TextStyle::default().size);
        assert_eq!(style.weight, Weight::Regular);
      }
      other => panic!("expected text, got {other:?}"),
    }
  }

  #[test]
  fn invalid_json_is_a_parse_error() {
    let err = Node::parse("{not json").unwrap_err();
    assert!(matches!(err, WidgetError::Parse(_)));
  }

  #[test]
  fn font_size_is_clamped_to_the_allowed_range() {
    match Content::text("x").font_size(1000.0).build() {
      Node::Text { style, .. } => assert_eq!(style.size, MAX_FONT_SIZE),
      other => panic!("expected text, got {other:?}"),
    }
    match Content::text("x").font_size(-5.0).build() {
      Node::Text { style, .. } => assert_eq!(style.size, MIN_FONT_SIZE),
      other => panic!("expected text, got {other:?}"),
    }
    match Content::text("x").font_size(f32::NAN).build() {
      Node::Text { style, .. } => assert_eq!(style.size, TextStyle::default().size),
      other => panic!("expected text, got {other:?}"),
    }
  }

  #[test]
  fn rgba_paint_round_trips_through_hex() {
    let paint = Paint::Rgba(0x12, 0x34, 0x56, 0x78);
    let encoded = paint.to_json();
    assert_eq!(encoded.as_str(), Some("#12345678"));
    assert_eq!(Paint::from_json(Some(&encoded)), paint);
  }

  #[test]
  fn a_broken_hex_paint_falls_back_to_text() {
    let broken = JsonValue::Str("#zz".to_string());
    assert_eq!(Paint::from_json(Some(&broken)), Paint::Text);
    let short = JsonValue::Str("#abc".to_string());
    assert_eq!(Paint::from_json(Some(&short)), Paint::Text);
  }

  #[test]
  fn progress_total_never_reaches_zero() {
    let raw = r#"{"n":"progress","value":1.0,"total":0.0}"#;
    match Node::parse(raw).unwrap() {
      Node::Progress { total, .. } => assert!(total > 0.0),
      other => panic!("expected progress, got {other:?}"),
    }
  }

  #[test]
  fn padding_wraps_the_previous_node() {
    let inner = Content::text("x").build();
    match Content::from_node(inner.clone()).padding(8.0).build() {
      Node::Padding { all, child } => {
        assert_eq!(all, 8.0);
        assert_eq!(*child, inner);
      }
      other => panic!("expected padding, got {other:?}"),
    }
  }

  #[test]
  fn a_padded_tree_is_counted_through() {
    let bare = Content::text("x").build();
    assert_eq!(bare.node_count(), 1);
    let padded = Content::from_node(bare).padding(4.0).build();
    assert_eq!(padded.node_count(), 2);
    assert_eq!(padded.children().len(), 1);
  }

  #[test]
  fn leaf_nodes_ignore_children_and_containers_ignore_styles() {
    let leaf = Content::text("x").child(Content::text("y")).build();
    assert_eq!(leaf.children().len(), 0);

    let stack = Content::stack().font_size(40.0).build();
    assert!(matches!(stack, Node::Stack { .. }));
  }

  #[test]
  fn weights_and_alignments_round_trip() {
    for weight in [Weight::Regular, Weight::Medium, Weight::Semibold, Weight::Bold] {
      assert_eq!(Weight::from_str(weight.as_str()), weight);
    }
    for align in [Align::Leading, Align::Center, Align::Trailing] {
      assert_eq!(Align::from_str(align.as_str()), align);
    }
    for align in [TextAlign::Leading, TextAlign::Center, TextAlign::Trailing] {
      assert_eq!(TextAlign::from_str(align.as_str()), align);
    }
  }
}