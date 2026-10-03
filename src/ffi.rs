//! C bindings for WidgetKit.
//!
//! Every entry point converts a Rust panic into a NULL handle, -1 or 0 return
//! value plus a thread local message readable through
//! [`widgetkit_last_error`]. Strings handed to C are owned by the caller and
//! released with [`widgetkit_free_string`].

use std::cell::RefCell;
use std::ffi::{c_char, CStr, CString};
use std::mem::size_of;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::ptr;

/// C `size_t`.
pub type size_t = usize;

const _: () = assert!(size_of::<size_t>() == size_of::<usize>());

use foundation::json::JsonValue;

use crate::descriptor::{RegisteredWidget, WidgetDescriptor};
use crate::node::{Align, Content, Node, Paint, TextAlign, Weight};
use crate::registry::Registry;
use crate::size::WidgetSize;

// ---------------------------------------------------------------------------
// C types, mirroring Headers/widgetkit.h
// ---------------------------------------------------------------------------

/// Widget size variants, in context menu order.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetKitSize {
  Small = 0,
  Medium = 1,
  Large = 2,
}

/// Cross-axis alignment inside a stack.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetKitAlign {
  Leading = 0,
  Center = 1,
  Trailing = 2,
}

/// Text weight.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetKitWeight {
  Regular = 0,
  Medium = 1,
  Semibold = 2,
  Bold = 3,
}

/// Horizontal text alignment.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetKitTextAlign {
  Leading = 0,
  Center = 1,
  Trailing = 2,
}

/// Color request from an app.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetKitPaint {
  Background = 0,
  Text = 1,
  Accent = 2,
  Rgba = 3,
}

impl From<WidgetKitSize> for WidgetSize {
  fn from(value: WidgetKitSize) -> Self {
    match value {
      WidgetKitSize::Small => Self::Small,
      WidgetKitSize::Medium => Self::Medium,
      WidgetKitSize::Large => Self::Large,
    }
  }
}

impl From<WidgetKitAlign> for Align {
  fn from(value: WidgetKitAlign) -> Self {
    match value {
      WidgetKitAlign::Leading => Self::Leading,
      WidgetKitAlign::Center => Self::Center,
      WidgetKitAlign::Trailing => Self::Trailing,
    }
  }
}

impl From<WidgetKitWeight> for Weight {
  fn from(value: WidgetKitWeight) -> Self {
    match value {
      WidgetKitWeight::Regular => Self::Regular,
      WidgetKitWeight::Medium => Self::Medium,
      WidgetKitWeight::Semibold => Self::Semibold,
      WidgetKitWeight::Bold => Self::Bold,
    }
  }
}

impl From<WidgetKitTextAlign> for TextAlign {
  fn from(value: WidgetKitTextAlign) -> Self {
    match value {
      WidgetKitTextAlign::Leading => Self::Leading,
      WidgetKitTextAlign::Center => Self::Center,
      WidgetKitTextAlign::Trailing => Self::Trailing,
    }
  }
}

// ---------------------------------------------------------------------------
// Handles and error plumbing
// ---------------------------------------------------------------------------

/// Opaque descriptor handle.
pub struct WidgetDescriptorHandle {
  inner: WidgetDescriptor,
}

/// Opaque content builder handle.
pub struct WidgetContentHandle {
  inner: Content,
}

thread_local! {
  static LAST_ERROR: RefCell<Option<CString>> = const { RefCell::new(None) };
  /// Owned copy of the socket path set through `widgetkit_set_socket_path`.
  static SOCKET_OVERRIDE: RefCell<Option<CString>> = const { RefCell::new(None) };
}

fn set_error(message: impl Into<String>) {
  let text = CString::new(message.into()).unwrap_or_else(|_| c"error".to_owned());
  LAST_ERROR.with(|slot| *slot.borrow_mut() = Some(text));
}

