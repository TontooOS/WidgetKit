/*
 * Tontoo WidgetKit - C Header
 * Declare desktop widgets and register them with the settings daemon.
 * Companion to the Rust crate; all calls are blocking and thread safe.
 */

#ifndef WIDGETKIT_H
#define WIDGETKIT_H

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ======================== */
/* Version                  */
/* ======================== */

/**
 * Get the WidgetKit library version.
 * @return static version string, e.g. "27.0.0" (do NOT free)
 */
const char* widgetkit_version(void);

/**
 * Get the last error message for the current thread.
 * @return allocated string (free with widgetkit_free_string) or NULL
 */
char* widgetkit_last_error(void);

/**
 * Free a string returned by widgetkit_* functions.
 * @param s string to free
 */
void widgetkit_free_string(char *s);

/* ======================== */
/* Sizes                    */
/* ======================== */

/** Widget size variants, in context menu order. */
typedef enum {
  WIDGETKIT_SIZE_SMALL = 0,  /**< 2x2 cells, 130x130 px. */
  WIDGETKIT_SIZE_MEDIUM = 1, /**< 4x2 cells, 266x130 px. */
  WIDGETKIT_SIZE_LARGE = 2   /**< 4x4 cells, 266x266 px. */
} WidgetKitSize;

/**
 * Logical width of one grid cell in px.
 * @return 62.0
 */
float widgetkit_cell(void);

/**
 * Logical gap between two cells in px.
 * @return 6.0
 */
float widgetkit_gap(void);

/**
 * Logical width of a span of cells.
 * @param cells cell count, values below 1 are treated as 1
 * @return width in logical px
 */
float widgetkit_span_px(uint32_t cells);

/**
 * Snap a coordinate to the nearest grid cell boundary.
 * @param value coordinate in logical px
 * @return snapped coordinate in logical px
 */
float widgetkit_snap_to_grid(float value);

/**
 * Stable lowercase name of a size variant.
 * @param size variant
 * @return "small", "medium" or "large" (do NOT free)
 */
const char* widgetkit_size_name(WidgetKitSize size);

/* ======================== */
/* Descriptors              */
/* ======================== */

/** Opaque descriptor handle. */
typedef struct WidgetDescriptorHandle WidgetDescriptorHandle;

/**
 * Create a descriptor.
 *
 * @param id globally unique widget id, must not be empty
 * @param name display name, already localized by the app
 * @return handle, or NULL when id is NULL/empty (check widgetkit_last_error)
 */
WidgetDescriptorHandle* widgetkit_descriptor_new(const char *id, const char *name);

/**
 * Set the SF Symbol name or absolute icon path.
 * @param handle descriptor
 * @param icon icon name or path
 */
void widgetkit_descriptor_set_icon(WidgetDescriptorHandle *handle, const char *icon);

/**
 * Replace the supported sizes.
 * @param handle descriptor
 * @param sizes array of WidgetKitSize
 * @param count number of entries in sizes
 */
void widgetkit_descriptor_set_sizes(WidgetDescriptorHandle *handle,
                                    const WidgetKitSize *sizes, size_t count);

/**
 * Add one supported size.
 * @param handle descriptor
 * @param size variant to add, ignored when already present
 */
void widgetkit_descriptor_add_size(WidgetDescriptorHandle *handle, WidgetKitSize size);

/**
 * Set the size a freshly placed widget starts at.
 * @param handle descriptor
 * @param size variant, ignored when not in the supported list
 */
void widgetkit_descriptor_set_default_size(WidgetDescriptorHandle *handle, WidgetKitSize size);

/**
 * Set the seconds between content pushes.
 * @param handle descriptor
 * @param seconds interval, 0 means change driven only
 */
void widgetkit_descriptor_set_refresh_interval(WidgetDescriptorHandle *handle,
                                               uint64_t seconds);

/**
 * Set the one line description shown in the edit panel.
 * @param handle descriptor
 * @param summary description text
 */
void widgetkit_descriptor_set_summary(WidgetDescriptorHandle *handle, const char *summary);

/**
 * Validate a descriptor.
 * @param handle descriptor
 * @return 1 when valid, 0 when the id is empty or no size is supported
 */
int widgetkit_descriptor_validate(const WidgetDescriptorHandle *handle);

/**
 * Encode a descriptor as JSON.
 * @param handle descriptor
 * @return allocated JSON string (free with widgetkit_free_string) or NULL
 */
