                _ => {}
            }
        }
    }

    Ok(None)
}

// =============================================================================
// perry/ui generic dispatch table
// =============================================================================

/// How a perry/ui FFI function expects each argument to be passed.
#[derive(Copy, Clone, Debug)]
enum UiArgKind {
    /// Widget handle: lower the JSValue, unbox the POINTER bits as i64.
    /// Used for the `handle` first arg of every setter, plus child / parent
    /// handle args. The runtime gets the raw 1-based widget handle.
    Widget,
    /// String pointer: lower the JSValue, then call
    /// `js_get_string_pointer_unified` to extract the underlying StringHeader
    /// pointer as i64. Handles both literal strings and runtime-built ones.
    Str,
    /// Raw f64 number. The JSValue is already a NaN-boxed double for numbers,
    /// so we pass it as-is. Used for sizes, colors, weights, alignment ids.
    F64,
    /// Closure handle: lower the JSValue (which is a `js_closure_alloc`
    /// pointer NaN-boxed as POINTER) and pass it as a raw f64. The runtime
    /// extracts the closure pointer via the same NaN-boxing convention.
    Closure,
    /// Raw i64 (rare; some setters take an enum tag as i64).
    I64Raw,
}

/// What the perry/ui FFI function returns and how to box it.
#[derive(Copy, Clone, Debug)]
enum UiReturnKind {
    /// Widget handle: NaN-box the i64 result with POINTER_TAG.
    Widget,
    /// Raw f64: pass through unchanged. Used by `scrollviewGetOffset` etc.
    F64,
    /// Void return: emit `call void` and return the `0.0` sentinel f64.
    Void,
    /// `*mut StringHeader` (i64 ptr) → NaN-box with `STRING_TAG`. Used by
    /// the `perry/i18n` format wrappers (`Currency`, `Percent`, …) so the
    /// returned value reads back as a real string in `console.log`,
    /// template interpolation, and `typeof === "string"` checks.
    Str,
    /// i64 result converted to plain JS number via `sitofp`. Used for integer
    /// counts/IDs that the TS caller should see as a JS number (not a handle).
    I64AsF64,
}

#[derive(Copy, Clone, Debug)]
struct UiSig {
    /// TypeScript method name as it appears in the import (e.g. "Text",
    /// "textSetFontSize"). Matched against `method` from
    /// `lower_native_method_call` for `module == "perry/ui"`.
    method: &'static str,
    /// `perry_ui_*` runtime function symbol. Lazily declared via
    /// `pending_declares` so the linker picks it up from
    /// `libperry_ui_macos.a` (or the equivalent platform-specific lib).
    runtime: &'static str,
    /// Per-argument coercion rules. Length must equal `args.len()` at
    /// the call site, otherwise the dispatch falls through to the
    /// receiver-less early-out (which lowers everything as side effects
    /// and returns 0.0).
    args: &'static [UiArgKind],
    ret: UiReturnKind,
}

/// Static dispatch table for perry/ui receiver-less calls. Covers the
/// constructors + setters mango uses, plus the most common widgets from
/// the cross-cutting "any perry/ui app" surface. Keep alphabetized by
/// `method` for easy scanning.
///
/// Entries NOT in this table fall through to the receiver-less early-out
/// in `lower_native_method_call` (which lowers args for side effects and
/// returns the zero-sentinel). That's the behavior the entire perry/ui
/// surface had pre-v0.5.10 — adding a row here flips one method from
/// "silent no-op" to "real call into libperry_ui_macos.a".
const PERRY_CONTAINER_TABLE: &[UiSig] = &[
    UiSig { method: "run", runtime: "js_container_run",
            args: &[UiArgKind::Str], ret: UiReturnKind::Promise },
    UiSig { method: "create", runtime: "js_container_create",
            args: &[UiArgKind::Str], ret: UiReturnKind::Promise },
    UiSig { method: "start", runtime: "js_container_start",
            args: &[UiArgKind::Str], ret: UiReturnKind::Promise },
    UiSig { method: "stop", runtime: "js_container_stop",
            args: &[UiArgKind::Str, UiArgKind::F64], ret: UiReturnKind::Promise },
    UiSig { method: "remove", runtime: "js_container_remove",
            args: &[UiArgKind::Str, UiArgKind::F64], ret: UiReturnKind::Promise },
    UiSig { method: "list", runtime: "js_container_list",
            args: &[UiArgKind::F64], ret: UiReturnKind::Promise },
    UiSig { method: "inspect", runtime: "js_container_inspect",
            args: &[UiArgKind::Str], ret: UiReturnKind::Promise },
    UiSig { method: "logs", runtime: "js_container_logs",
            args: &[UiArgKind::Str, UiArgKind::F64], ret: UiReturnKind::Promise },
    UiSig { method: "exec", runtime: "js_container_exec",
            args: &[UiArgKind::Str, UiArgKind::Str, UiArgKind::Str, UiArgKind::Str],
            ret: UiReturnKind::Promise },
    UiSig { method: "pullImage", runtime: "js_container_pullImage",
            args: &[UiArgKind::Str], ret: UiReturnKind::Promise },
    UiSig { method: "listImages", runtime: "js_container_listImages",
            args: &[], ret: UiReturnKind::Promise },
    UiSig { method: "removeImage", runtime: "js_container_removeImage",
            args: &[UiArgKind::Str, UiArgKind::F64], ret: UiReturnKind::Promise },
    UiSig { method: "getBackend", runtime: "js_container_getBackend",
            args: &[], ret: UiReturnKind::Str },
    UiSig { method: "detectBackend", runtime: "js_container_detectBackend",
            args: &[], ret: UiReturnKind::Promise },
    UiSig { method: "build", runtime: "js_container_build",
            args: &[UiArgKind::Str, UiArgKind::Str], ret: UiReturnKind::Promise },
    UiSig { method: "composeUp", runtime: "js_container_composeUp",
            args: &[UiArgKind::Str], ret: UiReturnKind::Promise },
];

const PERRY_COMPOSE_TABLE: &[UiSig] = &[
    UiSig { method: "up", runtime: "js_compose_up",
            args: &[UiArgKind::Str], ret: UiReturnKind::Promise },
    UiSig { method: "down", runtime: "js_compose_down",
            args: &[UiArgKind::F64, UiArgKind::F64], ret: UiReturnKind::Promise },
    UiSig { method: "ps", runtime: "js_compose_ps",
            args: &[UiArgKind::F64], ret: UiReturnKind::Promise },
    UiSig { method: "logs", runtime: "js_compose_logs",
            args: &[UiArgKind::F64, UiArgKind::Str, UiArgKind::F64], ret: UiReturnKind::Promise },
    UiSig { method: "exec", runtime: "js_compose_exec",
            args: &[UiArgKind::F64, UiArgKind::Str, UiArgKind::Str, UiArgKind::Str],
            ret: UiReturnKind::Promise },
    UiSig { method: "config", runtime: "js_compose_config",
            args: &[UiArgKind::F64], ret: UiReturnKind::Promise },
    UiSig { method: "start", runtime: "js_compose_start",
            args: &[UiArgKind::F64, UiArgKind::Str], ret: UiReturnKind::Promise },
    UiSig { method: "stop", runtime: "js_compose_stop",
            args: &[UiArgKind::F64, UiArgKind::Str], ret: UiReturnKind::Promise },
    UiSig { method: "restart", runtime: "js_compose_restart",
            args: &[UiArgKind::F64, UiArgKind::Str], ret: UiReturnKind::Promise },
];

const PERRY_WORKLOADS_TABLE: &[UiSig] = &[
    UiSig { method: "graph", runtime: "js_workload_graph",
            args: &[UiArgKind::Str, UiArgKind::Str], ret: UiReturnKind::Str },
    UiSig { method: "runGraph", runtime: "js_workload_runGraph",
            args: &[UiArgKind::Str, UiArgKind::Str], ret: UiReturnKind::Promise },
];

const PERRY_CONTAINER_TABLE: &[UiSig] = &[
    UiSig { method: "run", runtime: "js_container_run",
            args: &[UiArgKind::Str], ret: UiReturnKind::Promise },
    UiSig { method: "create", runtime: "js_container_create",
            args: &[UiArgKind::Str], ret: UiReturnKind::Promise },
    UiSig { method: "start", runtime: "js_container_start",
            args: &[UiArgKind::Str], ret: UiReturnKind::Promise },
    UiSig { method: "stop", runtime: "js_container_stop",
            args: &[UiArgKind::Str, UiArgKind::F64], ret: UiReturnKind::Promise },
    UiSig { method: "remove", runtime: "js_container_remove",
            args: &[UiArgKind::Str, UiArgKind::F64], ret: UiReturnKind::Promise },
    UiSig { method: "list", runtime: "js_container_list",
            args: &[UiArgKind::F64], ret: UiReturnKind::Promise },
    UiSig { method: "inspect", runtime: "js_container_inspect",
            args: &[UiArgKind::Str], ret: UiReturnKind::Promise },
    UiSig { method: "logs", runtime: "js_container_logs",
            args: &[UiArgKind::Str, UiArgKind::F64], ret: UiReturnKind::Promise },
    UiSig { method: "exec", runtime: "js_container_exec",
            args: &[UiArgKind::Str, UiArgKind::Str, UiArgKind::Str, UiArgKind::Str],
            ret: UiReturnKind::Promise },
    UiSig { method: "pullImage", runtime: "js_container_pullImage",
            args: &[UiArgKind::Str], ret: UiReturnKind::Promise },
    UiSig { method: "listImages", runtime: "js_container_listImages",
            args: &[], ret: UiReturnKind::Promise },
    UiSig { method: "removeImage", runtime: "js_container_removeImage",
            args: &[UiArgKind::Str, UiArgKind::F64], ret: UiReturnKind::Promise },
    UiSig { method: "getBackend", runtime: "js_container_getBackend",
            args: &[], ret: UiReturnKind::Str },
    UiSig { method: "detectBackend", runtime: "js_container_detectBackend",
            args: &[], ret: UiReturnKind::Promise },
    UiSig { method: "build", runtime: "js_container_build",
            args: &[UiArgKind::Str, UiArgKind::Str], ret: UiReturnKind::Promise },
    UiSig { method: "composeUp", runtime: "js_container_composeUp",
            args: &[UiArgKind::Str], ret: UiReturnKind::Promise },
];