fn clear_error() {
  LAST_ERROR.with(|slot| *slot.borrow_mut() = None);
}

/// Run `body`, turning a panic into `fallback` plus a recorded error.
fn guard<T>(fallback: T, body: impl FnOnce() -> T) -> T {
  match catch_unwind(AssertUnwindSafe(body)) {
    Ok(value) => value,
    Err(_) => {
      set_error("widgetkit: panic in FFI call");
      fallback
    }
  }
}

/// Read a C string into an owned `String`. Returns `None` for NULL.
unsafe fn borrow_str(raw: *const c_char) -> Option<String> {
  if raw.is_null() {
    return None;
  }
  CStr::from_ptr(raw).to_str().ok().map(str::to_owned)
}

/// Hand a Rust string to C as an owned allocation.
unsafe fn export(text: String) -> *mut c_char {
  match CString::new(text) {
    Ok(value) => value.into_raw(),
    Err(_) => {
      set_error("widgetkit: interior nul in returned string");
      ptr::null_mut()
    }
  }
}

/// Shared descriptor of a handle, `None` when the pointer is NULL.
unsafe fn descriptor<'a>(handle: *const WidgetDescriptorHandle) -> Option<&'a WidgetDescriptor> {
  if handle.is_null() {
    None
  } else {
    Some(&(*handle).inner)
  }
}

/// Mutable descriptor of a handle, `None` when the pointer is NULL.
unsafe fn descriptor_mut<'a>(
  handle: *mut WidgetDescriptorHandle,
) -> Option<&'a mut WidgetDescriptor> {
  if handle.is_null() {
    None
  } else {
    Some(&mut (*handle).inner)
  }
}

/// Shared content builder of a handle, `None` when the pointer is NULL.
unsafe fn builder<'a>(handle: *const WidgetContentHandle) -> Option<&'a Content> {
  if handle.is_null() {
    None
  } else {
    Some(&(*handle).inner)
  }
}

/// Mutable content builder of a handle, `None` when the pointer is NULL.
unsafe fn builder_mut<'a>(handle: *mut WidgetContentHandle) -> Option<&'a mut Content> {
  if handle.is_null() {
    None
  } else {
    Some(&mut (*handle).inner)
  }
}

/// Registry honouring the socket override set through the C API.
fn registry() -> Registry {
  let override_path = SOCKET_OVERRIDE.with(|slot| slot.borrow().as_ref().map(|s| s.to_string_lossy().to_string()));
  match override_path {
    Some(path) => Registry::with_socket(path),
    None => Registry::from_env(),
  }
}

// ---------------------------------------------------------------------------
// Version and errors
// ---------------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn widgetkit_version() -> *const c_char {
  c"27.0.0".as_ptr()
}

#[no_mangle]
pub extern "C" fn widgetkit_last_error() -> *mut c_char {
  LAST_ERROR.with(|slot| match slot.borrow().as_ref() {
    Some(text) => text.clone().into_raw(),
    None => ptr::null_mut(),
  })
}

#[no_mangle]
pub extern "C" fn widgetkit_free_string(text: *mut c_char) {
  if !text.is_null() {
    drop(unsafe { CString::from_raw(text) });
  }
}

// ---------------------------------------------------------------------------
// Sizes
// ---------------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn widgetkit_cell() -> f32 {
  crate::size::CELL
}

#[no_mangle]
pub extern "C" fn widgetkit_gap() -> f32 {
  crate::size::GAP
}

#[no_mangle]
pub extern "C" fn widgetkit_span_px(cells: u32) -> f32 {
  crate::size::span_px(cells)
}

#[no_mangle]
pub extern "C" fn widgetkit_snap_to_grid(value: f32) -> f32 {
  crate::size::snap_to_grid(value)
}

#[no_mangle]
pub extern "C" fn widgetkit_size_name(size: WidgetKitSize) -> *const c_char {
  match size {
    WidgetKitSize::Small => c"small".as_ptr(),
    WidgetKitSize::Medium => c"medium".as_ptr(),
    WidgetKitSize::Large => c"large".as_ptr(),
  }
}

