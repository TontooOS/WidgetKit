//! Widget size variants and the desktop grid every widget snaps to.
//!
//! A desktop widget occupies a span of grid cells. Three fixed variants exist
//! ([`WidgetSize::Small`], [`WidgetSize::Medium`], [`WidgetSize::Large`]) plus
//! any free span the user drags out with the resize handle, bounded by
//! [`MIN_CELLS`] and [`MAX_CELLS`].
//!
//! All values are logical pixels; the widget engine multiplies by the output
//! scale when it rasterizes.

use crate::error::{Result, WidgetError};
use crate::lang::t;

/// Edge length of one grid cell in logical px.
pub const CELL: f32 = 62.0;

/// Gap between two cells in logical px.
pub const GAP: f32 = 6.0;

/// Smallest free span, in cells.
pub const MIN_CELLS: u32 = 2;

/// Largest free span, in cells.
pub const MAX_CELLS: u32 = 8;

/// Logical width of a span of `cells` columns.
pub fn span_px(cells: u32) -> f32 {
  let cells = cells.max(1) as f32;
  cells * CELL + (cells - 1.0) * GAP
}

/// Logical size in px of a `columns` x `rows` cell span.
pub fn span_size(columns: u32, rows: u32) -> (f32, f32) {
  (span_px(columns), span_px(rows))
}

/// The three fixed widget size variants offered in the context menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum WidgetSize {
  /// 2x2 cells, square. The compact accessory grid.
  Small,
  /// 4x2 cells, wide. The default variant.
  Medium,
  /// 4x4 cells, square. Tall list style layouts.
  Large,
}

impl WidgetSize {
  /// All variants in context menu order.
  pub const ALL: [WidgetSize; 3] = [WidgetSize::Small, WidgetSize::Medium, WidgetSize::Large];

  /// Cell span of this variant, `(columns, rows)`.
  pub fn cells(self) -> (u32, u32) {
    match self {
      Self::Small => (2, 2),
      Self::Medium => (4, 2),
      Self::Large => (4, 4),
    }
  }

  /// Logical size in px of this variant.
  pub fn size(self) -> (f32, f32) {
    let (columns, rows) = self.cells();
    span_size(columns, rows)
  }

  /// Stable lowercase name, used on the wire and in the store.
  pub fn as_str(self) -> &'static str {
    match self {
      Self::Small => "small",
      Self::Medium => "medium",
      Self::Large => "large",
    }
  }

  /// Parse [`WidgetSize::as_str`]. Unknown names yield
  /// [`WidgetError::InvalidDescriptor`].
  pub fn from_str(raw: &str) -> Result<Self> {
    match raw {
      "small" => Ok(Self::Small),
      "medium" => Ok(Self::Medium),
      "large" => Ok(Self::Large),
      other => Err(WidgetError::InvalidDescriptor(format!(
        "{}: {other}",
        t("invalid_size")
      ))),
    }
  }

  /// Index in [`WidgetSize::ALL`], used for the context menu checkmark.
  pub fn menu_index(self) -> usize {
    match self {
      Self::Small => 0,
      Self::Medium => 1,
      Self::Large => 2,
    }
  }

  /// The variant whose cell span matches exactly, if any. A free span that
  /// matches nothing returns `None`.
  pub fn from_cells(columns: u32, rows: u32) -> Option<Self> {
    Self::ALL.into_iter().find(|size| size.cells() == (columns, rows))
  }
}

impl Default for WidgetSize {
  fn default() -> Self {
    Self::Medium
  }
}

impl std::fmt::Display for WidgetSize {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str(self.as_str())
  }
}

/// A widget footprint on the desktop grid.
///
/// Fixed variants are a [`GridSpan`] of exactly their cells; a widget the user
/// resized by hand keeps a free span that may match no variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GridSpan {
  columns: u32,
  rows: u32,
}

impl GridSpan {
  /// Clamp `columns` and `rows` into `[MIN_CELLS, MAX_CELLS]`.
  pub fn new(columns: u32, rows: u32) -> Self {
    Self {
      columns: columns.clamp(MIN_CELLS, MAX_CELLS),
      rows: rows.clamp(MIN_CELLS, MAX_CELLS),
    }
  }

  /// The span of a fixed variant.
  pub fn of(size: WidgetSize) -> Self {
    let (columns, rows) = size.cells();
    Self { columns, rows }
  }

  /// Span covering an arbitrary px size, rounding to whole cells.
  pub fn from_px(width: f32, height: f32) -> Self {
    Self::new(px_to_cells(width), px_to_cells(height))
  }

  pub fn columns(self) -> u32 {
    self.columns
  }

  pub fn rows(self) -> u32 {
    self.rows
  }

  /// Logical size in px of this span.
  pub fn size(self) -> (f32, f32) {
    span_size(self.columns, self.rows)
  }