const PERRY_COMPOSE_TABLE: &[UiSig] = &[
    UiSig { method: "up", runtime: "js_compose_up",
            args: &[UiArgKind::Str], ret: UiReturnKind::Promise },
    UiSig { method: "down", runtime: "js_compose_down",
            args: &[UiArgKind::F64, UiArgKind::F64], ret: UiReturnKind::Promise },
    UiSig { method: "ps", runtime: "js_compose_ps",
            args: &[UiArgKind::F64], ret: UiReturnKind::Promise },
    UiSig { method: "logs", runtime: "js_compose_logs",
            args: &[UiArgKind::F64, UiArgKind::Str, UiArgKind::F64], ret: UiReturnKind::Promise },
    UiSig { method: "exec", runtime: "js_compose_exec",
            args: &[UiArgKind::F64, UiArgKind::Str, UiArgKind::Str, UiArgKind::Str],
            ret: UiReturnKind::Promise },
    UiSig { method: "config", runtime: "js_compose_config",
            args: &[UiArgKind::F64], ret: UiReturnKind::Promise },
    UiSig { method: "start", runtime: "js_compose_start",
            args: &[UiArgKind::F64, UiArgKind::Str], ret: UiReturnKind::Promise },
    UiSig { method: "stop", runtime: "js_compose_stop",
            args: &[UiArgKind::F64, UiArgKind::Str], ret: UiReturnKind::Promise },
    UiSig { method: "restart", runtime: "js_compose_restart",
            args: &[UiArgKind::F64, UiArgKind::Str], ret: UiReturnKind::Promise },
];

const PERRY_WORKLOADS_TABLE: &[UiSig] = &[
    UiSig { method: "graph", runtime: "js_workload_graph",
            args: &[UiArgKind::Str, UiArgKind::Str], ret: UiReturnKind::Str },
    UiSig { method: "runGraph", runtime: "js_workload_runGraph",
            args: &[UiArgKind::Str, UiArgKind::Str], ret: UiReturnKind::Promise },
];