// ---------------------------------------------------------------------------
// Descriptors
// ---------------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn widgetkit_descriptor_new(
  id: *const c_char,
  name: *const c_char,
) -> *mut WidgetDescriptorHandle {
  guard(ptr::null_mut(), || {
    let Some(id) = (unsafe { borrow_str(id) }) else {
      set_error(crate::lang::t("empty_id"));
      return ptr::null_mut();
    };
    if id.trim().is_empty() {
      set_error(crate::lang::t("empty_id"));
      return ptr::null_mut();
    }
    let name = unsafe { borrow_str(name) }.unwrap_or_else(|| id.clone());
    Box::into_raw(Box::new(WidgetDescriptorHandle {
      inner: WidgetDescriptor::new(id, name),
    }))
  })
}

#[no_mangle]
pub extern "C" fn widgetkit_descriptor_set_icon(
  handle: *mut WidgetDescriptorHandle,
  icon: *const c_char,
) {
  guard((), || {
    let Some(icon) = (unsafe { borrow_str(icon) }) else {
      return;
    };
    if let Some(slot) = unsafe { descriptor_mut(handle) } {
      *slot = slot.clone().icon(icon);
    }
  })
}

#[no_mangle]
pub extern "C" fn widgetkit_descriptor_set_sizes(
  handle: *mut WidgetDescriptorHandle,
  sizes: *const WidgetKitSize,
  count: size_t,
) {
  guard((), || {
    if sizes.is_null() {
      return;
    }
    let list: Vec<WidgetSize> = (0..count)
      .filter_map(|index| {
        let raw = unsafe { *sizes.add(index) };
        WidgetSize::from(raw).into()
      })
      .collect();
    if let Some(slot) = unsafe { descriptor_mut(handle) } {
      *slot = slot.clone().sizes(list);
    }
  })
}

#[no_mangle]
pub extern "C" fn widgetkit_descriptor_add_size(
  handle: *mut WidgetDescriptorHandle,
  size: WidgetKitSize,
) {
  guard((), || {
    if let Some(slot) = unsafe { descriptor_mut(handle) } {
      *slot = slot.clone().add_size(size.into());
    }
  })
}

#[no_mangle]
pub extern "C" fn widgetkit_descriptor_set_default_size(
  handle: *mut WidgetDescriptorHandle,
  size: WidgetKitSize,
) {
  guard((), || {
    if let Some(slot) = unsafe { descriptor_mut(handle) } {
      *slot = slot.clone().default_size(size.into());
    }
  })
}

#[no_mangle]
pub extern "C" fn widgetkit_descriptor_set_refresh_interval(
  handle: *mut WidgetDescriptorHandle,
  seconds: u64,
) {
  guard((), || {
    if let Some(slot) = unsafe { descriptor_mut(handle) } {
      *slot = slot.clone().refresh_interval(seconds);
    }
  })
}

#[no_mangle]
pub extern "C" fn widgetkit_descriptor_set_summary(
  handle: *mut WidgetDescriptorHandle,
  summary: *const c_char,
) {
  guard((), || {
    let Some(summary) = (unsafe { borrow_str(summary) }) else {
      return;
    };
    if let Some(slot) = unsafe { descriptor_mut(handle) } {
      *slot = slot.clone().summary(summary);
    }
  })
}

#[no_mangle]
pub extern "C" fn widgetkit_descriptor_validate(handle: *const WidgetDescriptorHandle) -> i32 {
  guard(0, || {
    match unsafe { descriptor(handle) } {
      None => 0,
      Some(inner) => match inner.validate() {
        Ok(()) => {
          clear_error();
          1
        }
        Err(err) => {
          set_error(err.message());
          0
        }
      },
    }
  })
}