char* widgetkit_descriptor_to_json(const WidgetDescriptorHandle *handle);

/**
 * Free a descriptor.
 * @param handle descriptor, may be NULL
 */
void widgetkit_descriptor_free(WidgetDescriptorHandle *handle);

/* ======================== */
/* Content trees            */
/* ======================== */

/** Opaque content builder handle. */
typedef struct WidgetContentHandle WidgetContentHandle;

/** Cross-axis alignment inside a stack. */
typedef enum {
  WIDGETKIT_ALIGN_LEADING = 0,
  WIDGETKIT_ALIGN_CENTER = 1,
  WIDGETKIT_ALIGN_TRAILING = 2
} WidgetKitAlign;

/** Text weight. */
typedef enum {
  WIDGETKIT_WEIGHT_REGULAR = 0,
  WIDGETKIT_WEIGHT_MEDIUM = 1,
  WIDGETKIT_WEIGHT_SEMIBOLD = 2,
  WIDGETKIT_WEIGHT_BOLD = 3
} WidgetKitWeight;

/** Horizontal text alignment. */
typedef enum {
  WIDGETKIT_TEXT_ALIGN_LEADING = 0,
  WIDGETKIT_TEXT_ALIGN_CENTER = 1,
  WIDGETKIT_TEXT_ALIGN_TRAILING = 2
} WidgetKitTextAlign;

/** Color request. The system widget chrome uses only background and text. */
typedef enum {
  WIDGETKIT_PAINT_BACKGROUND = 0,
  WIDGETKIT_PAINT_TEXT = 1,
  WIDGETKIT_PAINT_ACCENT = 2,
  WIDGETKIT_PAINT_RGBA = 3 /**< use widgetkit_content_set_rgba */
} WidgetKitPaint;

/**
 * Create a horizontal stack.
 * @param spacing gap between children in logical px
 * @return content handle
 */
WidgetContentHandle* widgetkit_content_row(float spacing);

/**
 * Create a vertical stack.
 * @param spacing gap between children in logical px
 * @return content handle
 */
WidgetContentHandle* widgetkit_content_column(float spacing);

/**
 * Create an overlay stack where every child shares the same rect.
 * @return content handle
 */
WidgetContentHandle* widgetkit_content_stack(void);

/**
 * Create flexible empty space.
 * @param factor share of the remaining space
 * @return content handle
 */
WidgetContentHandle* widgetkit_content_spacer(float factor);

/**
 * Create a text run.
 * @param text text content
 * @return content handle
 */
WidgetContentHandle* widgetkit_content_text(const char *text);

/**
 * Create an SF Symbol.
 * @param name symbol name
 * @return content handle
 */
WidgetContentHandle* widgetkit_content_symbol(const char *name);

/**
 * Create an image reference.
 * @param path absolute image path
 * @return content handle
 */
WidgetContentHandle* widgetkit_content_image(const char *path);

/**
 * Create a horizontal progress bar.
 * @param value current value
 * @param total maximum value, values at or below 0 are treated as 1
 * @return content handle
 */
WidgetContentHandle* widgetkit_content_progress(float value, float total);

/**
 * Create a circular progress ring.
 * @param value current value
 * @param total maximum value, values at or below 0 are treated as 1
 * @return content handle
 */
WidgetContentHandle* widgetkit_content_gauge(float value, float total);

/**
 * Create a filled circle.
 * @param diameter diameter in logical px
 * @return content handle
 */
WidgetContentHandle* widgetkit_content_circle(float diameter);

/**
 * Create a rounded bar, used as a separator or a track.
 * @param height height in logical px
 * @return content handle
 */
WidgetContentHandle* widgetkit_content_bar(float height);

/**
 * Set the cross-axis alignment of a stack.
 * @param handle content
 * @param align alignment
 */
void widgetkit_content_set_align(WidgetContentHandle *handle, WidgetKitAlign align);

/**
 * Set the spacing between children of a stack.
 * @param handle content
 * @param spacing gap in logical px, clamped to 0..64
 */
void widgetkit_content_set_spacing(WidgetContentHandle *handle, float spacing);

/**
 * Set the font size of a text node.
 * @param handle content
 * @param size logical px, clamped to 8..96
 */
void widgetkit_content_set_font_size(WidgetContentHandle *handle, float size);

/**
 * Set the weight of a text node.
 * @param handle content
 * @param weight weight
 */