const PERRY_UI_TABLE: &[UiSig] = &[
    // ---- Constructors (return widget handle) ----
    UiSig { method: "Divider", runtime: "perry_ui_divider_create",
            args: &[], ret: UiReturnKind::Widget },
    UiSig { method: "ScrollView", runtime: "perry_ui_scrollview_create",
            args: &[], ret: UiReturnKind::Widget },
    UiSig { method: "Spacer", runtime: "perry_ui_spacer_create",
            args: &[], ret: UiReturnKind::Widget },
    UiSig { method: "Text", runtime: "perry_ui_text_create",
            args: &[UiArgKind::Str], ret: UiReturnKind::Widget },
    UiSig { method: "TextArea", runtime: "perry_ui_textarea_create",
            args: &[UiArgKind::Str, UiArgKind::Closure], ret: UiReturnKind::Widget },
    UiSig { method: "TextField", runtime: "perry_ui_textfield_create",
            args: &[UiArgKind::Str, UiArgKind::Closure], ret: UiReturnKind::Widget },

    // ---- Menu / menu bar ----
    UiSig { method: "menuAddItem", runtime: "perry_ui_menu_add_item",
            args: &[UiArgKind::Widget, UiArgKind::Str, UiArgKind::Closure],
            ret: UiReturnKind::Void },
    UiSig { method: "menuAddSeparator", runtime: "perry_ui_menu_add_separator",
            args: &[UiArgKind::Widget], ret: UiReturnKind::Void },
    UiSig { method: "menuAddStandardAction", runtime: "perry_ui_menu_add_standard_action",
            args: &[UiArgKind::Widget, UiArgKind::Str, UiArgKind::Str, UiArgKind::Str],
            ret: UiReturnKind::Void },
    UiSig { method: "menuBarAddMenu", runtime: "perry_ui_menubar_add_menu",
            args: &[UiArgKind::Widget, UiArgKind::Str, UiArgKind::Widget],
            ret: UiReturnKind::Void },
    UiSig { method: "menuBarAttach", runtime: "perry_ui_menubar_attach",
            args: &[UiArgKind::Widget], ret: UiReturnKind::Void },
    UiSig { method: "menuBarCreate", runtime: "perry_ui_menubar_create",
            args: &[], ret: UiReturnKind::Widget },
    UiSig { method: "menuCreate", runtime: "perry_ui_menu_create",
            args: &[], ret: UiReturnKind::Widget },

    // ---- ScrollView ----
    UiSig { method: "scrollviewSetChild", runtime: "perry_ui_scrollview_set_child",
            args: &[UiArgKind::Widget, UiArgKind::Widget], ret: UiReturnKind::Void },
    UiSig { method: "scrollViewSetChild", runtime: "perry_ui_scrollview_set_child",
            args: &[UiArgKind::Widget, UiArgKind::Widget], ret: UiReturnKind::Void },
    UiSig { method: "scrollViewGetOffset", runtime: "perry_ui_scrollview_get_offset",
            args: &[UiArgKind::Widget], ret: UiReturnKind::F64 },
    UiSig { method: "scrollViewSetOffset", runtime: "perry_ui_scrollview_set_offset",
            args: &[UiArgKind::Widget, UiArgKind::F64, UiArgKind::F64], ret: UiReturnKind::Void },
    UiSig { method: "scrollViewScrollTo", runtime: "perry_ui_scrollview_scroll_to",
            args: &[UiArgKind::Widget, UiArgKind::F64, UiArgKind::F64], ret: UiReturnKind::Void },

    // ---- Stack layout ----
    UiSig { method: "stackSetAlignment", runtime: "perry_ui_stack_set_alignment",
            args: &[UiArgKind::Widget, UiArgKind::F64], ret: UiReturnKind::Void },
    UiSig { method: "stackSetDistribution", runtime: "perry_ui_stack_set_distribution",
            args: &[UiArgKind::Widget, UiArgKind::F64], ret: UiReturnKind::Void },

    // ---- Text setters ----
    UiSig { method: "textSetColor", runtime: "perry_ui_text_set_color",
            args: &[UiArgKind::Widget, UiArgKind::F64, UiArgKind::F64, UiArgKind::F64, UiArgKind::F64],
            ret: UiReturnKind::Void },
    UiSig { method: "textSetFontFamily", runtime: "perry_ui_text_set_font_family",
            args: &[UiArgKind::Widget, UiArgKind::Str], ret: UiReturnKind::Void },
    UiSig { method: "textSetFontSize", runtime: "perry_ui_text_set_font_size",
            args: &[UiArgKind::Widget, UiArgKind::F64], ret: UiReturnKind::Void },
    UiSig { method: "textSetFontWeight", runtime: "perry_ui_text_set_font_weight",
            args: &[UiArgKind::Widget, UiArgKind::F64, UiArgKind::F64], ret: UiReturnKind::Void },
    UiSig { method: "textSetString", runtime: "perry_ui_text_set_string",
            args: &[UiArgKind::Widget, UiArgKind::Str], ret: UiReturnKind::Void },
    UiSig { method: "textSetWraps", runtime: "perry_ui_text_set_wraps",
            args: &[UiArgKind::Widget, UiArgKind::F64], ret: UiReturnKind::Void },

    // ---- Button setters ----
    UiSig { method: "buttonSetBordered", runtime: "perry_ui_button_set_bordered",
            args: &[UiArgKind::Widget, UiArgKind::F64], ret: UiReturnKind::Void },
    UiSig { method: "buttonSetTextColor", runtime: "perry_ui_button_set_text_color",
            args: &[UiArgKind::Widget, UiArgKind::F64, UiArgKind::F64, UiArgKind::F64, UiArgKind::F64],
            ret: UiReturnKind::Void },
    UiSig { method: "buttonSetTitle", runtime: "perry_ui_button_set_title",
            args: &[UiArgKind::Widget, UiArgKind::Str], ret: UiReturnKind::Void },

    // ---- TextField / TextArea ----
    UiSig { method: "textfieldSetString", runtime: "perry_ui_textfield_set_string",
            args: &[UiArgKind::Widget, UiArgKind::Str], ret: UiReturnKind::Void },
    UiSig { method: "textareaSetString", runtime: "perry_ui_textarea_set_string",
            args: &[UiArgKind::Widget, UiArgKind::Str], ret: UiReturnKind::Void },

    // ---- Generic widget ops ----
    UiSig { method: "setCornerRadius", runtime: "perry_ui_widget_set_corner_radius",
            args: &[UiArgKind::Widget, UiArgKind::F64], ret: UiReturnKind::Void },
    UiSig { method: "widgetAddChild", runtime: "perry_ui_widget_add_child",
            args: &[UiArgKind::Widget, UiArgKind::Widget], ret: UiReturnKind::Void },
    UiSig { method: "widgetClearChildren", runtime: "perry_ui_widget_clear_children",
            args: &[UiArgKind::Widget], ret: UiReturnKind::Void },
    UiSig { method: "widgetMatchParentHeight", runtime: "perry_ui_widget_match_parent_height",
            args: &[UiArgKind::Widget], ret: UiReturnKind::Void },
    UiSig { method: "widgetMatchParentWidth", runtime: "perry_ui_widget_match_parent_width",
            args: &[UiArgKind::Widget], ret: UiReturnKind::Void },
    UiSig { method: "widgetSetBackgroundColor", runtime: "perry_ui_widget_set_background_color",
            args: &[UiArgKind::Widget, UiArgKind::F64, UiArgKind::F64, UiArgKind::F64, UiArgKind::F64],
            ret: UiReturnKind::Void },
    UiSig { method: "widgetSetBackgroundGradient", runtime: "perry_ui_widget_set_background_gradient",
            args: &[
                UiArgKind::Widget, UiArgKind::F64, UiArgKind::F64, UiArgKind::F64, UiArgKind::F64,
                UiArgKind::F64, UiArgKind::F64, UiArgKind::F64, UiArgKind::F64, UiArgKind::F64,
            ], ret: UiReturnKind::Void },
    UiSig { method: "widgetSetHeight", runtime: "perry_ui_widget_set_height",
            args: &[UiArgKind::Widget, UiArgKind::F64], ret: UiReturnKind::Void },
    UiSig { method: "widgetSetHidden", runtime: "perry_ui_set_widget_hidden",
            args: &[UiArgKind::Widget, UiArgKind::I64Raw], ret: UiReturnKind::Void },
    UiSig { method: "widgetSetHugging", runtime: "perry_ui_widget_set_hugging",
            args: &[UiArgKind::Widget, UiArgKind::F64], ret: UiReturnKind::Void },
    UiSig { method: "widgetSetWidth", runtime: "perry_ui_widget_set_width",
            args: &[UiArgKind::Widget, UiArgKind::F64], ret: UiReturnKind::Void },

    // ---- Image ----
    UiSig { method: "ImageFile", runtime: "perry_ui_image_create_file",
            args: &[UiArgKind::Str], ret: UiReturnKind::Widget },
    UiSig { method: "ImageSymbol", runtime: "perry_ui_image_create_symbol",
            args: &[UiArgKind::Str], ret: UiReturnKind::Widget },
    UiSig { method: "imageSetSize", runtime: "perry_ui_image_set_size",
            args: &[UiArgKind::Widget, UiArgKind::F64, UiArgKind::F64], ret: UiReturnKind::Void },
    UiSig { method: "imageSetTint", runtime: "perry_ui_image_set_tint",
            args: &[UiArgKind::Widget, UiArgKind::F64, UiArgKind::F64, UiArgKind::F64, UiArgKind::F64],
            ret: UiReturnKind::Void },

    // ---- Padding / Edge Insets ----
    UiSig { method: "setPadding", runtime: "perry_ui_widget_set_edge_insets",
            args: &[UiArgKind::Widget, UiArgKind::F64, UiArgKind::F64, UiArgKind::F64, UiArgKind::F64],
            ret: UiReturnKind::Void },
    UiSig { method: "widgetSetEdgeInsets", runtime: "perry_ui_widget_set_edge_insets",
            args: &[UiArgKind::Widget, UiArgKind::F64, UiArgKind::F64, UiArgKind::F64, UiArgKind::F64],
            ret: UiReturnKind::Void },

    // ---- LazyVStack (virtualized list) ----
    // `LazyVStack(count, (i) => Widget)` — on macOS backed by NSTableView
    // with lazy row rendering. The render closure is invoked only for rows
    // currently in the visible rect.
    UiSig { method: "LazyVStack", runtime: "perry_ui_lazyvstack_create",
            args: &[UiArgKind::F64, UiArgKind::Closure], ret: UiReturnKind::Widget },
    UiSig { method: "lazyvstackUpdate", runtime: "perry_ui_lazyvstack_update",
            args: &[UiArgKind::Widget, UiArgKind::I64Raw], ret: UiReturnKind::Void },
    UiSig { method: "lazyvstackSetRowHeight", runtime: "perry_ui_lazyvstack_set_row_height",
            args: &[UiArgKind::Widget, UiArgKind::F64], ret: UiReturnKind::Void },

    // ---- State ----
    UiSig { method: "State", runtime: "perry_ui_state_create",
            args: &[UiArgKind::F64], ret: UiReturnKind::Widget },
    UiSig { method: "stateCreate", runtime: "perry_ui_state_create",
            args: &[UiArgKind::F64], ret: UiReturnKind::Widget },
    UiSig { method: "stateGet", runtime: "perry_ui_state_get",
            args: &[UiArgKind::Widget], ret: UiReturnKind::F64 },
    UiSig { method: "stateSet", runtime: "perry_ui_state_set",
            args: &[UiArgKind::Widget, UiArgKind::F64], ret: UiReturnKind::Void },
    UiSig { method: "stateOnChange", runtime: "perry_ui_state_on_change",
            args: &[UiArgKind::Widget, UiArgKind::Closure], ret: UiReturnKind::Void },
    UiSig { method: "stateBindTextNumeric", runtime: "perry_ui_state_bind_text_numeric",
            args: &[UiArgKind::Widget, UiArgKind::Widget, UiArgKind::Str, UiArgKind::Str],
            ret: UiReturnKind::Void },
    UiSig { method: "stateBindSlider", runtime: "perry_ui_state_bind_slider",
            args: &[UiArgKind::Widget, UiArgKind::Widget], ret: UiReturnKind::Void },
    UiSig { method: "stateBindToggle", runtime: "perry_ui_state_bind_toggle",
            args: &[UiArgKind::Widget, UiArgKind::Widget], ret: UiReturnKind::Void },
    UiSig { method: "stateBindVisibility", runtime: "perry_ui_state_bind_visibility",
            args: &[UiArgKind::Widget, UiArgKind::Widget, UiArgKind::Widget],
            ret: UiReturnKind::Void },
    UiSig { method: "stateBindTextfield", runtime: "perry_ui_state_bind_textfield",
            args: &[UiArgKind::Widget, UiArgKind::Widget], ret: UiReturnKind::Void },

    // ---- TextField extras ----
    UiSig { method: "textfieldGetString", runtime: "perry_ui_textfield_get_string",
            args: &[UiArgKind::Widget], ret: UiReturnKind::F64 },
    UiSig { method: "textfieldFocus", runtime: "perry_ui_textfield_focus",
            args: &[UiArgKind::Widget], ret: UiReturnKind::Void },
    UiSig { method: "textfieldBlurAll", runtime: "perry_ui_textfield_blur_all",
            args: &[], ret: UiReturnKind::Void },
    UiSig { method: "textfieldSetNextKeyView", runtime: "perry_ui_textfield_set_next_key_view",
            args: &[UiArgKind::Widget, UiArgKind::Widget], ret: UiReturnKind::Void },
    UiSig { method: "textfieldSetOnSubmit", runtime: "perry_ui_textfield_set_on_submit",
            args: &[UiArgKind::Widget, UiArgKind::Closure], ret: UiReturnKind::Void },
    UiSig { method: "textfieldSetOnFocus", runtime: "perry_ui_textfield_set_on_focus",
            args: &[UiArgKind::Widget, UiArgKind::Closure], ret: UiReturnKind::Void },
    UiSig { method: "textfieldSetBackgroundColor", runtime: "perry_ui_textfield_set_background_color",
            args: &[UiArgKind::Widget, UiArgKind::F64, UiArgKind::F64, UiArgKind::F64, UiArgKind::F64],
            ret: UiReturnKind::Void },
    UiSig { method: "textfieldSetBorderless", runtime: "perry_ui_textfield_set_borderless",
            args: &[UiArgKind::Widget, UiArgKind::F64], ret: UiReturnKind::Void },
    UiSig { method: "textfieldSetFontSize", runtime: "perry_ui_textfield_set_font_size",
            args: &[UiArgKind::Widget, UiArgKind::F64], ret: UiReturnKind::Void },
    UiSig { method: "textfieldSetTextColor", runtime: "perry_ui_textfield_set_text_color",
            args: &[UiArgKind::Widget, UiArgKind::F64, UiArgKind::F64, UiArgKind::F64, UiArgKind::F64],
            ret: UiReturnKind::Void },
    UiSig { method: "textareaGetString", runtime: "perry_ui_textarea_get_string",
            args: &[UiArgKind::Widget], ret: UiReturnKind::F64 },

    // ---- Text extras ----
    UiSig { method: "textSetSelectable", runtime: "perry_ui_text_set_selectable",
            args: &[UiArgKind::Widget, UiArgKind::F64], ret: UiReturnKind::Void },
    // Text decoration (issue #185 Phase B): 0=none, 1=underline,
    // 2=strikethrough. Wired on every backend (Apple via
    // NSAttributedString, Android via Paint flags, GTK4 via Pango
    // attributes, Web via CSS `text-decoration`, watchOS via tree
    // metadata + SwiftUI host modifier). Windows is stub-with-state.
    UiSig { method: "textSetDecoration", runtime: "perry_ui_text_set_decoration",
            args: &[UiArgKind::Widget, UiArgKind::I64Raw], ret: UiReturnKind::Void },

    // ---- Widget extras ----
    UiSig { method: "widgetAddChildAt", runtime: "perry_ui_widget_add_child_at",
            args: &[UiArgKind::Widget, UiArgKind::Widget, UiArgKind::I64Raw],
            ret: UiReturnKind::Void },
    UiSig { method: "widgetRemoveChild", runtime: "perry_ui_widget_remove_child",
            args: &[UiArgKind::Widget, UiArgKind::Widget], ret: UiReturnKind::Void },
    UiSig { method: "widgetReorderChild", runtime: "perry_ui_widget_reorder_child",
            args: &[UiArgKind::Widget, UiArgKind::I64Raw, UiArgKind::I64Raw],
            ret: UiReturnKind::Void },
    UiSig { method: "widgetSetOpacity", runtime: "perry_ui_widget_set_opacity",
            args: &[UiArgKind::Widget, UiArgKind::F64], ret: UiReturnKind::Void },
    UiSig { method: "widgetSetEnabled", runtime: "perry_ui_widget_set_enabled",
            args: &[UiArgKind::Widget, UiArgKind::I64Raw], ret: UiReturnKind::Void },
    UiSig { method: "widgetSetTooltip", runtime: "perry_ui_widget_set_tooltip",
            args: &[UiArgKind::Widget, UiArgKind::Str], ret: UiReturnKind::Void },
    UiSig { method: "widgetSetControlSize", runtime: "perry_ui_widget_set_control_size",
            args: &[UiArgKind::Widget, UiArgKind::I64Raw], ret: UiReturnKind::Void },
    UiSig { method: "widgetSetOnClick", runtime: "perry_ui_widget_set_on_click",
            args: &[UiArgKind::Widget, UiArgKind::Closure], ret: UiReturnKind::Void },
    UiSig { method: "widgetSetOnHover", runtime: "perry_ui_widget_set_on_hover",
            args: &[UiArgKind::Widget, UiArgKind::Closure], ret: UiReturnKind::Void },
    UiSig { method: "widgetSetOnDoubleClick", runtime: "perry_ui_widget_set_on_double_click",
            args: &[UiArgKind::Widget, UiArgKind::Closure], ret: UiReturnKind::Void },
    UiSig { method: "widgetAnimateOpacity", runtime: "perry_ui_widget_animate_opacity",
            args: &[UiArgKind::Widget, UiArgKind::F64, UiArgKind::F64], ret: UiReturnKind::Void },
    UiSig { method: "widgetAnimatePosition", runtime: "perry_ui_widget_animate_position",
            args: &[UiArgKind::Widget, UiArgKind::F64, UiArgKind::F64, UiArgKind::F64],
            ret: UiReturnKind::Void },
    UiSig { method: "widgetAddOverlay", runtime: "perry_ui_widget_add_overlay",
            args: &[UiArgKind::Widget, UiArgKind::Widget], ret: UiReturnKind::Void },
    UiSig { method: "widgetSetBorderColor", runtime: "perry_ui_widget_set_border_color",
            args: &[UiArgKind::Widget, UiArgKind::F64, UiArgKind::F64, UiArgKind::F64, UiArgKind::F64],
            ret: UiReturnKind::Void },
    UiSig { method: "widgetSetBorderWidth", runtime: "perry_ui_widget_set_border_width",
            args: &[UiArgKind::Widget, UiArgKind::F64], ret: UiReturnKind::Void },
    // Drop shadow setter (issue #185 Phase B). Args: handle, r,g,b,a (color
    // 0-1; alpha lands in shadowOpacity), blur, offset_x, offset_y. Wired
    // on every Apple platform; Phase B closures will add Android (elevation),
    // GTK4 (CSS box-shadow), Web (CSS), Windows (DirectComposition).
    UiSig { method: "widgetSetShadow", runtime: "perry_ui_widget_set_shadow",
            args: &[
                UiArgKind::Widget,
                UiArgKind::F64, UiArgKind::F64, UiArgKind::F64, UiArgKind::F64,
                UiArgKind::F64, UiArgKind::F64, UiArgKind::F64,
            ],
            ret: UiReturnKind::Void },
    UiSig { method: "widgetSetContextMenu", runtime: "perry_ui_widget_set_context_menu",
            args: &[UiArgKind::Widget, UiArgKind::Widget], ret: UiReturnKind::Void },
    UiSig { method: "stackSetDetachesHidden", runtime: "perry_ui_stack_set_detaches_hidden",
            args: &[UiArgKind::Widget, UiArgKind::F64], ret: UiReturnKind::Void },

    // ---- Additional constructors ----
    UiSig { method: "Toggle", runtime: "perry_ui_toggle_create",
            args: &[UiArgKind::Str, UiArgKind::Closure], ret: UiReturnKind::Widget },
    UiSig { method: "Slider", runtime: "perry_ui_slider_create",
            args: &[UiArgKind::F64, UiArgKind::F64, UiArgKind::Closure], ret: UiReturnKind::Widget },
    UiSig { method: "SecureField", runtime: "perry_ui_securefield_create",
            args: &[UiArgKind::Str, UiArgKind::Closure], ret: UiReturnKind::Widget },
    UiSig { method: "ProgressView", runtime: "perry_ui_progressview_create",
            args: &[], ret: UiReturnKind::Widget },
    UiSig { method: "ZStack", runtime: "perry_ui_zstack_create",
            args: &[], ret: UiReturnKind::Widget },
    UiSig { method: "Section", runtime: "perry_ui_section_create",
            args: &[UiArgKind::Str], ret: UiReturnKind::Widget },

    // ---- ProgressView ----
    UiSig { method: "progressviewSetValue", runtime: "perry_ui_progressview_set_value",
            args: &[UiArgKind::Widget, UiArgKind::F64], ret: UiReturnKind::Void },

    // ---- Picker ----
    UiSig { method: "Picker", runtime: "perry_ui_picker_create",
            args: &[UiArgKind::Closure], ret: UiReturnKind::Widget },
    UiSig { method: "pickerAddItem", runtime: "perry_ui_picker_add_item",
            args: &[UiArgKind::Widget, UiArgKind::Str], ret: UiReturnKind::Void },
    UiSig { method: "pickerGetSelected", runtime: "perry_ui_picker_get_selected",
            args: &[UiArgKind::Widget], ret: UiReturnKind::F64 },
    UiSig { method: "pickerSetSelected", runtime: "perry_ui_picker_set_selected",
            args: &[UiArgKind::Widget, UiArgKind::I64Raw], ret: UiReturnKind::Void },

    // ---- NavigationStack ----
    UiSig { method: "NavStack", runtime: "perry_ui_navstack_create",
            args: &[], ret: UiReturnKind::Widget },
    UiSig { method: "navstackPush", runtime: "perry_ui_navstack_push",
            args: &[UiArgKind::Widget, UiArgKind::Widget, UiArgKind::Str], ret: UiReturnKind::Void },
    UiSig { method: "navstackPop", runtime: "perry_ui_navstack_pop",
            args: &[UiArgKind::Widget], ret: UiReturnKind::Void },

    // ---- TabBar ----
    UiSig { method: "TabBar", runtime: "perry_ui_tabbar_create",
            args: &[UiArgKind::Closure], ret: UiReturnKind::Widget },
    UiSig { method: "tabbarAddTab", runtime: "perry_ui_tabbar_add_tab",
            args: &[UiArgKind::Widget, UiArgKind::Str, UiArgKind::Widget], ret: UiReturnKind::Void },
    UiSig { method: "tabbarSetSelected", runtime: "perry_ui_tabbar_set_selected",
            args: &[UiArgKind::Widget, UiArgKind::I64Raw], ret: UiReturnKind::Void },

    // ---- Menu extras ----
    UiSig { method: "menuAddSubmenu", runtime: "perry_ui_menu_add_submenu",
            args: &[UiArgKind::Widget, UiArgKind::Str, UiArgKind::Widget],
            ret: UiReturnKind::Void },
    UiSig { method: "menuClear", runtime: "perry_ui_menu_clear",
            args: &[UiArgKind::Widget], ret: UiReturnKind::Void },
    UiSig { method: "menuAddItemWithShortcut", runtime: "perry_ui_menu_add_item_with_shortcut",
            args: &[UiArgKind::Widget, UiArgKind::Str, UiArgKind::Str, UiArgKind::Closure],
            ret: UiReturnKind::Void },

    // ---- ScrollView extras ----
    UiSig { method: "scrollViewSetOffset", runtime: "perry_ui_scrollview_set_offset",
            args: &[UiArgKind::Widget, UiArgKind::F64, UiArgKind::F64], ret: UiReturnKind::Void },
    UiSig { method: "scrollViewScrollTo", runtime: "perry_ui_scrollview_scroll_to",
            args: &[UiArgKind::Widget, UiArgKind::F64, UiArgKind::F64], ret: UiReturnKind::Void },

    // ---- Button extras ----
    UiSig { method: "buttonSetContentTintColor", runtime: "perry_ui_button_set_content_tint_color",
            args: &[UiArgKind::Widget, UiArgKind::F64, UiArgKind::F64, UiArgKind::F64, UiArgKind::F64],
            ret: UiReturnKind::Void },
    UiSig { method: "buttonSetImage", runtime: "perry_ui_button_set_image",
            args: &[UiArgKind::Widget, UiArgKind::Str], ret: UiReturnKind::Void },
    UiSig { method: "buttonSetImagePosition", runtime: "perry_ui_button_set_image_position",
            args: &[UiArgKind::Widget, UiArgKind::I64Raw], ret: UiReturnKind::Void },

    // ---- Clipboard ----
    UiSig { method: "clipboardRead", runtime: "perry_ui_clipboard_read",
            args: &[], ret: UiReturnKind::F64 },
    UiSig { method: "clipboardWrite", runtime: "perry_ui_clipboard_write",
            args: &[UiArgKind::Str], ret: UiReturnKind::Void },

    // ---- Alert ----
    // `alert(title, message)` dispatches to a dedicated 2-arg FFI; the prior
    // entry pointed at the 4-arg `perry_ui_alert` symbol, which was ABI-broken
    // (buttons/callback read from uninitialized registers, usually segfaulting
    // inside js_array_get_length).
    UiSig { method: "alert", runtime: "perry_ui_alert_simple",
            args: &[UiArgKind::Str, UiArgKind::Str], ret: UiReturnKind::Void },
    // `alertWithButtons(title, message, buttons, cb)` — buttons is a JS array
    // of labels, callback receives the 0-based button index. Passed as F64
    // because the runtime extracts the array pointer via
    // `js_nanbox_get_pointer` just like closures.
    UiSig { method: "alertWithButtons", runtime: "perry_ui_alert",
            args: &[UiArgKind::Str, UiArgKind::Str, UiArgKind::F64, UiArgKind::Closure],
            ret: UiReturnKind::Void },

    // ---- Window (constructor — receiver-less) ----
    UiSig { method: "Window", runtime: "perry_ui_window_create",
            args: &[UiArgKind::Str, UiArgKind::F64, UiArgKind::F64], ret: UiReturnKind::Widget },

    // ---- VStack/HStack with built-in insets (no children array — children added via widgetAddChild) ----
    UiSig { method: "VStackWithInsets", runtime: "perry_ui_vstack_create_with_insets",
            args: &[UiArgKind::F64, UiArgKind::F64, UiArgKind::F64, UiArgKind::F64, UiArgKind::F64],
            ret: UiReturnKind::Widget },
    UiSig { method: "HStackWithInsets", runtime: "perry_ui_hstack_create_with_insets",
            args: &[UiArgKind::F64, UiArgKind::F64, UiArgKind::F64, UiArgKind::F64, UiArgKind::F64],
            ret: UiReturnKind::Widget },

    // ---- Embed external NSView ----
    UiSig { method: "embedNSView", runtime: "perry_ui_embed_nsview",
            args: &[UiArgKind::I64Raw], ret: UiReturnKind::Widget },

    // ---- File dialogs ----
    UiSig { method: "openFileDialog", runtime: "perry_ui_open_file_dialog",
            args: &[UiArgKind::Closure], ret: UiReturnKind::Void },
    UiSig { method: "openFolderDialog", runtime: "perry_ui_open_folder_dialog",
            args: &[UiArgKind::Closure], ret: UiReturnKind::Void },
    UiSig { method: "saveFileDialog", runtime: "perry_ui_save_file_dialog",
            args: &[UiArgKind::Closure, UiArgKind::Str, UiArgKind::Str],
            ret: UiReturnKind::Void },

    // ---- Widget overlay frame ----
    UiSig { method: "widgetSetOverlayFrame", runtime: "perry_ui_widget_set_overlay_frame",
            args: &[UiArgKind::Widget, UiArgKind::F64, UiArgKind::F64, UiArgKind::F64, UiArgKind::F64],
            ret: UiReturnKind::Void },

    // ---- Toolbar ----
    UiSig { method: "toolbarCreate", runtime: "perry_ui_toolbar_create",
            args: &[], ret: UiReturnKind::Widget },
    UiSig { method: "toolbarAddItem", runtime: "perry_ui_toolbar_add_item",
            args: &[UiArgKind::Widget, UiArgKind::Str, UiArgKind::Str, UiArgKind::Closure],
            ret: UiReturnKind::Void },
    UiSig { method: "toolbarAttach", runtime: "perry_ui_toolbar_attach",
            args: &[UiArgKind::Widget, UiArgKind::Widget], ret: UiReturnKind::Void },

    // ---- SplitView ----
    UiSig { method: "SplitView", runtime: "perry_ui_splitview_create",
            args: &[], ret: UiReturnKind::Widget },
    UiSig { method: "splitViewAddChild", runtime: "perry_ui_splitview_add_child",
            args: &[UiArgKind::Widget, UiArgKind::Widget], ret: UiReturnKind::Void },

    // ---- Sheet ----
    UiSig { method: "sheetCreate", runtime: "perry_ui_sheet_create",
            args: &[UiArgKind::Widget, UiArgKind::F64, UiArgKind::F64], ret: UiReturnKind::Widget },
    UiSig { method: "sheetPresent", runtime: "perry_ui_sheet_present",
            args: &[UiArgKind::Widget], ret: UiReturnKind::Void },
    UiSig { method: "sheetDismiss", runtime: "perry_ui_sheet_dismiss",
            args: &[UiArgKind::Widget], ret: UiReturnKind::Void },

    // ---- FrameSplit (NSSplitView wrapper) ----
    UiSig { method: "frameSplitCreate", runtime: "perry_ui_frame_split_create",
            args: &[UiArgKind::F64], ret: UiReturnKind::Widget },
    UiSig { method: "frameSplitAddChild", runtime: "perry_ui_frame_split_add_child",
            args: &[UiArgKind::Widget, UiArgKind::Widget], ret: UiReturnKind::Void },

    // ---- File dialog polling ----
    UiSig { method: "pollOpenFile", runtime: "perry_ui_poll_open_file",
            args: &[], ret: UiReturnKind::F64 },

    // ---- Keyboard shortcuts ----
    // `modifiers` is a bitfield: 1=Cmd, 2=Shift, 4=Option, 8=Control.
    UiSig { method: "addKeyboardShortcut", runtime: "perry_ui_add_keyboard_shortcut",
            args: &[UiArgKind::Str, UiArgKind::F64, UiArgKind::Closure], ret: UiReturnKind::Void },

    // ---- App lifecycle hooks ----
    UiSig { method: "onTerminate", runtime: "perry_ui_app_on_terminate",
            args: &[UiArgKind::Closure], ret: UiReturnKind::Void },
    UiSig { method: "onActivate", runtime: "perry_ui_app_on_activate",
            args: &[UiArgKind::Closure], ret: UiReturnKind::Void },

    // ---- App extras ----
    UiSig { method: "appSetTimer", runtime: "perry_ui_app_set_timer",
            args: &[UiArgKind::Widget, UiArgKind::F64, UiArgKind::Closure], ret: UiReturnKind::Void },
    UiSig { method: "appSetMinSize", runtime: "perry_ui_app_set_min_size",
            args: &[UiArgKind::Widget, UiArgKind::F64, UiArgKind::F64], ret: UiReturnKind::Void },
    UiSig { method: "appSetMaxSize", runtime: "perry_ui_app_set_max_size",
            args: &[UiArgKind::Widget, UiArgKind::F64, UiArgKind::F64], ret: UiReturnKind::Void },

    // ---- Extra ScrollView alias (lowercase-v spelling matching the runtime FFI
    // symbol; the runtime takes a single vertical offset, not the x/y pair
    // declared on `scrollViewSetOffset` in index.d.ts — they coexist for now). ----
    UiSig { method: "scrollviewSetOffset", runtime: "perry_ui_scrollview_set_offset",
            args: &[UiArgKind::Widget, UiArgKind::F64], ret: UiReturnKind::Void },

    // ---- Table (issue #192) ----
    // NSTableView-backed scrollable table. Real implementation lives in
    // `perry-ui-macos`; iOS / Android / GTK4 / Windows / tvOS / visionOS /
    // watchOS export no-op stubs (returns handle 0, all setters no-op).
    // The render closure is `(row: number, col: number) => Widget` —
    // returns a Text/HStack/etc. that becomes the cell view. Free-function
    // call shape mirrors `pickerAddItem` / `pickerSetSelected` rather
    // than the `picker.addItem(...)` method form, matching the existing
    // wasm/js dispatch tables that already route `tableSetColumnHeader`
    // and friends.
    UiSig { method: "Table", runtime: "perry_ui_table_create",
            args: &[UiArgKind::F64, UiArgKind::F64, UiArgKind::Closure],
            ret: UiReturnKind::Widget },
    UiSig { method: "tableSetColumnHeader", runtime: "perry_ui_table_set_column_header",
            args: &[UiArgKind::Widget, UiArgKind::I64Raw, UiArgKind::Str],
            ret: UiReturnKind::Void },
    UiSig { method: "tableSetColumnWidth", runtime: "perry_ui_table_set_column_width",
            args: &[UiArgKind::Widget, UiArgKind::I64Raw, UiArgKind::F64],
            ret: UiReturnKind::Void },
    UiSig { method: "tableUpdateRowCount", runtime: "perry_ui_table_update_row_count",
            args: &[UiArgKind::Widget, UiArgKind::I64Raw], ret: UiReturnKind::Void },
    UiSig { method: "tableSetOnRowSelect", runtime: "perry_ui_table_set_on_row_select",
            args: &[UiArgKind::Widget, UiArgKind::Closure], ret: UiReturnKind::Void },
    UiSig { method: "tableGetSelectedRow", runtime: "perry_ui_table_get_selected_row",
            args: &[UiArgKind::Widget], ret: UiReturnKind::I64AsF64 },

    // ---- Camera (issue #191) ----
    // Live camera preview widget. Real implementations live in
    // `perry-ui-ios` (AVCaptureSession) and `perry-ui-android` (Camera2).
    // tvOS / visionOS / watchOS / macOS / GTK4 / Windows export no-op
    // stubs so cross-platform user code links cleanly. `cameraSampleColor`
    // returns packed RGB (`r*65536 + g*256 + b`) or `-1` if no frame is
    // available — F64 return is preserved as a plain JS number.
    UiSig { method: "CameraView", runtime: "perry_ui_camera_create",
            args: &[], ret: UiReturnKind::Widget },
    UiSig { method: "cameraStart", runtime: "perry_ui_camera_start",
            args: &[UiArgKind::Widget], ret: UiReturnKind::Void },
    UiSig { method: "cameraStop", runtime: "perry_ui_camera_stop",
            args: &[UiArgKind::Widget], ret: UiReturnKind::Void },
    UiSig { method: "cameraFreeze", runtime: "perry_ui_camera_freeze",
            args: &[UiArgKind::Widget], ret: UiReturnKind::Void },
    UiSig { method: "cameraUnfreeze", runtime: "perry_ui_camera_unfreeze",
            args: &[UiArgKind::Widget], ret: UiReturnKind::Void },
    UiSig { method: "cameraSampleColor", runtime: "perry_ui_camera_sample_color",
            args: &[UiArgKind::F64, UiArgKind::F64], ret: UiReturnKind::F64 },
    UiSig { method: "cameraSetOnTap", runtime: "perry_ui_camera_set_on_tap",
            args: &[UiArgKind::Widget, UiArgKind::Closure], ret: UiReturnKind::Void },

    // ---- Canvas ----
    UiSig { method: "Canvas", runtime: "perry_ui_canvas_create",
            args: &[UiArgKind::F64, UiArgKind::F64], ret: UiReturnKind::Widget },
];