#[no_mangle]
pub extern "C" fn widgetkit_descriptor_to_json(
  handle: *const WidgetDescriptorHandle,
) -> *mut c_char {
  guard(ptr::null_mut(), || match unsafe { descriptor(handle) } {
    None => {
      set_error("widgetkit: descriptor handle is NULL");
      ptr::null_mut()
    }
    Some(inner) => unsafe { export(inner.to_json().stringify(false)) },
  })
}

#[no_mangle]
pub extern "C" fn widgetkit_descriptor_free(handle: *mut WidgetDescriptorHandle) {
  if !handle.is_null() {
    drop(unsafe { Box::from_raw(handle) });
  }
}

// ---------------------------------------------------------------------------
// Content trees
// ---------------------------------------------------------------------------

fn wrap(node: Content) -> *mut WidgetContentHandle {
  Box::into_raw(Box::new(WidgetContentHandle { inner: node }))
}

#[no_mangle]
pub extern "C" fn widgetkit_content_row(spacing: f32) -> *mut WidgetContentHandle {
  guard(ptr::null_mut(), || wrap(Content::row(spacing)))
}

#[no_mangle]
pub extern "C" fn widgetkit_content_column(spacing: f32) -> *mut WidgetContentHandle {
  guard(ptr::null_mut(), || wrap(Content::column(spacing)))
}

#[no_mangle]
pub extern "C" fn widgetkit_content_stack() -> *mut WidgetContentHandle {
  guard(ptr::null_mut(), || wrap(Content::stack()))
}

#[no_mangle]
pub extern "C" fn widgetkit_content_spacer(factor: f32) -> *mut WidgetContentHandle {
  guard(ptr::null_mut(), || wrap(Content::spacer(factor)))
}

#[no_mangle]
pub extern "C" fn widgetkit_content_text(text: *const c_char) -> *mut WidgetContentHandle {
  guard(ptr::null_mut(), || {
    wrap(Content::text(unsafe { borrow_str(text) }.unwrap_or_default()))
  })
}

#[no_mangle]
pub extern "C" fn widgetkit_content_symbol(name: *const c_char) -> *mut WidgetContentHandle {
  guard(ptr::null_mut(), || {
    wrap(Content::symbol(unsafe { borrow_str(name) }.unwrap_or_default()))
  })
}

#[no_mangle]
pub extern "C" fn widgetkit_content_image(path: *const c_char) -> *mut WidgetContentHandle {
  guard(ptr::null_mut(), || {
    wrap(Content::image(unsafe { borrow_str(path) }.unwrap_or_default()))
  })
}

#[no_mangle]
pub extern "C" fn widgetkit_content_progress(value: f32, total: f32) -> *mut WidgetContentHandle {
  guard(ptr::null_mut(), || wrap(Content::progress(value, total)))
}

#[no_mangle]
pub extern "C" fn widgetkit_content_gauge(value: f32, total: f32) -> *mut WidgetContentHandle {
  guard(ptr::null_mut(), || wrap(Content::gauge(value, total)))
}

#[no_mangle]
pub extern "C" fn widgetkit_content_circle(diameter: f32) -> *mut WidgetContentHandle {
  guard(ptr::null_mut(), || wrap(Content::circle(diameter)))
}

#[no_mangle]
pub extern "C" fn widgetkit_content_bar(height: f32) -> *mut WidgetContentHandle {
  guard(ptr::null_mut(), || wrap(Content::bar(height)))
}

#[no_mangle]
pub extern "C" fn widgetkit_content_set_align(
  handle: *mut WidgetContentHandle,
  align: WidgetKitAlign,
) {
  guard((), || {
    if let Some(slot) = unsafe { builder_mut(handle) } {
      *slot = slot.clone().align(align.into());
    }
  })
}