void widgetkit_content_set_weight(WidgetContentHandle *handle, WidgetKitWeight weight);

/**
 * Set the horizontal alignment of a text node.
 * @param handle content
 * @param align alignment
 */
void widgetkit_content_set_text_align(WidgetContentHandle *handle, WidgetKitTextAlign align);

/**
 * Set the tint of a symbol, progress, gauge, circle or bar.
 * @param handle content
 * @param paint paint request
 */
void widgetkit_content_set_tint(WidgetContentHandle *handle, WidgetKitPaint paint);

/**
 * Set an explicit rgba tint. Implies WIDGETKIT_PAINT_RGBA.
 * @param handle content
 * @param r red 0..255
 * @param g green 0..255
 * @param b blue 0..255
 * @param a alpha 0..255
 */
void widgetkit_content_set_rgba(WidgetContentHandle *handle,
                                uint8_t r, uint8_t g, uint8_t b, uint8_t a);

/**
 * Set the symbol size.
 * @param handle content
 * @param size logical px
 */
void widgetkit_content_set_symbol_size(WidgetContentHandle *handle, float size);

/**
 * Set the explicit image size.
 * @param handle content
 * @param width logical px, 0 means intrinsic
 * @param height logical px, 0 means intrinsic
 */
void widgetkit_content_set_image_size(WidgetContentHandle *handle, float width, float height);

/**
 * Set the minimum extent of a spacer.
 * @param handle content
 * @param min logical px
 */
void widgetkit_content_set_min(WidgetContentHandle *handle, float min);

/**
 * Wrap this node and its subtree in an inset.
 * @param handle content
 * @param all inset in logical px, clamped to 0..128
 */
void widgetkit_content_set_padding(WidgetContentHandle *handle, float all);

/**
 * Append a child to a stack. Ignored on leaf nodes.
 * @param handle parent stack
 * @param child child content
 */
void widgetkit_content_add_child(WidgetContentHandle *handle, WidgetContentHandle *child);

/**
 * Count every node in the subtree, including the root.
 * @param handle content
 * @return node count
 */
uint32_t widgetkit_content_node_count(const WidgetContentHandle *handle);

/**
 * Encode a content tree as compact JSON.
 * @param handle content
 * @return allocated JSON string (free with widgetkit_free_string) or NULL
 */
char* widgetkit_content_to_json(const WidgetContentHandle *handle);

/**
 * Decode a content tree from JSON. Unknown node names become empty spacers.
 * @param json JSON text
 * @return content handle, or NULL when json is NULL or invalid
 */
WidgetContentHandle* widgetkit_content_from_json(const char *json);

/**
 * Free a content handle.
 * @param handle content, may be NULL
 */
void widgetkit_content_free(WidgetContentHandle *handle);

/* ======================== */
/* Registry                 */
/* ======================== */

/**
 * Register or replace the widgets of one app.
 *
 * Every descriptor is validated before anything is sent, so an invalid
 * descriptor leaves the registry untouched.
 *
 * @param app bundle id of the calling app
 * @param descriptors array of descriptor handles
 * @param count number of entries in descriptors
 * @return number of widgets now registered for app, or -1 on error
 */
int widgetkit_register(const char *app, const WidgetDescriptorHandle *const *descriptors,
                       size_t count);

/**
 * List every registered widget as a JSON array.
 *
 * @return allocated JSON string (free with widgetkit_free_string) or NULL
 */
char* widgetkit_list(void);

/**
 * Drop every widget registered by one app.
 * @param app bundle id
 * @return number of removed widgets, or -1 on error
 */
int widgetkit_unregister(const char *app);

/**
 * Push the content tree of one widget.
 * @param widget_id widget id, must be registered
 * @param json content tree JSON, see widgetkit_content_to_json
 * @return 1 when accepted, 0 when the widget is unknown, -1 on error
 */
int widgetkit_publish_content(const char *widget_id, const char *json);

/**
 * Read the current content tree of every widget as a JSON array.
 * @return allocated JSON string (free with widgetkit_free_string) or NULL
 */
char* widgetkit_content_list(void);

/**
 * Override the settings daemon socket path. Must be called before any other
 * call. Passing NULL restores the default /run/tontoo-settings.sock.
 * @param path socket path or NULL
 */
void widgetkit_set_socket_path(const char *path);

#ifdef __cplusplus
}
#endif

#endif /* WIDGETKIT_H */