/// Instance method table for perry/ui receiver-based calls.
/// These methods are called on a widget/window handle: `handle.method(args)`.
/// The handle is automatically prepended as the first i64 arg.
const PERRY_UI_INSTANCE_TABLE: &[UiSig] = &[
    // ---- Window instance methods ----
    UiSig { method: "show", runtime: "perry_ui_window_show",
            args: &[], ret: UiReturnKind::Void },
    UiSig { method: "hide", runtime: "perry_ui_window_hide",
            args: &[], ret: UiReturnKind::Void },
    UiSig { method: "close", runtime: "perry_ui_window_close",
            args: &[], ret: UiReturnKind::Void },
    UiSig { method: "setBody", runtime: "perry_ui_window_set_body",
            args: &[UiArgKind::Widget], ret: UiReturnKind::Void },
    UiSig { method: "setSize", runtime: "perry_ui_window_set_size",
            args: &[UiArgKind::F64, UiArgKind::F64], ret: UiReturnKind::Void },
    UiSig { method: "onFocusLost", runtime: "perry_ui_window_on_focus_lost",
            args: &[UiArgKind::Closure], ret: UiReturnKind::Void },

    // ---- State instance methods ----
    UiSig { method: "value", runtime: "perry_ui_state_get",
            args: &[], ret: UiReturnKind::F64 },
    UiSig { method: "set", runtime: "perry_ui_state_set",
            args: &[UiArgKind::F64], ret: UiReturnKind::Void },

    // ---- Canvas instance methods ----
    UiSig { method: "setFillColor", runtime: "perry_ui_canvas_set_fill_color",
            args: &[UiArgKind::F64, UiArgKind::F64, UiArgKind::F64, UiArgKind::F64],
            ret: UiReturnKind::Void },
    UiSig { method: "setStrokeColor", runtime: "perry_ui_canvas_set_stroke_color",
            args: &[UiArgKind::F64, UiArgKind::F64, UiArgKind::F64, UiArgKind::F64],
            ret: UiReturnKind::Void },
    UiSig { method: "setLineWidth", runtime: "perry_ui_canvas_set_line_width",
            args: &[UiArgKind::F64], ret: UiReturnKind::Void },
    UiSig { method: "fillRect", runtime: "perry_ui_canvas_fill_rect",
            args: &[UiArgKind::F64, UiArgKind::F64, UiArgKind::F64, UiArgKind::F64],
            ret: UiReturnKind::Void },
    UiSig { method: "strokeRect", runtime: "perry_ui_canvas_stroke_rect",
            args: &[UiArgKind::F64, UiArgKind::F64, UiArgKind::F64, UiArgKind::F64],
            ret: UiReturnKind::Void },
    UiSig { method: "clearRect", runtime: "perry_ui_canvas_clear_rect",
            args: &[UiArgKind::F64, UiArgKind::F64, UiArgKind::F64, UiArgKind::F64],
            ret: UiReturnKind::Void },
    UiSig { method: "beginPath", runtime: "perry_ui_canvas_begin_path",
            args: &[], ret: UiReturnKind::Void },
    UiSig { method: "moveTo", runtime: "perry_ui_canvas_move_to",
            args: &[UiArgKind::F64, UiArgKind::F64], ret: UiReturnKind::Void },
    UiSig { method: "lineTo", runtime: "perry_ui_canvas_line_to",
            args: &[UiArgKind::F64, UiArgKind::F64], ret: UiReturnKind::Void },
    UiSig { method: "arc", runtime: "perry_ui_canvas_arc",
            args: &[UiArgKind::F64, UiArgKind::F64, UiArgKind::F64, UiArgKind::F64, UiArgKind::F64],
            ret: UiReturnKind::Void },
    UiSig { method: "closePath", runtime: "perry_ui_canvas_close_path",
            args: &[], ret: UiReturnKind::Void },
    UiSig { method: "fill", runtime: "perry_ui_canvas_fill",
            args: &[], ret: UiReturnKind::Void },
    // `stroke()` maps to perry_ui_canvas_stroke_path (no-arg stateful form).
    // The older perry_ui_canvas_stroke(h,r,g,b,a,lw) stateless form is kept
    // for the legacy fill_gradient API and is not removed.
    UiSig { method: "stroke", runtime: "perry_ui_canvas_stroke_path",
            args: &[], ret: UiReturnKind::Void },
    UiSig { method: "fillText", runtime: "perry_ui_canvas_fill_text",
            args: &[UiArgKind::Str, UiArgKind::F64, UiArgKind::F64],
            ret: UiReturnKind::Void },
    UiSig { method: "setFont", runtime: "perry_ui_canvas_set_font",
            args: &[UiArgKind::Str], ret: UiReturnKind::Void },
];