#[no_mangle]
pub extern "C" fn widgetkit_content_set_spacing(handle: *mut WidgetContentHandle, spacing: f32) {
  guard((), || {
    if let Some(slot) = unsafe { builder_mut(handle) } {
      *slot = slot.clone().spacing(spacing);
    }
  })
}

#[no_mangle]
pub extern "C" fn widgetkit_content_set_font_size(handle: *mut WidgetContentHandle, size: f32) {
  guard((), || {
    if let Some(slot) = unsafe { builder_mut(handle) } {
      *slot = slot.clone().font_size(size);
    }
  })
}

#[no_mangle]
pub extern "C" fn widgetkit_content_set_weight(
  handle: *mut WidgetContentHandle,
  weight: WidgetKitWeight,
) {
  guard((), || {
    if let Some(slot) = unsafe { builder_mut(handle) } {
      *slot = slot.clone().weight(weight.into());
    }
  })
}

#[no_mangle]
pub extern "C" fn widgetkit_content_set_text_align(
  handle: *mut WidgetContentHandle,
  align: WidgetKitTextAlign,
) {
  guard((), || {
    if let Some(slot) = unsafe { builder_mut(handle) } {
      *slot = slot.clone().text_align(align.into());
    }
  })
}

#[no_mangle]
pub extern "C" fn widgetkit_content_set_tint(
  handle: *mut WidgetContentHandle,
  paint: WidgetKitPaint,
) {
  guard((), || {
    let paint = match paint {
      WidgetKitPaint::Background => Paint::Background,
      WidgetKitPaint::Text => Paint::Text,
      WidgetKitPaint::Accent => Paint::Accent,
      // An rgba request without components falls back to plain text.
      WidgetKitPaint::Rgba => Paint::Text,
    };
    if let Some(slot) = unsafe { builder_mut(handle) } {
      *slot = slot.clone().tint(paint);
    }
  })
}

#[no_mangle]
pub extern "C" fn widgetkit_content_set_rgba(
  handle: *mut WidgetContentHandle,
  r: u8,
  g: u8,
  b: u8,
  a: u8,
) {
  guard((), || {
    if let Some(slot) = unsafe { builder_mut(handle) } {
      *slot = slot.clone().tint(Paint::Rgba(r, g, b, a));
    }
  })
}

#[no_mangle]
pub extern "C" fn widgetkit_content_set_symbol_size(
  handle: *mut WidgetContentHandle,
  size: f32,
) {
  guard((), || {
    if let Some(slot) = unsafe { builder_mut(handle) } {
      *slot = slot.clone().symbol_size(size);
    }
  })
}

#[no_mangle]
pub extern "C" fn widgetkit_content_set_image_size(
  handle: *mut WidgetContentHandle,
  width: f32,
  height: f32,
) {
  guard((), || {
    if let Some(slot) = unsafe { builder_mut(handle) } {
      *slot = slot.clone().image_size(width, height);
    }
  })
}

#[no_mangle]
pub extern "C" fn widgetkit_content_set_min(handle: *mut WidgetContentHandle, min: f32) {
  guard((), || {
    if let Some(slot) = unsafe { builder_mut(handle) } {
      *slot = slot.clone().min(min);
    }
  })
}

#[no_mangle]
pub extern "C" fn widgetkit_content_set_padding(handle: *mut WidgetContentHandle, all: f32) {
  guard((), || {
    if let Some(slot) = unsafe { builder_mut(handle) } {
      *slot = slot.clone().padding(all);
    }
  })
}

#[no_mangle]
pub extern "C" fn widgetkit_content_add_child(
  handle: *mut WidgetContentHandle,
  child: *mut WidgetContentHandle,
) {
  guard((), || {
    if child.is_null() {
      return;
    }
    // The child is consumed by the parent, matching the Rust API.
    let child = unsafe { Box::from_raw(child) };
    if let Some(slot) = unsafe { builder_mut(handle) } {
      *slot = slot.clone().child(child.inner);
    }
  })
}