  /// The fixed variant with this exact span, if there is one.
  pub fn variant(self) -> Option<WidgetSize> {
    WidgetSize::from_cells(self.columns, self.rows)
  }

  /// Snap a px size up to the next whole-cell boundary, so a drag never ends
  /// on a fractional cell.
  pub fn snapped_from_px(self, width: f32, height: f32) -> Self {
    Self::new(px_to_cells(width), px_to_cells(height))
  }
}

impl Default for GridSpan {
  fn default() -> Self {
    Self::of(WidgetSize::default())
  }
}

/// Convert a px length to a whole cell count, rounding to nearest and never
/// returning less than one.
pub fn px_to_cells(value: f32) -> u32 {
  if !value.is_finite() || value <= 0.0 {
    return 1;
  }
  let cells = ((value + GAP) / (CELL + GAP)).round();
  if cells < 1.0 {
    1
  } else {
    cells as u32
  }
}

/// Snap a coordinate to the nearest cell boundary.
pub fn snap_to_grid(value: f32) -> f32 {
  if !value.is_finite() {
    return 0.0;
  }
  let step = CELL + GAP;
  (value / step).round() * step
}

/// Largest free span in px, used to clamp the resize handle.
pub fn max_span_size() -> (f32, f32) {
  span_size(MAX_CELLS, MAX_CELLS)
}

/// Smallest free span in px, used to clamp the resize handle.
pub fn min_span_size() -> (f32, f32) {
  span_size(MIN_CELLS, MIN_CELLS)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn fixed_variants_match_the_design() {
    assert_eq!(WidgetSize::Small.size(), (130.0, 130.0));
    assert_eq!(WidgetSize::Medium.size(), (266.0, 130.0));
    assert_eq!(WidgetSize::Large.size(), (266.0, 266.0));
  }

  #[test]
  fn variant_cell_spans_are_two_four_and_four() {
    assert_eq!(WidgetSize::Small.cells(), (2, 2));
    assert_eq!(WidgetSize::Medium.cells(), (4, 2));
    assert_eq!(WidgetSize::Large.cells(), (4, 4));
  }

  #[test]
  fn size_round_trips_through_its_name() {
    for size in WidgetSize::ALL {
      assert_eq!(WidgetSize::from_str(size.as_str()).unwrap(), size);
    }
  }

  #[test]
  fn unknown_size_name_is_rejected() {
    let err = WidgetSize::from_str("huge").unwrap_err();
    assert!(matches!(err, WidgetError::InvalidDescriptor(_)));
    assert!(err.to_string().contains("huge"));
  }

  #[test]
  fn default_is_medium() {
    assert_eq!(WidgetSize::default(), WidgetSize::Medium);
    assert_eq!(GridSpan::default().variant(), Some(WidgetSize::Medium));
  }

  #[test]
  fn free_span_is_clamped_to_the_allowed_range() {
    let tiny = GridSpan::new(1, 0);
    assert_eq!((tiny.columns(), tiny.rows()), (MIN_CELLS, MIN_CELLS));

    let huge = GridSpan::new(99, 99);
    assert_eq!((huge.columns(), huge.rows()), (MAX_CELLS, MAX_CELLS));
    assert_eq!(huge.size(), max_span_size());
  }

  #[test]
  fn px_to_cells_rounds_to_nearest() {
    assert_eq!(px_to_cells(130.0), 2);
    assert_eq!(px_to_cells(266.0), 4);
    assert_eq!(px_to_cells(60.0), 1);
    assert_eq!(px_to_cells(f32::NAN), 1);
    assert_eq!(px_to_cells(-5.0), 1);
  }

  #[test]
  fn grid_span_from_px_recovers_the_variant() {
    let span = GridSpan::from_px(266.0, 130.0);
    assert_eq!(span.variant(), Some(WidgetSize::Medium));
    let square = GridSpan::from_px(130.0, 130.0);
    assert_eq!(square.variant(), Some(WidgetSize::Small));
  }

  #[test]
  fn a_free_span_matches_no_variant() {
    let span = GridSpan::new(3, 2);
    assert_eq!(span.variant(), None);
    assert_eq!(span.size(), span_size(3, 2));
  }

  #[test]
  fn snapping_lands_on_cell_boundaries() {
    let step = CELL + GAP;
    assert_eq!(snap_to_grid(0.0), 0.0);
    assert_eq!(snap_to_grid(step * 2.0), step * 2.0);
    assert_eq!(snap_to_grid(step * 1.4), step);
    assert_eq!(snap_to_grid(step * 1.6), step * 2.0);
    assert_eq!(snap_to_grid(f32::INFINITY), 0.0);
  }

  #[test]
  fn menu_index_matches_the_context_menu_order() {
    assert_eq!(WidgetSize::Small.menu_index(), 0);
    assert_eq!(WidgetSize::Medium.menu_index(), 1);
    assert_eq!(WidgetSize::Large.menu_index(), 2);
  }
}