fn perry_ui_table_lookup(method: &str) -> Option<&'static UiSig> {
    PERRY_UI_TABLE.iter().find(|s| s.method == method)
}

fn perry_ui_instance_method_lookup(method: &str) -> Option<&'static UiSig> {
    PERRY_UI_INSTANCE_TABLE.iter().find(|s| s.method == method)
}

fn perry_container_table_lookup(method: &str) -> Option<&'static UiSig> {
    PERRY_CONTAINER_TABLE.iter().find(|s| s.method == method)
}

fn perry_compose_table_lookup(method: &str) -> Option<&'static UiSig> {
    PERRY_COMPOSE_TABLE.iter().find(|s| s.method == method)
}

fn perry_workloads_table_lookup(method: &str) -> Option<&'static UiSig> {
    PERRY_WORKLOADS_TABLE.iter().find(|s| s.method == method)
}

fn perry_container_table_lookup(method: &str) -> Option<&'static UiSig> {
    PERRY_CONTAINER_TABLE.iter().find(|s| s.method == method)
}

fn perry_compose_table_lookup(method: &str) -> Option<&'static UiSig> {
    PERRY_COMPOSE_TABLE.iter().find(|s| s.method == method)
}

fn perry_workloads_table_lookup(method: &str) -> Option<&'static UiSig> {
    PERRY_WORKLOADS_TABLE.iter().find(|s| s.method == method)
}