#[no_mangle]
pub extern "C" fn widgetkit_content_node_count(handle: *const WidgetContentHandle) -> u32 {
  guard(0, || match unsafe { builder(handle) } {
    None => 0,
    Some(inner) => inner.clone().build().node_count() as u32,
  })
}

#[no_mangle]
pub extern "C" fn widgetkit_content_to_json(handle: *const WidgetContentHandle) -> *mut c_char {
  guard(ptr::null_mut(), || match unsafe { builder(handle) } {
    None => {
      set_error("widgetkit: content handle is NULL");
      ptr::null_mut()
    }
    Some(inner) => unsafe { export(inner.clone().build().to_text()) },
  })
}

#[no_mangle]
pub extern "C" fn widgetkit_content_from_json(json: *const c_char) -> *mut WidgetContentHandle {
  guard(ptr::null_mut(), || {
    let Some(json) = (unsafe { borrow_str(json) }) else {
      set_error("widgetkit: json pointer is NULL");
      return ptr::null_mut();
    };
    match Node::parse(&json) {
      Ok(node) => wrap(Content::from_node(node)),
      Err(err) => {
        set_error(err.message());
        ptr::null_mut()
      }
    }
  })
}

#[no_mangle]
pub extern "C" fn widgetkit_content_free(handle: *mut WidgetContentHandle) {
  if !handle.is_null() {
    drop(unsafe { Box::from_raw(handle) });
  }
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn widgetkit_set_socket_path(path: *const c_char) {
  SOCKET_OVERRIDE.with(|slot| {
    *slot.borrow_mut() = if path.is_null() {
      None
    } else {
      unsafe { borrow_str(path) }.map(CString::new).map(|value| value.ok()).flatten()
    };
  });
}

#[no_mangle]
pub extern "C" fn widgetkit_register(
  app: *const c_char,
  descriptors: *const *const WidgetDescriptorHandle,
  count: size_t,
) -> i32 {
  guard(-1, || {
    let Some(app) = (unsafe { borrow_str(app) }) else {
      set_error("widgetkit: app pointer is NULL");
      return -1;
    };
    if descriptors.is_null() || count == 0 {
      set_error(crate::lang::t("no_descriptors"));
      return -1;
    }
    let list: Vec<WidgetDescriptor> = (0..count)
      .filter_map(|index| {
        let handle = unsafe { *descriptors.add(index) };
        unsafe { descriptor(handle) }.cloned()
      })
      .collect();
    match registry().register(&app, &list) {
      Ok(total) => {
        clear_error();
        total as i32
      }
      Err(err) => {
        set_error(err.message());
        -1
      }
    }
  })
}

#[no_mangle]
pub extern "C" fn widgetkit_list() -> *mut c_char {
  guard(ptr::null_mut(), || match registry().list() {
    Ok(entries) => {
      clear_error();
      let payload = JsonValue::Array(entries.iter().map(RegisteredWidget::to_json).collect());
      unsafe { export(payload.stringify(false)) }
    }
    Err(err) => {
      set_error(err.message());
      ptr::null_mut()
    }
  })
}

#[no_mangle]
pub extern "C" fn widgetkit_unregister(app: *const c_char) -> i32 {
  guard(-1, || {
    let Some(app) = (unsafe { borrow_str(app) }) else {
      set_error("widgetkit: app pointer is NULL");
      return -1;
    };
    match registry().unregister(&app) {
      Ok(removed) => {
        clear_error();
        removed as i32
      }
      Err(err) => {
        set_error(err.message());
        -1
      }
    }
  })
}

#[no_mangle]
pub extern "C" fn widgetkit_publish_content(widget_id: *const c_char, json: *const c_char) -> i32 {
  guard(-1, || {
    let Some(widget_id) = (unsafe { borrow_str(widget_id) }) else {
      set_error("widgetkit: widget id pointer is NULL");
      return -1;
    };
    let Some(json) = (unsafe { borrow_str(json) }) else {
      set_error("widgetkit: json pointer is NULL");
      return -1;
    };
    let node = match Node::parse(&json) {
      Ok(node) => node,
      Err(err) => {
        set_error(err.message());
        return -1;
      }
    };
    match registry().publish_content(&widget_id, &node) {
      Ok(true) => {
        clear_error();
        1
      }
      Ok(false) => 0,
      Err(err) => {
        set_error(err.message());
        -1
      }
    }
  })
}

#[no_mangle]
pub extern "C" fn widgetkit_content_list() -> *mut c_char {
  guard(ptr::null_mut(), || match registry().content_list() {
    Ok(entries) => {
      clear_error();
      let payload = JsonValue::Array(
        entries
          .iter()
          .map(|(id, node)| {
            JsonValue::Object(vec![
              ("widget".to_string(), JsonValue::Str(id.clone())),
              ("content".to_string(), node.to_json()),
            ])
          })
          .collect(),
      );
      unsafe { export(payload.stringify(false)) }
    }
    Err(err) => {
      set_error(err.message());
      ptr::null_mut()
    }
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn c_enum_values_match_the_header() {
    assert_eq!(WidgetKitSize::Small as i32, 0);
    assert_eq!(WidgetKitSize::Medium as i32, 1);
    assert_eq!(WidgetKitSize::Large as i32, 2);
    assert_eq!(WidgetKitAlign::Leading as i32, 0);
    assert_eq!(WidgetKitWeight::Bold as i32, 3);
    assert_eq!(WidgetKitTextAlign::Trailing as i32, 2);
    assert_eq!(WidgetKitPaint::Rgba as i32, 3);
  }

  #[test]
  fn grid_constants_are_exposed_to_c() {
    assert_eq!(widgetkit_cell(), 62.0);
    assert_eq!(widgetkit_gap(), 6.0);
    assert_eq!(widgetkit_span_px(2), 130.0);
    assert_eq!(widgetkit_span_px(4), 266.0);
    assert_eq!(widgetkit_snap_to_grid(0.0), 0.0);
  }

  #[test]
  fn c_enums_convert_into_rust_types() {
    assert_eq!(WidgetSize::from(WidgetKitSize::Large), WidgetSize::Large);
    assert_eq!(Align::from(WidgetKitAlign::Center), Align::Center);
    assert_eq!(Weight::from(WidgetKitWeight::Semibold), Weight::Semibold);
    assert_eq!(TextAlign::from(WidgetKitTextAlign::Center), TextAlign::Center);
  }

  #[test]
  fn a_null_descriptor_pointer_is_handled_everywhere() {
    assert_eq!(widgetkit_descriptor_validate(ptr::null()), 0);
    assert!(widgetkit_descriptor_to_json(ptr::null()).is_null());
    widgetkit_descriptor_set_icon(ptr::null_mut(), c"x".as_ptr());
    widgetkit_descriptor_free(ptr::null_mut());
    widgetkit_free_string(ptr::null_mut());
  }

  #[test]
  fn a_null_content_pointer_is_handled_everywhere() {
    assert_eq!(widgetkit_content_node_count(ptr::null()), 0);
    assert!(widgetkit_content_to_json(ptr::null()).is_null());
    assert!(widgetkit_content_from_json(ptr::null()).is_null());
    widgetkit_content_set_padding(ptr::null_mut(), 4.0);
    widgetkit_content_free(ptr::null_mut());
  }

  #[test]
  fn an_empty_descriptor_id_is_refused() {
    let empty = c"".as_ptr();
    assert!(widgetkit_descriptor_new(empty, c"Name".as_ptr()).is_null());
    assert!(!widgetkit_last_error().is_null());
    widgetkit_free_string(widgetkit_last_error());
    clear_error();
  }

  #[test]
  fn a_descriptor_survives_the_c_round_trip() {
    let handle = widgetkit_descriptor_new(
      c"com.tontoo.battery.level".as_ptr(),
      c"Battery".as_ptr(),
    );
    assert!(!handle.is_null());
    widgetkit_descriptor_set_icon(handle, c"battery.100".as_ptr());
    widgetkit_descriptor_set_refresh_interval(handle, 60);
    assert_eq!(widgetkit_descriptor_validate(handle), 1);

    let json = widgetkit_descriptor_to_json(handle);
    assert!(!json.is_null());
    let text = unsafe { CStr::from_ptr(json) }.to_string_lossy().to_string();
    assert!(text.contains("com.tontoo.battery.level"));
    assert!(text.contains("battery.100"));
    widgetkit_free_string(json);
    widgetkit_descriptor_free(handle);
  }

  #[test]
  fn a_content_tree_survives_the_c_round_trip() {
    let column = widgetkit_content_column(8.0);
    let label = widgetkit_content_text(c"12:34".as_ptr());
    widgetkit_content_set_font_size(label, 32.0);
    widgetkit_content_set_tint(label, WidgetKitPaint::Text);
    widgetkit_content_add_child(column, label);

    assert_eq!(widgetkit_content_node_count(column), 2);
    let json = widgetkit_content_to_json(column);
    assert!(!json.is_null());
    let text = unsafe { CStr::from_ptr(json) }.to_string_lossy().to_string();
    assert!(text.contains("12:34"));

    let decoded = widgetkit_content_from_json(json);
    assert!(!decoded.is_null());
    assert_eq!(widgetkit_content_node_count(decoded), 2);
    widgetkit_free_string(json);
    widgetkit_content_free(decoded);
    widgetkit_content_free(column);
  }

  #[test]
  fn invalid_content_json_is_refused_with_an_error() {
    let broken = c"{not json".as_ptr();
    assert!(widgetkit_content_from_json(broken).is_null());
    assert!(!widgetkit_last_error().is_null());
    widgetkit_free_string(widgetkit_last_error());
    clear_error();
  }

  #[test]
  fn registry_calls_against_a_missing_socket_report_minus_one() {
    widgetkit_set_socket_path(c"/nonexistent/tontoo-settings.sock".as_ptr());
    assert_eq!(widgetkit_unregister(c"com.tontoo.app".as_ptr()), -1);
    assert!(widgetkit_list().is_null());
    assert!(widgetkit_content_list().is_null());
    widgetkit_free_string(widgetkit_last_error());
    clear_error();
    widgetkit_set_socket_path(ptr::null());
  }

  #[test]
  fn registering_an_empty_descriptor_array_is_refused() {
    widgetkit_set_socket_path(c"/nonexistent/tontoo-settings.sock".as_ptr());
    assert_eq!(
      widgetkit_register(c"com.tontoo.app".as_ptr(), ptr::null(), 0),
      -1
    );
    assert_eq!(widgetkit_register(ptr::null(), ptr::null(), 1), -1);
    widgetkit_free_string(widgetkit_last_error());
    clear_error();
    widgetkit_set_socket_path(ptr::null());
  }

  #[test]
  fn publishing_to_a_missing_socket_reports_minus_one() {
    widgetkit_set_socket_path(c"/nonexistent/tontoo-settings.sock".as_ptr());
    let code = widgetkit_publish_content(
      c"com.tontoo.a.b".as_ptr(),
      c"{\"n\":\"text\",\"text\":\"x\"}".as_ptr(),
    );
    assert_eq!(code, -1);
    widgetkit_free_string(widgetkit_last_error());
    clear_error();
    widgetkit_set_socket_path(ptr::null());
  }

  #[test]
  fn the_version_string_is_the_package_version() {
    let text = unsafe { CStr::from_ptr(widgetkit_version()) }.to_string_lossy();
    assert_eq!(text, crate::WIDGETKIT_VERSION);
  }
}