// =============================================================================
// perry/system dispatch table
// =============================================================================

/// Maps JS import names from `perry/system` to their `perry_system_*` / `perry_*`
/// runtime C symbols. Uses the same UiSig + lower_perry_ui_table_call machinery
/// since the calling convention is identical.
static PERRY_SYSTEM_TABLE: &[UiSig] = &[
    UiSig { method: "isDarkMode", runtime: "perry_system_is_dark_mode",
            args: &[], ret: UiReturnKind::F64 },
    UiSig { method: "getDeviceIdiom", runtime: "perry_get_device_idiom",
            args: &[], ret: UiReturnKind::F64 },
    UiSig { method: "openURL", runtime: "perry_system_open_url",
            args: &[UiArgKind::Str], ret: UiReturnKind::Void },
    UiSig { method: "keychainSave", runtime: "perry_system_keychain_save",
            args: &[UiArgKind::Str, UiArgKind::Str], ret: UiReturnKind::Void },
    UiSig { method: "keychainGet", runtime: "perry_system_keychain_get",
            args: &[UiArgKind::Str], ret: UiReturnKind::F64 },
    UiSig { method: "keychainDelete", runtime: "perry_system_keychain_delete",
            args: &[UiArgKind::Str], ret: UiReturnKind::Void },
    UiSig { method: "preferencesGet", runtime: "perry_system_preferences_get",
            args: &[UiArgKind::Str], ret: UiReturnKind::F64 },
    UiSig { method: "preferencesSet", runtime: "perry_system_preferences_set",
            args: &[UiArgKind::Str, UiArgKind::F64], ret: UiReturnKind::Void },
    UiSig { method: "notificationSend", runtime: "perry_system_notification_send",
            args: &[UiArgKind::Str, UiArgKind::Str], ret: UiReturnKind::Void },
    UiSig { method: "notificationRegisterRemote", runtime: "perry_system_notification_register_remote",
            args: &[UiArgKind::Closure], ret: UiReturnKind::Void },
    UiSig { method: "notificationOnReceive", runtime: "perry_system_notification_on_receive",
            args: &[UiArgKind::Closure], ret: UiReturnKind::Void },
    UiSig { method: "notificationOnBackgroundReceive", runtime: "perry_system_notification_on_background_receive",
            args: &[UiArgKind::Closure], ret: UiReturnKind::Void },
    UiSig { method: "notificationCancel", runtime: "perry_system_notification_cancel",
            args: &[UiArgKind::Str], ret: UiReturnKind::Void },
    UiSig { method: "notificationOnTap", runtime: "perry_system_notification_on_tap",
            args: &[UiArgKind::Closure], ret: UiReturnKind::Void },
    UiSig { method: "audioStart", runtime: "perry_system_audio_start",
            args: &[], ret: UiReturnKind::F64 },
    UiSig { method: "audioStop", runtime: "perry_system_audio_stop",
            args: &[], ret: UiReturnKind::Void },
    UiSig { method: "audioGetLevel", runtime: "perry_system_audio_get_level",
            args: &[], ret: UiReturnKind::F64 },
    UiSig { method: "audioGetPeak", runtime: "perry_system_audio_get_peak",
            args: &[], ret: UiReturnKind::F64 },
    UiSig { method: "audioGetWaveform", runtime: "perry_system_audio_get_waveform",
            args: &[UiArgKind::F64], ret: UiReturnKind::F64 },
    UiSig { method: "getDeviceModel", runtime: "perry_system_get_device_model",
            args: &[], ret: UiReturnKind::F64 },
];

fn perry_system_table_lookup(method: &str) -> Option<&'static UiSig> {
    PERRY_SYSTEM_TABLE.iter().find(|s| s.method == method)
}

// =============================================================================
// perry/i18n format-wrapper dispatch table
// =============================================================================

/// Maps the TS exports from `types/perry/i18n/index.d.ts` (Currency, Percent,
/// FormatNumber, ShortDate, LongDate, FormatTime, Raw) to their `perry_i18n_*`
/// runtime symbols. Each runtime entry is a default-locale single-arg wrapper
/// over the lower-level `perry_i18n_format_*(value, locale_idx)` exports —
/// the wrapper folds in `LOCALE_INDEX` so the dispatch table here can stay
/// consistent with the other UiSig tables (one TS arg → one runtime arg).
///
/// `t()` is handled separately at the top of `lower_native_method_call`
/// because the perry-transform i18n pass replaces its first arg with an
/// `Expr::I18nString` — there's no runtime call involved.
static PERRY_I18N_TABLE: &[UiSig] = &[
    UiSig { method: "Currency",     runtime: "perry_i18n_format_currency_default",
            args: &[UiArgKind::F64], ret: UiReturnKind::Str },
    UiSig { method: "Percent",      runtime: "perry_i18n_format_percent_default",
            args: &[UiArgKind::F64], ret: UiReturnKind::Str },
    UiSig { method: "FormatNumber", runtime: "perry_i18n_format_number_default",
            args: &[UiArgKind::F64], ret: UiReturnKind::Str },
    UiSig { method: "ShortDate",    runtime: "perry_i18n_format_date_short",
            args: &[UiArgKind::F64], ret: UiReturnKind::Str },
    UiSig { method: "LongDate",     runtime: "perry_i18n_format_date_long",
            args: &[UiArgKind::F64], ret: UiReturnKind::Str },
    UiSig { method: "FormatTime",   runtime: "perry_i18n_format_time_default",
            args: &[UiArgKind::F64], ret: UiReturnKind::Str },
    UiSig { method: "Raw",          runtime: "perry_i18n_format_raw",
            args: &[UiArgKind::F64], ret: UiReturnKind::Str },
];

fn perry_i18n_table_lookup(method: &str) -> Option<&'static UiSig> {
    PERRY_I18N_TABLE.iter().find(|s| s.method == method)
}

// =============================================================================
// perry/plugin dispatch table
// =============================================================================

/// Receiver-less (host-side) functions exported from perry/plugin.
/// These map `import { loadPlugin, listPlugins, … } from "perry/plugin"` to
/// their `perry_plugin_*` runtime symbols. Arg shapes match plugin.rs exactly:
/// strings are passed as NaN-boxed f64 (`UiArgKind::F64`) because the runtime
/// calls `extract_string(nanboxed: f64)` internally — not raw pointer.
static PERRY_PLUGIN_TABLE: &[UiSig] = &[
    // loadPlugin(path) -> PluginId (NaN-boxed i64 handle, 0 on failure)
    UiSig { method: "loadPlugin", runtime: "perry_plugin_load",
            args: &[UiArgKind::F64], ret: UiReturnKind::Widget },
    // unloadPlugin(id) -> void
    UiSig { method: "unloadPlugin", runtime: "perry_plugin_unload",
            args: &[UiArgKind::Widget], ret: UiReturnKind::Void },
    // emitHook(hookName, context) -> context (possibly transformed by handlers)
    UiSig { method: "emitHook", runtime: "perry_plugin_emit_hook",
            args: &[UiArgKind::F64, UiArgKind::F64], ret: UiReturnKind::F64 },
    // emitEvent(event, data) -> undefined
    UiSig { method: "emitEvent", runtime: "perry_plugin_emit_event",
            args: &[UiArgKind::F64, UiArgKind::F64], ret: UiReturnKind::F64 },
    // invokeTool(name, args) -> handler return value
    UiSig { method: "invokeTool", runtime: "perry_plugin_invoke_tool",
            args: &[UiArgKind::F64, UiArgKind::F64], ret: UiReturnKind::F64 },
    // setPluginConfig(key, value) -> undefined
    UiSig { method: "setPluginConfig", runtime: "perry_plugin_set_config",
            args: &[UiArgKind::F64, UiArgKind::F64], ret: UiReturnKind::F64 },
    // discoverPlugins(dir) -> string[] of plugin paths
    UiSig { method: "discoverPlugins", runtime: "perry_plugin_discover",
            args: &[UiArgKind::F64], ret: UiReturnKind::F64 },
    // listPlugins() -> { id, name, version, description }[]
    UiSig { method: "listPlugins", runtime: "perry_plugin_list_plugins",
            args: &[], ret: UiReturnKind::F64 },
    // listHooks() -> string[]
    UiSig { method: "listHooks", runtime: "perry_plugin_list_hooks",
            args: &[], ret: UiReturnKind::F64 },
    // listTools() -> { name, description, pluginId }[]
    UiSig { method: "listTools", runtime: "perry_plugin_list_tools",
            args: &[], ret: UiReturnKind::F64 },
    // pluginCount() -> number
    UiSig { method: "pluginCount", runtime: "perry_plugin_count",
            args: &[], ret: UiReturnKind::I64AsF64 },
    // initPlugins() -> void  (call once from main before loading plugins)
    UiSig { method: "initPlugins", runtime: "perry_plugin_init",
            args: &[], ret: UiReturnKind::Void },
];

/// Instance methods on a PluginApi handle returned by `loadPlugin`.
/// The handle (NaN-boxed i64) is the receiver and is prepended as the
/// first `i64` arg (`api_handle`) in every runtime call.
static PERRY_PLUGIN_INSTANCE_TABLE: &[UiSig] = &[
    // api.registerHook(hookName, handler) -> undefined
    UiSig { method: "registerHook", runtime: "perry_plugin_register_hook",
            args: &[UiArgKind::F64, UiArgKind::Closure], ret: UiReturnKind::F64 },
    // api.registerHookEx(hookName, handler, priority, mode) -> undefined
    UiSig { method: "registerHookEx", runtime: "perry_plugin_register_hook_ex",
            args: &[UiArgKind::F64, UiArgKind::Closure, UiArgKind::I64Raw, UiArgKind::I64Raw],
            ret: UiReturnKind::F64 },
    // api.registerTool(name, description, handler) -> undefined
    UiSig { method: "registerTool", runtime: "perry_plugin_register_tool",
            args: &[UiArgKind::F64, UiArgKind::F64, UiArgKind::Closure], ret: UiReturnKind::F64 },
    // api.registerService(name, startFn, stopFn) -> undefined
    UiSig { method: "registerService", runtime: "perry_plugin_register_service",
            args: &[UiArgKind::F64, UiArgKind::Closure, UiArgKind::Closure], ret: UiReturnKind::F64 },
    // api.registerRoute(path, handler) -> undefined
    UiSig { method: "registerRoute", runtime: "perry_plugin_register_route",
            args: &[UiArgKind::F64, UiArgKind::Closure], ret: UiReturnKind::F64 },
    // api.getConfig(key) -> any
    UiSig { method: "getConfig", runtime: "perry_plugin_get_config",
            args: &[UiArgKind::F64], ret: UiReturnKind::F64 },
    // api.log(level, message) -> undefined   (level: 0=DEBUG,1=INFO,2=WARN,3=ERROR)
    UiSig { method: "log", runtime: "perry_plugin_log",
            args: &[UiArgKind::I64Raw, UiArgKind::F64], ret: UiReturnKind::F64 },
    // api.setMetadata(name, version, description) -> undefined
    UiSig { method: "setMetadata", runtime: "perry_plugin_set_metadata",
            args: &[UiArgKind::F64, UiArgKind::F64, UiArgKind::F64], ret: UiReturnKind::F64 },
    // api.on(event, handler) -> undefined
    UiSig { method: "on", runtime: "perry_plugin_on",
            args: &[UiArgKind::F64, UiArgKind::Closure], ret: UiReturnKind::F64 },
    // api.emit(event, data) -> undefined
    UiSig { method: "emit", runtime: "perry_plugin_emit",
            args: &[UiArgKind::F64, UiArgKind::F64], ret: UiReturnKind::F64 },
];

fn perry_plugin_table_lookup(method: &str) -> Option<&'static UiSig> {
    PERRY_PLUGIN_TABLE.iter().find(|s| s.method == method)
}

fn perry_plugin_instance_method_lookup(method: &str) -> Option<&'static UiSig> {
    PERRY_PLUGIN_INSTANCE_TABLE.iter().find(|s| s.method == method)
}

/// Lower a perry/ui call described by `sig`. Walks each arg, applies
/// the per-kind coercion to produce an LLVM SSA value of the right type,
/// lazy-declares the runtime function, emits the call, and boxes the
/// return value per `sig.ret`.
///
/// Args length mismatch (caller passed wrong number of args) → falls
/// back to lowering all args for side effects + returning the
/// zero-sentinel. The catch-all is intentional: TS users may write
/// `Text()` (no arg) or `Text(s, extra)` and we don't want to bail
/// the entire compilation.
fn lower_perry_ui_table_call(
    ctx: &mut FnCtx<'_>,
    sig: &UiSig,
    args: &[Expr],
) -> Result<String> {
    // Issue #185 Phase C step 4: when a Widget-returning constructor is
    // called with one extra trailing arg, treat it as an inline `style`
    // object and apply via `apply_inline_style` after the create call.
    // Lets every widget in the table (Text, Toggle, Slider, TextField,
    // Spacer, Divider, ImageFile, ImageSymbol, ProgressView, NavStack,
    // ZStack, etc.) accept the same React-style ergonomics that Button
    // already has, with no per-widget code edits.
    let inline_style_arg: Option<&Expr> =
        if args.len() == sig.args.len() + 1
            && matches!(sig.ret, UiReturnKind::Widget)
        {
            Some(&args[sig.args.len()])
        } else {
            None
        };
    let declared_arg_count = sig.args.len();

    if args.len() != declared_arg_count && inline_style_arg.is_none() {
        // Mismatched arity (and not a trailing-style absorption case)
        // — fall back to side-effect lowering only.
        for a in args {
            let _ = lower_expr(ctx, a)?;
        }
        return Ok(double_literal(0.0));
    }

    // Lower each arg according to its declared kind. Build two parallel
    // vectors so we can pass them through to `blk.call(...)` in one shot
    // without intermediate borrows. Iterate the declared sig args only
    // — the inline-style trailing arg (if present) is consumed below.
    let mut llvm_args: Vec<(crate::types::LlvmType, String)> =
        Vec::with_capacity(declared_arg_count);
    let mut runtime_param_types: Vec<crate::types::LlvmType> =
        Vec::with_capacity(declared_arg_count);
    for (kind, arg) in sig.args.iter().zip(args.iter().take(declared_arg_count)) {
        match kind {
            UiArgKind::Widget => {
                // Widgets are NaN-boxed pointers. Lower as JSValue,
                // strip the POINTER_TAG bits to get the raw 1-based
                // handle as i64.
                let v = lower_expr(ctx, arg)?;
                let blk = ctx.block();
                let h = unbox_to_i64(blk, &v);
                llvm_args.push((I64, h));
                runtime_param_types.push(I64);
            }
            UiArgKind::Str => {
                let h = get_raw_string_ptr(ctx, arg)?;
                llvm_args.push((I64, h));
                runtime_param_types.push(I64);
            }
            UiArgKind::F64 => {
                let v = lower_expr(ctx, arg)?;
                llvm_args.push((DOUBLE, v));
                runtime_param_types.push(DOUBLE);
            }
            UiArgKind::Closure => {
                // Closures are NaN-boxed pointers passed as f64. The
                // runtime side calls `js_closure_call0` (or callN) on
                // them, so it expects the f64 representation.
                let v = lower_expr(ctx, arg)?;
                llvm_args.push((DOUBLE, v));
                runtime_param_types.push(DOUBLE);
            }
            UiArgKind::I64Raw => {
                // Numeric arg the runtime wants as i64 (e.g. enum tag,
                // boolean flag). `fptosi` converts the f64 to a signed
                // integer.
                let v = lower_expr(ctx, arg)?;
                let blk = ctx.block();
                let i = blk.fptosi(DOUBLE, &v, I64);
                llvm_args.push((I64, i));
                runtime_param_types.push(I64);
            }
        }
    }

    // Lazy-declare the runtime function so the linker pulls in the
    // libperry_ui_*.a symbol. Same pending_declares mechanism the
    // cross-module call site uses for `perry_fn_*`.
    let return_type = match sig.ret {
        UiReturnKind::Widget | UiReturnKind::I64AsF64 | UiReturnKind::Promise => I64,
        UiReturnKind::F64 => DOUBLE,
        UiReturnKind::Void => crate::types::VOID,
        UiReturnKind::Str => I64,
    };
    ctx.pending_declares.push((
        sig.runtime.to_string(),
        return_type,
        runtime_param_types,
    ));

    // Emit the call. Slices need a borrow of `llvm_args` because the
    // tuple's second field is `String` and `blk.call` expects `&str`.
    let arg_slices: Vec<(crate::types::LlvmType, &str)> =
        llvm_args.iter().map(|(t, s)| (*t, s.as_str())).collect();
    match sig.ret {
        UiReturnKind::Widget => {
            // Scope `blk` so the mutable borrow on `ctx` is released
            // before the optional `apply_inline_style` call re-borrows.
            let handle = {
                let blk = ctx.block();
                blk.call(I64, sig.runtime, &arg_slices)
            };
            // Issue #185 Phase C step 4: apply inline style if a
            // trailing object literal was passed.
            if let Some(style_arg) = inline_style_arg {
                apply_inline_style(ctx, &handle, style_arg)?;
            }
            let blk = ctx.block();
            Ok(nanbox_pointer_inline(blk, &handle))
        }
        UiReturnKind::F64 => {
            Ok(ctx.block().call(DOUBLE, sig.runtime, &arg_slices))
        }
        UiReturnKind::Void => {
            ctx.block().call_void(sig.runtime, &arg_slices);
            Ok(double_literal(0.0))
        }
        UiReturnKind::Str => {
            let blk = ctx.block();
            let raw = blk.call(I64, sig.runtime, &arg_slices);
            Ok(crate::expr::nanbox_string_inline(blk, &raw))
        }
        UiReturnKind::I64AsF64 => {
            let blk = ctx.block();
            let raw = blk.call(I64, sig.runtime, &arg_slices);
            Ok(blk.sitofp(I64, &raw, DOUBLE))
        }
        UiReturnKind::Promise => {
            let blk = ctx.block();
            let raw = blk.call(I64, sig.runtime, &arg_slices);
            // Promise handles are I64 (pointers), NaN-box them as POINTER
            Ok(nanbox_pointer_inline(blk, &raw))
        }
        UiReturnKind::Promise => {
            let blk = ctx.block();
            let raw = blk.call(I64, sig.runtime, &arg_slices);
            // Promise handles are I64 (pointers), NaN-box them as POINTER
            Ok(nanbox_pointer_inline(blk, &raw))
        }
    }
}

// ============================================================================
// Native stdlib module dispatch (fastify, mysql2, ws, pg, ioredis, mongodb,
// better-sqlite3, etc.). Ported from the old Cranelift codegen's dispatch
// table that was lost in the v0.5.0 LLVM cutover.
// ============================================================================

/// How each argument should be coerced before passing to the runtime fn.
#[derive(Copy, Clone, Debug)]
enum NativeArgKind {
    /// NaN-boxed f64 — pass as-is (objects, generic JSValues).
    F64,
    /// NaN-boxed string → extract raw i64 pointer via js_get_string_pointer_unified.
    /// Use for Rust signatures like `*const StringHeader`.
    StrPtr,
    /// NaN-boxed closure/pointer → unbox to i64 via the standard mask.
    PtrI64,
    /// Pass the NaN-boxed JSValue bits as-is (bitcast f64 → i64, no
    /// unboxing). Use for Rust signatures where the function receives
    /// `name: i64` and internally calls `string_from_nanboxed(name)` or
    /// similar — the callee expects the full NaN-boxed value, not an
    /// unboxed raw pointer. Common pattern in fastify context methods.
    JsvalI64,
}

/// What the runtime function returns.
#[derive(Copy, Clone, Debug)]
enum NativeRetKind {
    /// Returns i64 handle → NaN-box as POINTER.
    Ptr,
    /// Returns `*mut StringHeader` → NaN-box as STRING. Use for runtime
    /// functions whose Rust signature returns a raw string pointer; the
    /// caller (and `JSON.stringify`, string-comparison, etc.) needs the
    /// STRING_TAG to recognize it as a string rather than a heap object.
    Str,
    /// Returns f64 → pass through (NaN-boxed JSValue).
    F64,
    /// Returns i32 → ignored, return TAG_UNDEFINED.
    I32Void,
    /// Returns void → return TAG_UNDEFINED.
    Void,
}

#[derive(Copy, Clone, Debug)]
struct NativeModSig {
    module: &'static str,
    has_receiver: bool,
    method: &'static str,
    /// Optional class_name filter. When Some, only matches if the HIR's
    /// class_name equals this value (e.g. "Pool" vs "Connection" for mysql2).
    /// When None, matches regardless of class_name.
    class_filter: Option<&'static str>,
    runtime: &'static str,
    args: &'static [NativeArgKind],
    ret: NativeRetKind,
}

// Short aliases to keep the table compact without wildcard imports
// (wildcard would clash with crate::types::* names like I64, DOUBLE).
const NA_F64: NativeArgKind = NativeArgKind::F64;
const NA_STR: NativeArgKind = NativeArgKind::StrPtr;
const NA_PTR: NativeArgKind = NativeArgKind::PtrI64;
const NA_JSV: NativeArgKind = NativeArgKind::JsvalI64;
const NR_PTR: NativeRetKind = NativeRetKind::Ptr;
const NR_STR: NativeRetKind = NativeRetKind::Str;
const NR_F64: NativeRetKind = NativeRetKind::F64;
const NR_I32: NativeRetKind = NativeRetKind::I32Void;
const NR_VOID: NativeRetKind = NativeRetKind::Void;

/// Static dispatch table for native stdlib modules. Each entry maps
/// `(module, has_receiver, method)` → runtime function, with per-arg
/// coercion rules and return-value boxing.
///
/// The receiver (when `has_receiver = true`) is always NaN-unboxed to
/// an i64 pointer and passed as the first argument.
const NATIVE_MODULE_TABLE: &[NativeModSig] = &[
    // ========== Fastify HTTP Framework ==========
    NativeModSig { module: "fastify", has_receiver: false, method: "default",
        class_filter: None,
        runtime: "js_fastify_create_with_opts", args: &[NA_F64], ret: NR_PTR },
    NativeModSig { module: "fastify", has_receiver: true, method: "get",
        class_filter: None,
        runtime: "js_fastify_get", args: &[NA_STR, NA_PTR], ret: NR_I32 },
    NativeModSig { module: "fastify", has_receiver: true, method: "post",
        class_filter: None,
        runtime: "js_fastify_post", args: &[NA_STR, NA_PTR], ret: NR_I32 },
    NativeModSig { module: "fastify", has_receiver: true, method: "put",
        class_filter: None,
        runtime: "js_fastify_put", args: &[NA_STR, NA_PTR], ret: NR_I32 },
    NativeModSig { module: "fastify", has_receiver: true, method: "delete",
        class_filter: None,
        runtime: "js_fastify_delete", args: &[NA_STR, NA_PTR], ret: NR_I32 },
    NativeModSig { module: "fastify", has_receiver: true, method: "patch",
        class_filter: None,
        runtime: "js_fastify_patch", args: &[NA_STR, NA_PTR], ret: NR_I32 },
    NativeModSig { module: "fastify", has_receiver: true, method: "head",
        class_filter: None,
        runtime: "js_fastify_head", args: &[NA_STR, NA_PTR], ret: NR_I32 },
    NativeModSig { module: "fastify", has_receiver: true, method: "options",
        class_filter: None,
        runtime: "js_fastify_options", args: &[NA_STR, NA_PTR], ret: NR_I32 },
    NativeModSig { module: "fastify", has_receiver: true, method: "all",
        class_filter: None,
        runtime: "js_fastify_all", args: &[NA_STR, NA_PTR], ret: NR_I32 },
    NativeModSig { module: "fastify", has_receiver: true, method: "route",
        class_filter: None,
        runtime: "js_fastify_route", args: &[NA_STR, NA_STR, NA_PTR], ret: NR_I32 },
    NativeModSig { module: "fastify", has_receiver: true, method: "addHook",
        class_filter: None,
        runtime: "js_fastify_add_hook", args: &[NA_STR, NA_PTR], ret: NR_I32 },
    NativeModSig { module: "fastify", has_receiver: true, method: "setErrorHandler",
        class_filter: None,
        runtime: "js_fastify_set_error_handler", args: &[NA_PTR], ret: NR_I32 },
    NativeModSig { module: "fastify", has_receiver: true, method: "register",
        class_filter: None,
        runtime: "js_fastify_register", args: &[NA_PTR, NA_F64], ret: NR_I32 },
    NativeModSig { module: "fastify", has_receiver: true, method: "listen",
        class_filter: None,
        runtime: "js_fastify_listen", args: &[NA_F64, NA_PTR], ret: NR_VOID },
    // Fastify request methods
    NativeModSig { module: "fastify", has_receiver: true, method: "method",
        class_filter: None,
        runtime: "js_fastify_req_method", args: &[], ret: NR_STR },
    NativeModSig { module: "fastify", has_receiver: true, method: "url",
        class_filter: None,
        runtime: "js_fastify_req_url", args: &[], ret: NR_STR },
    NativeModSig { module: "fastify", has_receiver: true, method: "params",
        class_filter: None,
        // Returns the parsed path-params object (e.g. `{id: "42"}` for /users/:id),
        // not the raw JSON string — `request.params.id` must be the value, not
        // undefined. `js_fastify_req_params` (string) is still available via
        // the lower-level FFI but isn't reachable from TypeScript.
        runtime: "js_fastify_req_params_object", args: &[], ret: NR_F64 },
    NativeModSig { module: "fastify", has_receiver: true, method: "param",
        class_filter: None,
        runtime: "js_fastify_req_param", args: &[NA_JSV], ret: NR_STR },
    NativeModSig { module: "fastify", has_receiver: true, method: "query",
        class_filter: None,
        runtime: "js_fastify_req_query_object", args: &[], ret: NR_F64 },
    NativeModSig { module: "fastify", has_receiver: true, method: "rawBody",
        class_filter: None,
        runtime: "js_fastify_req_body", args: &[], ret: NR_STR },
    NativeModSig { module: "fastify", has_receiver: true, method: "headers",
        class_filter: None,
        runtime: "js_fastify_req_headers", args: &[], ret: NR_PTR },
    NativeModSig { module: "fastify", has_receiver: true, method: "header",
        class_filter: None,
        runtime: "js_fastify_req_header", args: &[NA_JSV], ret: NR_STR },
    NativeModSig { module: "fastify", has_receiver: true, method: "user",
        class_filter: None,
        runtime: "js_fastify_req_get_user_data", args: &[], ret: NR_F64 },
    // Fastify reply methods
    NativeModSig { module: "fastify", has_receiver: true, method: "status",
        class_filter: None,
        runtime: "js_fastify_reply_status", args: &[NA_F64], ret: NR_PTR },
    // `reply.code(N)` is an alias for `reply.status(N)` in npm Fastify. Without
    // this row, `reply.code(201)` silently no-op'd and the HTTP status stayed 200.
    NativeModSig { module: "fastify", has_receiver: true, method: "code",
        class_filter: None,
        runtime: "js_fastify_reply_status", args: &[NA_F64], ret: NR_PTR },
    NativeModSig { module: "fastify", has_receiver: true, method: "send",
        class_filter: None,
        runtime: "js_fastify_reply_send", args: &[NA_F64], ret: NR_I32 },
    // Fastify context methods (Hono-style)
    NativeModSig { module: "fastify", has_receiver: true, method: "text",
        class_filter: None,
        runtime: "js_fastify_ctx_text", args: &[NA_JSV, NA_F64], ret: NR_F64 },
    NativeModSig { module: "fastify", has_receiver: true, method: "html",
        class_filter: None,
        runtime: "js_fastify_ctx_html", args: &[NA_JSV, NA_F64], ret: NR_F64 },
    NativeModSig { module: "fastify", has_receiver: true, method: "redirect",
        class_filter: None,
        runtime: "js_fastify_ctx_redirect", args: &[NA_JSV, NA_F64], ret: NR_F64 },
    NativeModSig { module: "fastify", has_receiver: true, method: "json",
        class_filter: None,
        runtime: "js_fastify_ctx_json", args: &[NA_F64, NA_F64], ret: NR_F64 },
    NativeModSig { module: "fastify", has_receiver: true, method: "body",
        class_filter: None,
        runtime: "js_fastify_req_json", args: &[], ret: NR_F64 },

    // ========== MySQL2 ==========
    NativeModSig { module: "mysql2", has_receiver: false, method: "createConnection",
        class_filter: None,
        runtime: "js_mysql2_create_connection", args: &[NA_F64], ret: NR_PTR },
    NativeModSig { module: "mysql2", has_receiver: false, method: "createPool",
        class_filter: None,
        runtime: "js_mysql2_create_pool", args: &[NA_F64], ret: NR_PTR },
    NativeModSig { module: "mysql2/promise", has_receiver: false, method: "createConnection",
        class_filter: None,
        runtime: "js_mysql2_create_connection", args: &[NA_F64], ret: NR_PTR },
    NativeModSig { module: "mysql2/promise", has_receiver: false, method: "createPool",
        class_filter: None,
        runtime: "js_mysql2_create_pool", args: &[NA_F64], ret: NR_PTR },
    // mysql2 Pool-specific methods (class_filter: Some("Pool"))
    NativeModSig { module: "mysql2", has_receiver: true, method: "query",
        class_filter: Some("Pool"),
        runtime: "js_mysql2_pool_query", args: &[NA_STR, NA_PTR], ret: NR_PTR },
    NativeModSig { module: "mysql2", has_receiver: true, method: "execute",
        class_filter: Some("Pool"),
        runtime: "js_mysql2_pool_execute", args: &[NA_STR, NA_PTR], ret: NR_PTR },
    NativeModSig { module: "mysql2", has_receiver: true, method: "end",
        class_filter: Some("Pool"),
        runtime: "js_mysql2_pool_end", args: &[], ret: NR_PTR },
    NativeModSig { module: "mysql2/promise", has_receiver: true, method: "query",
        class_filter: Some("Pool"),
        runtime: "js_mysql2_pool_query", args: &[NA_STR, NA_PTR], ret: NR_PTR },
    NativeModSig { module: "mysql2/promise", has_receiver: true, method: "execute",
        class_filter: Some("Pool"),
        runtime: "js_mysql2_pool_execute", args: &[NA_STR, NA_PTR], ret: NR_PTR },
    NativeModSig { module: "mysql2/promise", has_receiver: true, method: "end",
        class_filter: Some("Pool"),
        runtime: "js_mysql2_pool_end", args: &[], ret: NR_PTR },
