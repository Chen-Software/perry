// Perry Web Runtime - maps perry/ui widgets to DOM elements
// This file is embedded via include_str! and injected into HTML output.

(function() {
"use strict";

// --- Handle System ---
// Widget handles are wrapper objects with methods that delegate to DOM elements.
// State handles are objects with .value getter/setter and methods.

const handles = new Map();   // handle int → DOM element
const states = new Map();    // handle int → { _value, subscribers[] }
let nextHandle = 1;

function allocHandle(el) {
    const h = nextHandle++;
    handles.set(h, el);
    return h;
}

function getHandle(h) {
    if (typeof h === "object" && h !== null && h._perryHandle) return handles.get(h._perryHandle);
    return handles.get(h);
}

function getHandleId(h) {
    if (typeof h === "object" && h !== null && h._perryHandle) return h._perryHandle;
    return h;
}

// Create a widget wrapper object with all perry/ui methods
function wrapWidget(h) {
    const w = {
        _perryHandle: h,
        addChild(child) { perry_ui_widget_add_child(h, getHandleId(child)); },
        removeAllChildren() { perry_ui_widget_remove_all_children(h); },
        setBackground(r, g, b, a) { perry_ui_set_background(h, r, g, b, a); },
        setForeground(r, g, b, a) { perry_ui_set_foreground(h, r, g, b, a); },
        setFontSize(size) { perry_ui_set_font_size(h, size); },
        setFontWeight(weight) { perry_ui_set_font_weight(h, weight); },
        setFontFamily(family) { perry_ui_set_font_family(h, family); },
        setPadding(val) { perry_ui_set_padding(h, val); },
        setFrame(w, ht) { perry_ui_set_frame(h, w, ht); },
        setCornerRadius(r) { perry_ui_set_corner_radius(h, r); },
        setBorder(w, r, g, b, a) { perry_ui_set_border(h, w, r, g, b, a); },
        setOpacity(o) { perry_ui_set_opacity(h, o); },
        setEnabled(e) { perry_ui_set_enabled(h, e); },
        setTooltip(t) { perry_ui_set_tooltip(h, t); },
        setControlSize(s) { perry_ui_set_control_size(h, s); },
        animateOpacity(from, to, dur) { perry_ui_animate_opacity(h, from, to, dur); },
        animatePosition(fx, fy, tx, ty, dur) { perry_ui_animate_position(h, fx, fy, tx, ty, dur); },
        setOnClick(cb) { perry_ui_set_on_click(h, cb); },
        setOnHover(cb) { perry_ui_set_on_hover(h, cb); },
        setOnDoubleClick(cb) { perry_ui_set_on_double_click(h, cb); },
        run() { perry_ui_app_run(); },
        // Canvas methods
        fillRect(x, y, w, ht) { perry_ui_canvas_fill_rect(h, x, y, w, ht); },
        strokeRect(x, y, w, ht) { perry_ui_canvas_stroke_rect(h, x, y, w, ht); },
        clearRect(x, y, w, ht) { perry_ui_canvas_clear_rect(h, x, y, w, ht); },
        setFillColor(r, g, b, a) { perry_ui_canvas_set_fill_color(h, r, g, b, a); },
        setStrokeColor(r, g, b, a) { perry_ui_canvas_set_stroke_color(h, r, g, b, a); },
        beginPath() { perry_ui_canvas_begin_path(h); },
        moveTo(x, y) { perry_ui_canvas_move_to(h, x, y); },
        lineTo(x, y) { perry_ui_canvas_line_to(h, x, y); },
        arc(x, y, r, sa, ea) { perry_ui_canvas_arc(h, x, y, r, sa, ea); },
        closePath() { perry_ui_canvas_close_path(h); },
        fill() { perry_ui_canvas_fill(h); },
        stroke() { perry_ui_canvas_stroke(h); },
        setLineWidth(w) { perry_ui_canvas_set_line_width(h, w); },
        fillText(t, x, y) { perry_ui_canvas_fill_text(h, t, x, y); },
        setFont(f) { perry_ui_canvas_set_font(h, f); },
    };
    return w;
}

// --- State Reactive System ---
function stateCreate(initialValue) {
    const h = nextHandle++;
    const sObj = { _value: initialValue, subscribers: [] };
    states.set(h, sObj);
    // Return a state wrapper with .value getter/setter and methods
    const wrapper = {
        _perryHandle: h,
        _perryState: true,
        get value() { return sObj._value; },
        set value(v) { stateSet(h, v); },
        get() { return sObj._value; },
        set(v) { stateSet(h, v); },
        bindText(widget) { perry_ui_state_bind_text(h, getHandleId(widget)); },
        bindTextNumeric(widget) { perry_ui_state_bind_text_numeric(h, getHandleId(widget)); },
        bindSlider(widget) { perry_ui_state_bind_slider(h, getHandleId(widget)); },
        bindToggle(widget) { perry_ui_state_bind_toggle(h, getHandleId(widget)); },
        bindVisibility(widget) { perry_ui_state_bind_visibility(h, getHandleId(widget)); },
        bindForEach(parent, fn) { perry_ui_state_bind_foreach(h, getHandleId(parent), fn); },
        onChange(cb) { perry_ui_state_on_change(h, cb); },
    };
    return wrapper;
}

function stateGet(h) {
    const hId = getHandleId(h);
    const s = states.get(hId);
    return s ? s._value : undefined;
}

function stateSet(h, value) {
    const hId = getHandleId(h);
    const s = states.get(hId);
    if (!s) return;
    s._value = value;
    for (const sub of s.subscribers) {
        try { sub(value); } catch(e) { console.error("State subscriber error:", e); }
    }
}

function stateSubscribe(h, fn) {
    const hId = getHandleId(h);
    const s = states.get(hId);
    if (s) s.subscribers.push(fn);
}

// --- CSS Reset ---
const style = document.createElement("style");
style.textContent = `
*, *::before, *::after { box-sizing: border-box; margin: 0; padding: 0; }
html, body { width: 100vw; height: 100vh; overflow: hidden;
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif; }
#perry-root { display: flex; flex-direction: column; width: 100%; flex: 1 1 0%; min-height: 0; overflow: hidden; }
button { cursor: pointer; padding: 6px 16px; border: 1px solid #ccc; border-radius: 6px; background: transparent; font: inherit; color: inherit; }
button:hover { opacity: 0.85; }
button:active { opacity: 0.7; }
input[type="text"], input[type="password"], select, textarea { padding: 6px 10px; border: 1px solid #ccc; border-radius: 6px; font: inherit; }
input[type="range"] { width: 100%; }
hr { border: none; border-top: 1px solid #ddd; margin: 4px 0; }
fieldset { border: 1px solid #ddd; border-radius: 8px; padding: 12px; }
legend { font-weight: 600; padding: 0 6px; }
progress { width: 100%; }
`;
document.head.appendChild(style);

// --- Root ---
let perryRoot = null;
function getRoot() {
    if (!perryRoot) {
        perryRoot = document.getElementById("perry-root");
        if (!perryRoot) {
            perryRoot = document.createElement("div");
            perryRoot.id = "perry-root";
            document.body.appendChild(perryRoot);
        }
    }
    return perryRoot;
}

// --- Widget Creation ---
function perry_ui_app_create(titleOrOpts, width, height) {
    let title, bodyHandle;
    if (typeof titleOrOpts === "object" && titleOrOpts !== null) {
        // Called as App({title, width, height, body})
        title = titleOrOpts.title || "Perry App";
        bodyHandle = titleOrOpts.body;
    } else {
        title = titleOrOpts;
    }
    document.title = title;
    const root = getRoot();
    root.style.width = "100%";
    root.style.minHeight = "0";
    root.style.display = "flex";
    root.style.flexDirection = "column";
    root.style.overflow = "hidden";
    // Append body widget to root if provided
    if (bodyHandle != null) {
        const h = (typeof bodyHandle === "object" && bodyHandle._perryHandle) ? bodyHandle._perryHandle : bodyHandle;
        const bodyEl = handles.get(h);
        if (bodyEl) {
            bodyEl.style.flex = "1 1 0%";
            root.appendChild(bodyEl);
        }
    }
    return wrapWidget(allocHandle(root));
}

function _appendChildren(el, children) {
    if (!children || !Array.isArray(children)) return;
    for (const child of children) {
        const ch = (typeof child === "object" && child !== null && child._perryHandle)
            ? child._perryHandle : child;
        const childEl = handles.get(ch);
        if (childEl) el.appendChild(childEl);
    }
}

function perry_ui_vstack_create(spacing, children) {
    const el = document.createElement("div");
    el.style.display = "flex";
    el.style.flexDirection = "column";
    el.style.gap = spacing + "px";
    const h = allocHandle(el);
    _appendChildren(el, children);
    return wrapWidget(h);
}

function perry_ui_hstack_create(spacing, children) {
    const el = document.createElement("div");
    el.style.display = "flex";
    el.style.flexDirection = "row";
    el.style.gap = spacing + "px";
    el.style.alignItems = "stretch";
    const h = allocHandle(el);
    _appendChildren(el, children);
    return wrapWidget(h);
}

function perry_ui_zstack_create(children) {
    const el = document.createElement("div");
    el.style.position = "relative";
    const h = allocHandle(el);
    _appendChildren(el, children);
    return wrapWidget(h);
}

function perry_ui_text_create(text) {
    const el = document.createElement("span");
    el.textContent = text;
    return wrapWidget(allocHandle(el));
}

function perry_ui_button_create(label, callback) {
    const el = document.createElement("button");
    el.textContent = label;
    if (typeof callback === "function") {
        el.addEventListener("click", callback);
    }
    return wrapWidget(allocHandle(el));
}

function perry_ui_textfield_create(placeholder, callback) {
    const el = document.createElement("input");
    el.type = "text";
    el.placeholder = placeholder || "";
    if (typeof callback === "function") {
        el.addEventListener("input", () => callback(el.value));
    }
    return wrapWidget(allocHandle(el));
}

function perry_ui_securefield_create(placeholder, callback) {
    const el = document.createElement("input");
    el.type = "password";
    el.placeholder = placeholder || "";
    if (typeof callback === "function") {
        el.addEventListener("input", () => callback(el.value));
    }
    return wrapWidget(allocHandle(el));
}

function perry_ui_toggle_create(label, callback) {
    const wrapper = document.createElement("label");
    wrapper.style.display = "flex";
    wrapper.style.alignItems = "center";
    wrapper.style.gap = "8px";
    wrapper.style.cursor = "pointer";
    const input = document.createElement("input");
    input.type = "checkbox";
    wrapper.appendChild(input);
    wrapper.appendChild(document.createTextNode(label || ""));
    if (typeof callback === "function") {
        input.addEventListener("change", () => callback(input.checked ? 1.0 : 0.0));
    }
    wrapper._input = input;
    return wrapWidget(allocHandle(wrapper));
}

function perry_ui_slider_create(min, max, initial, callback) {
    const el = document.createElement("input");
    el.type = "range";
    el.min = min;
    el.max = max;
    el.value = initial;
    el.step = "any";
    if (typeof callback === "function") {
        el.addEventListener("input", () => callback(parseFloat(el.value)));
    }
    return wrapWidget(allocHandle(el));
}

function perry_ui_scrollview_create() {
    const el = document.createElement("div");
    el.style.overflow = "auto";
    el.style.flex = "1";
    return wrapWidget(allocHandle(el));
}

function perry_ui_spacer_create() {
    const el = document.createElement("div");
    el.style.flex = "1";
    return wrapWidget(allocHandle(el));
}

function perry_ui_divider_create() {
    const el = document.createElement("hr");
    return wrapWidget(allocHandle(el));
}

function perry_ui_progressview_create(value) {
    const el = document.createElement("progress");
    el.max = 1;
    el.value = (value != null) ? value : 0;
    return wrapWidget(allocHandle(el));
}

function perry_ui_image_create(src, width, height) {
    const el = document.createElement("img");
    el.src = src || "";
    if (width > 0) el.style.width = width + "px";
    if (height > 0) el.style.height = height + "px";
    el.style.objectFit = "contain";
    return wrapWidget(allocHandle(el));
}

function perry_ui_picker_create(items_json, selected, callback) {
    const el = document.createElement("select");
    let items = [];
    try { items = JSON.parse(items_json); } catch(e) {}
    for (let i = 0; i < items.length; i++) {
        const opt = document.createElement("option");
        opt.value = i;
        opt.textContent = items[i];
        if (i === selected) opt.selected = true;
        el.appendChild(opt);
    }
    if (typeof callback === "function") {
        el.addEventListener("change", () => callback(parseInt(el.value)));
    }
    return wrapWidget(allocHandle(el));
}

function perry_ui_form_create() {
    const el = document.createElement("form");
    el.addEventListener("submit", e => e.preventDefault());
    el.style.display = "flex";
    el.style.flexDirection = "column";
    el.style.gap = "8px";
    return wrapWidget(allocHandle(el));
}

function perry_ui_section_create(title) {
    const el = document.createElement("fieldset");
    if (title) {
        const legend = document.createElement("legend");
        legend.textContent = title;
        el.appendChild(legend);
    }
    el.style.display = "flex";
    el.style.flexDirection = "column";
    el.style.gap = "6px";
    return wrapWidget(allocHandle(el));
}

function perry_ui_navigationstack_create() {
    const el = document.createElement("div");
    el._navStack = [];
    return wrapWidget(allocHandle(el));
}

function perry_ui_canvas_create(width, height) {
    const el = document.createElement("canvas");
    el.width = width;
    el.height = height;
    el._ctx = el.getContext("2d");
    return wrapWidget(allocHandle(el));
}

// --- Child Management ---
function perry_ui_widget_add_child(parent_h, child_h) {
    const parent = getHandle(parent_h);
    const child = getHandle(child_h);
    if (parent && child) parent.appendChild(child);
}

function perry_ui_widget_remove_all_children(h) {
    const el = getHandle(h);
    if (el) {
        while (el.lastChild && el.lastChild.tagName !== "LEGEND") {
            el.removeChild(el.lastChild);
        }
    }
}

// Resolve handle-or-wrapper to int for internal use
function resolveHandle(h) {
    if (typeof h === "object" && h !== null && h._perryHandle) return h._perryHandle;
    return h;
}

// --- Styling ---
function perry_ui_set_background(h, r, g, b, a) {
    const el = getHandle(h);
    if (el) el.style.backgroundColor = `rgba(${Math.round(r*255)},${Math.round(g*255)},${Math.round(b*255)},${a})`;
}

function perry_ui_set_foreground(h, r, g, b, a) {
    const el = getHandle(h);
    if (el) el.style.color = `rgba(${Math.round(r*255)},${Math.round(g*255)},${Math.round(b*255)},${a})`;
}

function perry_ui_set_font_size(h, size) {
    const el = getHandle(h);
    if (el) el.style.fontSize = size + "px";
}

function perry_ui_set_font_weight(h, weight) {
    const el = getHandle(h);
    if (el) el.style.fontWeight = weight === 1 ? "bold" : "normal";
}

function perry_ui_set_font_family(h, family) {
    const el = getHandle(h);
    if (el) el.style.fontFamily = family;
}

function perry_ui_set_padding(h, value) {
    const el = getHandle(h);
    if (el) el.style.padding = value + "px";
}

function perry_ui_set_frame(h, width, height) {
    const el = getHandle(h);
    if (el) {
        if (width > 0) el.style.width = width + "px";
        if (height > 0) el.style.height = height + "px";
    }
}

function perry_ui_set_corner_radius(h, radius) {
    const el = getHandle(h);
    if (el) el.style.borderRadius = radius + "px";
}

function perry_ui_set_border(h, width, r, g, b, a) {
    const el = getHandle(h);
    if (el) el.style.border = `${width}px solid rgba(${Math.round(r*255)},${Math.round(g*255)},${Math.round(b*255)},${a})`;
}

function perry_ui_set_opacity(h, opacity) {
    const el = getHandle(h);
    if (el) el.style.opacity = opacity;
}

function perry_ui_set_enabled(h, enabled) {
    const el = getHandle(h);
    if (el) {
        el.disabled = !enabled;
        el.style.opacity = enabled ? "1" : "0.5";
        el.style.pointerEvents = enabled ? "auto" : "none";
    }
}

function perry_ui_set_tooltip(h, text) {
    const el = getHandle(h);
    if (el) el.title = text;
}

function perry_ui_set_control_size(h, size) {
    const el = getHandle(h);
    if (!el) return;
    const scale = size === 0 ? 0.85 : size === 2 ? 1.2 : 1.0;
    el.style.fontSize = (scale * 100) + "%";
}

// --- Animations ---
function perry_ui_animate_opacity(h, from, to, duration) {
    const el = getHandle(h);
    if (!el) return;
    el.style.opacity = from;
    el.style.transition = `opacity ${duration}s ease`;
    requestAnimationFrame(() => { el.style.opacity = to; });
}

function perry_ui_animate_position(h, fromX, fromY, toX, toY, duration) {
    const el = getHandle(h);
    if (!el) return;
    el.style.position = "relative";
    el.style.left = fromX + "px";
    el.style.top = fromY + "px";
    el.style.transition = `left ${duration}s ease, top ${duration}s ease`;
    requestAnimationFrame(() => { el.style.left = toX + "px"; el.style.top = toY + "px"; });
}

// --- Event Handlers ---
function perry_ui_set_on_click(h, callback) {
    const el = getHandle(h);
    if (el && typeof callback === "function") el.addEventListener("click", callback);
}

function perry_ui_set_on_hover(h, callback) {
    const el = getHandle(h);
    if (!el || typeof callback !== "function") return;
    el.addEventListener("mouseenter", () => callback(1));
    el.addEventListener("mouseleave", () => callback(0));
}

function perry_ui_set_on_double_click(h, callback) {
    const el = getHandle(h);
    if (el && typeof callback === "function") el.addEventListener("dblclick", callback);
}

// --- State Bindings ---
function perry_ui_state_bind_text(stateH, widgetH) {
    const el = getHandle(widgetH);
    if (!el) return;
    stateSubscribe(stateH, (v) => { el.textContent = String(v); });
    el.textContent = String(stateGet(stateH));
}

function perry_ui_state_bind_text_numeric(stateH, widgetH) {
    perry_ui_state_bind_text(stateH, widgetH);
}

function perry_ui_state_bind_slider(stateH, widgetH) {
    const el = getHandle(widgetH);
    if (!el) return;
    stateSubscribe(stateH, (v) => { el.value = v; });
    el.value = stateGet(stateH);
}

function perry_ui_state_bind_toggle(stateH, widgetH) {
    const el = getHandle(widgetH);
    if (!el) return;
    const input = el._input || el.querySelector("input[type=checkbox]");
    if (!input) return;
    stateSubscribe(stateH, (v) => { input.checked = !!v; });
    input.checked = !!stateGet(stateH);
}

function perry_ui_state_bind_visibility(stateH, widgetH) {
    const el = getHandle(widgetH);
    if (!el) return;
    function update(v) { el.style.display = v ? "" : "none"; }
    stateSubscribe(stateH, update);
    update(stateGet(stateH));
}

function perry_ui_state_bind_foreach(stateH, parentH, templateFn) {
    const parent = getHandle(parentH);
    if (!parent || typeof templateFn !== "function") return;
    function update(items) {
        perry_ui_widget_remove_all_children(parentH);
        if (Array.isArray(items)) {
            for (let i = 0; i < items.length; i++) {
                templateFn(items[i], i);
            }
        }
    }
    stateSubscribe(stateH, update);
    update(stateGet(stateH));
}

function perry_ui_state_on_change(stateH, callback) {
    if (typeof callback === "function") {
        stateSubscribe(stateH, callback);
    }
}

// --- System APIs ---
function perry_system_open_url(url) {
    window.open(url, "_blank");
}

function perry_system_is_dark_mode() {
    return window.matchMedia && window.matchMedia("(prefers-color-scheme: dark)").matches ? 1.0 : 0.0;
}

function perry_system_preferences_get(key) {
    return localStorage.getItem(key) || "";
}

function perry_system_preferences_set(key, value) {
    localStorage.setItem(key, value);
}

// --- Canvas Operations ---
function perry_ui_canvas_fill_rect(h, x, y, w, ht) {
    const el = getHandle(h);
    if (el && el._ctx) el._ctx.fillRect(x, y, w, ht);
}

function perry_ui_canvas_stroke_rect(h, x, y, w, ht) {
    const el = getHandle(h);
    if (el && el._ctx) el._ctx.strokeRect(x, y, w, ht);
}

function perry_ui_canvas_clear_rect(h, x, y, w, ht) {
    const el = getHandle(h);
    if (el && el._ctx) el._ctx.clearRect(x, y, w, ht);
}

function perry_ui_canvas_set_fill_color(h, r, g, b, a) {
    const el = getHandle(h);
    if (el && el._ctx) el._ctx.fillStyle = `rgba(${Math.round(r*255)},${Math.round(g*255)},${Math.round(b*255)},${a})`;
}

function perry_ui_canvas_set_stroke_color(h, r, g, b, a) {
    const el = getHandle(h);
    if (el && el._ctx) el._ctx.strokeStyle = `rgba(${Math.round(r*255)},${Math.round(g*255)},${Math.round(b*255)},${a})`;
}

function perry_ui_canvas_begin_path(h) {
    const el = getHandle(h);
    if (el && el._ctx) el._ctx.beginPath();
}

function perry_ui_canvas_move_to(h, x, y) {
    const el = getHandle(h);
    if (el && el._ctx) el._ctx.moveTo(x, y);
}

function perry_ui_canvas_line_to(h, x, y) {
    const el = getHandle(h);
    if (el && el._ctx) el._ctx.lineTo(x, y);
}

function perry_ui_canvas_arc(h, x, y, radius, startAngle, endAngle) {
    const el = getHandle(h);
    if (el && el._ctx) el._ctx.arc(x, y, radius, startAngle, endAngle);
}

function perry_ui_canvas_close_path(h) {
    const el = getHandle(h);
    if (el && el._ctx) el._ctx.closePath();
}

function perry_ui_canvas_fill(h) {
    const el = getHandle(h);
    if (el && el._ctx) el._ctx.fill();
}

function perry_ui_canvas_stroke(h) {
    const el = getHandle(h);
    if (el && el._ctx) el._ctx.stroke();
}

function perry_ui_canvas_set_line_width(h, w) {
    const el = getHandle(h);
    if (el && el._ctx) el._ctx.lineWidth = w;
}

function perry_ui_canvas_fill_text(h, text, x, y) {
    const el = getHandle(h);
    if (el && el._ctx) el._ctx.fillText(text, x, y);
}

function perry_ui_canvas_set_font(h, font) {
    const el = getHandle(h);
    if (el && el._ctx) el._ctx.font = font;
}

// --- App Lifecycle ---
function perry_ui_app_set_body(app_h, root_h) {
    const root = getHandle(app_h);
    const child = getHandle(root_h);
    if (root && child) { root.innerHTML = ""; root.appendChild(child); }
}

function perry_ui_app_set_min_size(app_h, w, h) {
    const root = getHandle(app_h);
    if (root) { root.style.minWidth = w + "px"; root.style.minHeight = h + "px"; }
}

function perry_ui_app_set_max_size(app_h, w, h) {
    const root = getHandle(app_h);
    if (root) { root.style.maxWidth = w + "px"; root.style.maxHeight = h + "px"; }
}

function perry_ui_app_on_activate(callback) {
    if (typeof callback === "function") {
        document.addEventListener("visibilitychange", () => { if (!document.hidden) callback(); });
    }
}

function perry_ui_app_on_terminate(callback) {
    if (typeof callback === "function") {
        window.addEventListener("beforeunload", () => callback());
    }
}

function perry_ui_app_set_timer(interval_ms, callback) {
    if (typeof callback === "function") setInterval(callback, interval_ms);
}

// --- Multi-Window ---
const _windows = new Map();
let _nextWindowId = 1;

function perry_ui_window_create(title, width, height) {
    const overlay = document.createElement("div");
    overlay.style.cssText = "position:fixed;top:0;left:0;right:0;bottom:0;background:rgba(0,0,0,0.3);display:none;z-index:1000;justify-content:center;align-items:center;";
    const win = document.createElement("div");
    win.style.cssText = `background:#fff;border-radius:8px;box-shadow:0 4px 24px rgba(0,0,0,0.2);width:${width}px;min-height:${height}px;padding:16px;position:relative;`;
    if (title) { const t = document.createElement("div"); t.textContent = title; t.style.fontWeight = "bold"; t.style.marginBottom = "8px"; win.appendChild(t); }
    overlay.appendChild(win);
    document.body.appendChild(overlay);
    const id = _nextWindowId++;
    _windows.set(id, { overlay, win, body: null });
    return id;
}

function perry_ui_window_set_body(window_h, widget_h) {
    const w = _windows.get(window_h);
    const child = getHandle(widget_h);
    if (w && child) { w.body = child; w.win.appendChild(child); }
}

function perry_ui_window_show(window_h) {
    const w = _windows.get(window_h);
    if (w) w.overlay.style.display = "flex";
}

function perry_ui_window_close(window_h) {
    const w = _windows.get(window_h);
    if (w) w.overlay.style.display = "none";
}

// --- State (canonical function names) ---
function perry_ui_state_create(initial) { return stateCreate(initial); }
function perry_ui_state_get(h) { return stateGet(h); }
function perry_ui_state_set(h, v) { stateSet(h, v); }

function perry_ui_state_bind_textfield(stateH, widgetH) {
    const el = getHandle(widgetH);
    if (!el) return;
    stateSubscribe(stateH, (v) => { el.value = String(v); });
    el.value = String(stateGet(stateH) || "");
    el.addEventListener("input", () => stateSet(stateH, el.value));
}

// --- Widget Operations ---
function perry_ui_widget_add_child_at(parent_h, child_h, index) {
    const parent = getHandle(parent_h);
    const child = getHandle(child_h);
    if (parent && child) {
        const ref = parent.children[Math.floor(index)] || null;
        parent.insertBefore(child, ref);
    }
}

function perry_ui_set_widget_hidden(h, hidden) {
    const el = getHandle(h);
    if (el) el.style.display = hidden ? "none" : "";
}

function perry_ui_lazyvstack_create(count, renderFn) {
    const scroll = document.createElement("div");
    scroll.style.overflow = "auto"; scroll.style.flex = "1";
    const inner = document.createElement("div");
    inner.style.display = "flex"; inner.style.flexDirection = "column";
    scroll.appendChild(inner);
    scroll._inner = inner; scroll._renderFn = renderFn;
    if (typeof renderFn === "function") {
        for (let i = 0; i < count; i++) renderFn(i);
    }
    return wrapWidget(allocHandle(scroll));
}

function perry_ui_lazyvstack_update(h, count) {
    const el = getHandle(h);
    if (el && el._inner && el._renderFn) {
        el._inner.innerHTML = "";
        for (let i = 0; i < count; i++) el._renderFn(i);
    }
}

// --- Table (DOM <table> implementation) ---
function perry_ui_table_create(rowCount, colCount, renderFn) {
    const scroll = document.createElement("div");
    scroll.style.overflow = "auto"; scroll.style.flex = "1";
    const tbl = document.createElement("table");
    tbl.style.borderCollapse = "collapse"; tbl.style.width = "100%";
    const thead = document.createElement("thead");
    const headerRow = document.createElement("tr");
    for (let c = 0; c < colCount; c++) {
        const th = document.createElement("th");
        th.style.borderBottom = "1px solid #ccc"; th.style.padding = "4px 8px";
        headerRow.appendChild(th);
    }
    thead.appendChild(headerRow);
    const tbody = document.createElement("tbody");
    tbl.appendChild(thead); tbl.appendChild(tbody);
    scroll.appendChild(tbl);
    scroll._tbl = tbl; scroll._thead = thead; scroll._tbody = tbody;
    scroll._colCount = colCount; scroll._renderFn = renderFn;
    scroll._selectedRow = -1; scroll._onRowSelect = null;
    function buildRows(rc) {
        tbody.innerHTML = "";
        for (let r = 0; r < rc; r++) {
            const tr = document.createElement("tr");
            (function(row) {
                tr.onclick = function() {
                    scroll._selectedRow = row;
                    if (typeof scroll._onRowSelect === "function") scroll._onRowSelect(row);
                };
            })(r);
            for (let c = 0; c < colCount; c++) {
                const td = document.createElement("td");
                td.style.padding = "4px 8px"; td.style.borderBottom = "1px solid #eee";
                if (typeof renderFn === "function") renderFn(r, c);
                tr.appendChild(td);
            }
            tbody.appendChild(tr);
        }
    }
    buildRows(rowCount);
    scroll._buildRows = buildRows;
    return wrapWidget(allocHandle(scroll));
}
function perry_ui_table_set_column_header(h, col, title) {
    const el = getHandle(h);
    if (el && el._thead) {
        const ths = el._thead.querySelectorAll("th");
        if (ths[col]) ths[col].textContent = title || "";
    }
}
function perry_ui_table_set_column_width(h, col, width) {
    const el = getHandle(h);
    if (el && el._thead) {
        const ths = el._thead.querySelectorAll("th");
        if (ths[col]) ths[col].style.width = width + "px";
    }
}
function perry_ui_table_update_row_count(h, count) {
    const el = getHandle(h);
    if (el && el._buildRows) el._buildRows(count);
}
function perry_ui_table_set_on_row_select(h, cb) {
    const el = getHandle(h);
    if (el) el._onRowSelect = cb;
}
function perry_ui_table_get_selected_row(h) {
    const el = getHandle(h);
    return el ? el._selectedRow : -1;
}

// --- Text Operations ---
function perry_ui_text_set_string(h, text) {
    const el = getHandle(h);
    if (el) el.textContent = text;
}

function perry_ui_text_set_selectable(h, selectable) {
    const el = getHandle(h);
    if (el) el.style.userSelect = selectable ? "text" : "none";
}

// --- Button Operations ---
function perry_ui_button_set_bordered(h, bordered) {
    const el = getHandle(h);
    if (el) el.style.border = bordered ? "1px solid #ccc" : "none";
}

function perry_ui_button_set_title(h, title) {
    const el = getHandle(h);
    if (el) el.textContent = title;
}

function perry_ui_button_set_text_color(h, r, g, b, a) {
    const el = getHandle(h);
    if (el) el.style.color = `rgba(${Math.round(r*255)},${Math.round(g*255)},${Math.round(b*255)},${a})`;
}

function perry_ui_button_set_image(h, name) {
    const el = getHandle(h);
    if (!el) return;
    // Map SF Symbol names to Unicode icons
    const iconMap = {
        "folder.fill": "\uD83D\uDCC1", "folder": "\uD83D\uDCC2",
        "doc.text": "\uD83D\uDCC4", "doc.on.doc": "\uD83D\uDDC2\uFE0F",
        "magnifyingglass": "\uD83D\uDD0D", "gearshape": "\u2699\uFE0F",
        "arrow.triangle.branch": "\uD83D\uDD00", "ladybug": "\uD83D\uDC1E",
        "puzzlepiece.extension": "\uD83E\uDDE9", "sparkles": "\u2728",
        "terminal": "\u25B6", "xmark": "\u2715", "plus": "+",
        "chevron.right": "\u203A", "chevron.down": "\u2304",
        "square.and.arrow.up": "\u2B06\uFE0F", "trash": "\uD83D\uDDD1\uFE0F",
    };
    const icon = iconMap[name] || name;
    // Prepend icon to existing text or set as content
    const existingText = el.textContent || "";
    if (existingText.trim()) {
        el.textContent = icon + " " + existingText;
    } else {
        el.textContent = icon;
    }
}

function perry_ui_button_set_content_tint_color(h, r, g, b, a) {
    const el = getHandle(h);
    if (el) el.style.color = `rgba(${Math.round(r*255)},${Math.round(g*255)},${Math.round(b*255)},${a})`;
}

function perry_ui_widget_set_width(h, w) {
    const el = getHandle(h);
    if (el) { el.style.width = w + "px"; el.style.minWidth = w + "px"; }
}

function perry_ui_widget_set_hugging(h, priority) {
    const el = getHandle(h);
    if (!el) return;
    if (priority <= 250) {
        el.style.flex = "1 1 0%";
    } else {
        el.style.flex = "0 0 auto";
    }
}

function perry_ui_open_folder_dialog(callback) {
    if (window.showDirectoryPicker) {
        window.showDirectoryPicker().then(function(dirHandle) {
            window.__perryDirHandle = dirHandle;
            window.__perryFileCache = {};
            _buildFileCacheAsync(dirHandle, "/" + dirHandle.name).then(function() {
                return _prefetchAllFiles();
            }).then(function() {
                if (typeof callback === "function") callback("/" + dirHandle.name);
            });
        }).catch(function() { /* user cancelled */ });
    } else {
        // Fallback: input with webkitdirectory
        const input = document.createElement("input");
        input.type = "file";
        input.webkitdirectory = true;
        input.addEventListener("change", function() {
            if (input.files.length > 0 && typeof callback === "function") {
                const rootName = input.files[0].webkitRelativePath.split("/")[0];
                window.__perryFileCache = {};
                _buildFileCacheFromFileList(input.files, rootName);
                _prefetchAllFilesFromFileList(input.files).then(function() {
                    callback("/" + rootName);
                });
            }
        });
        input.click();
    }
}

// Eagerly cache directory structure from File System Access API
async function _buildFileCacheAsync(dirHandle, path) {
    if (!window.__perryFileCache) window.__perryFileCache = {};
    const children = [];
    for await (const [name, handle] of dirHandle.entries()) {
        const childPath = path + "/" + name;
        const isDir = handle.kind === "directory";
        children.push(name);
        window.__perryFileCache[childPath] = { isDir: isDir, handle: handle, content: null, children: isDir ? [] : null };
        if (isDir) {
            await _buildFileCacheAsync(handle, childPath);
        }
    }
    if (window.__perryFileCache[path]) {
        window.__perryFileCache[path].children = children;
    } else {
        window.__perryFileCache[path] = { isDir: true, handle: dirHandle, content: null, children: children };
    }
}

// Fallback: build cache from FileList (webkitdirectory)
function _buildFileCacheFromFileList(files, rootName) {
    const dirs = {};
    for (let i = 0; i < files.length; i++) {
        const rel = "/" + files[i].webkitRelativePath;
        const parts = rel.split("/");
        // Register all parent directories
        for (let j = 2; j < parts.length; j++) {
            const dirPath = parts.slice(0, j).join("/");
            if (!dirs[dirPath]) {
                dirs[dirPath] = [];
            }
            const childName = parts[j];
            if (dirs[dirPath].indexOf(childName) < 0) dirs[dirPath].push(childName);
        }
        // Register the file
        window.__perryFileCache[rel] = { isDir: false, handle: files[i], content: null, children: null };
    }
    // Register directories
    for (const dirPath in dirs) {
        if (!window.__perryFileCache[dirPath]) {
            window.__perryFileCache[dirPath] = { isDir: true, handle: null, content: null, children: dirs[dirPath] };
        }
    }
}

// Read all files from a FileList (webkitdirectory fallback)
async function _prefetchAllFilesFromFileList(files) {
    for (var i = 0; i < files.length; i++) {
        var file = files[i];
        var path = "/" + file.webkitRelativePath;
        try {
            var text = await file.text();
            if (window.__perryFileCache[path]) {
                window.__perryFileCache[path].content = text;
            }
        } catch(e) {}
    }
}

function perry_ui_embed_ns_view(handleId) {
    // On web, handleId is an editor handle ID. Look up the DOM element and wrap it.
    const el = getHandle(handleId);
    if (!el) return wrapWidget(allocHandle(document.createElement("div")));
    const container = document.createElement("div");
    container.style.flex = "1 1 0%";
    container.style.display = "flex";
    container.style.overflow = "hidden";
    container.appendChild(el);
    return wrapWidget(allocHandle(container));
}

// --- TextField Operations ---
function perry_ui_textfield_focus(h) {
    const el = getHandle(h);
    if (el) el.focus();
}

function perry_ui_textfield_set_string(h, text) {
    const el = getHandle(h);
    if (el) el.value = text;
}

// --- ScrollView Operations ---
function perry_ui_scrollview_set_child(scroll_h, child_h) {
    const scroll = getHandle(scroll_h);
    const child = getHandle(child_h);
    if (scroll && child) { scroll.innerHTML = ""; scroll.appendChild(child); }
}

function perry_ui_scrollview_scroll_to(scroll_h, child_h) {
    const scroll = getHandle(scroll_h);
    const child = getHandle(child_h);
    if (scroll && child) child.scrollIntoView({ behavior: "smooth", block: "nearest" });
}

function perry_ui_scrollview_get_offset(scroll_h) {
    const el = getHandle(scroll_h);
    return el ? el.scrollTop : 0;
}

function perry_ui_scrollview_set_offset(scroll_h, offset) {
    const el = getHandle(scroll_h);
    if (el) el.scrollTop = offset;
}

// --- Styling ---
function perry_ui_widget_set_background_gradient(h, r1, g1, b1, a1, r2, g2, b2, a2, direction) {
    const el = getHandle(h);
    if (!el) return;
    const c1 = `rgba(${Math.round(r1*255)},${Math.round(g1*255)},${Math.round(b1*255)},${a1})`;
    const c2 = `rgba(${Math.round(r2*255)},${Math.round(g2*255)},${Math.round(b2*255)},${a2})`;
    const dir = direction < 0.5 ? "to bottom" : "to right";
    el.style.background = `linear-gradient(${dir}, ${c1}, ${c2})`;
}

function perry_ui_canvas_fill_gradient(h, r1, g1, b1, a1, r2, g2, b2, a2, direction) {
    const el = getHandle(h);
    if (!el || !el._ctx) return;
    const ctx = el._ctx;
    const grad = direction < 0.5
        ? ctx.createLinearGradient(0, 0, 0, el.height)
        : ctx.createLinearGradient(0, 0, el.width, 0);
    grad.addColorStop(0, `rgba(${Math.round(r1*255)},${Math.round(g1*255)},${Math.round(b1*255)},${a1})`);
    grad.addColorStop(1, `rgba(${Math.round(r2*255)},${Math.round(g2*255)},${Math.round(b2*255)},${a2})`);
    ctx.fillStyle = grad;
    ctx.fillRect(0, 0, el.width, el.height);
}

// --- Layout with Insets ---
function perry_ui_vstack_create_with_insets(spacing, top, left, bottom, right) {
    const el = document.createElement("div");
    el.style.display = "flex"; el.style.flexDirection = "column"; el.style.gap = spacing + "px";
    el.style.padding = `${top}px ${right}px ${bottom}px ${left}px`;
    return wrapWidget(allocHandle(el));
}

function perry_ui_hstack_create_with_insets(spacing, top, left, bottom, right) {
    const el = document.createElement("div");
    el.style.display = "flex"; el.style.flexDirection = "row"; el.style.gap = spacing + "px";
    el.style.alignItems = "center";
    el.style.padding = `${top}px ${right}px ${bottom}px ${left}px`;
    return wrapWidget(allocHandle(el));
}

// --- Navigation ---
function perry_ui_navstack_push(h, body_h) {
    const nav = getHandle(h);
    const body = getHandle(body_h);
    if (!nav || !body) return;
    // Hide current children
    for (const child of nav.children) child.style.display = "none";
    nav.appendChild(body);
    if (!nav._navStack) nav._navStack = [];
    nav._navStack.push(body);
}

function perry_ui_navstack_pop(h) {
    const nav = getHandle(h);
    if (!nav || !nav._navStack || nav._navStack.length <= 1) return;
    const removed = nav._navStack.pop();
    if (removed) removed.style.display = "none";
    const top = nav._navStack[nav._navStack.length - 1];
    if (top) top.style.display = "";
}

// --- Picker Operations ---
function perry_ui_picker_add_item(h, title) {
    const el = getHandle(h);
    if (!el) return;
    const opt = document.createElement("option");
    opt.value = el.children.length;
    opt.textContent = title;
    el.appendChild(opt);
}

function perry_ui_picker_set_selected(h, index) {
    const el = getHandle(h);
    if (el) el.selectedIndex = index;
}

function perry_ui_picker_get_selected(h) {
    const el = getHandle(h);
    return el ? el.selectedIndex : -1;
}

// --- Image Operations ---
function perry_ui_image_create_symbol(name) {
    const el = document.createElement("span");
    el.textContent = name; // Use text as placeholder for symbols
    el.style.fontSize = "24px";
    return wrapWidget(allocHandle(el));
}

function perry_ui_image_set_size(h, width, height) {
    const el = getHandle(h);
    if (el) { el.style.width = width + "px"; el.style.height = height + "px"; }
}

function perry_ui_image_set_tint(h, r, g, b, a) {
    const el = getHandle(h);
    if (el) el.style.color = `rgba(${Math.round(r*255)},${Math.round(g*255)},${Math.round(b*255)},${a})`;
}

// --- ProgressView ---
function perry_ui_progressview_set_value(h, value) {
    const el = getHandle(h);
    if (el) { el.removeAttribute("indeterminate"); el.value = value; }
}

// --- Menus ---
const _menus = new Map();
let _nextMenuId = 1;

function perry_ui_menu_create() {
    const id = _nextMenuId++;
    _menus.set(id, []);
    return id;
}

function perry_ui_menu_add_item(menu_h, title, callback, shortcut) {
    const items = _menus.get(menu_h);
    if (items) items.push({ type: "item", title, callback, shortcut: shortcut || undefined });
}

function perry_ui_menu_add_item_with_shortcut(menu_h, title, callback, shortcut) {
    const items = _menus.get(menu_h);
    if (items) items.push({ type: "item", title, callback, shortcut });
}

function perry_ui_menu_add_separator(menu_h) {
    const items = _menus.get(menu_h);
    if (items) items.push({ type: "separator" });
}

function perry_ui_menu_add_submenu(menu_h, title, submenu_h) {
    const items = _menus.get(menu_h);
    if (items) items.push({ type: "submenu", title, submenu: submenu_h });
}

const _menubars = new Map();
let _nextMenubarId = 1;

function perry_ui_menubar_create() {
    const id = _nextMenubarId++;
    _menubars.set(id, { menus: [] });
    return id;
}

function perry_ui_menubar_add_menu(bar_h, title, menu_h) {
    const bar = _menubars.get(bar_h);
    if (bar) bar.menus.push({ title, menu_h });
}

function perry_ui_menubar_attach(bar_h) {
    const bar = _menubars.get(bar_h);
    if (!bar) return;

    // Remove existing menubar if any
    const old = document.querySelector(".perry-menubar");
    if (old) old.remove();

    const barEl = document.createElement("div");
    barEl.className = "perry-menubar";
    barEl.style.cssText = "display:flex;background:#f0f0f0;border-bottom:1px solid #ccc;padding:0;font-family:system-ui,-apple-system,sans-serif;font-size:13px;user-select:none;position:relative;z-index:10000;";

    let openDropdown = null;
    let openTitle = null;

    function dismissAll() {
        if (openDropdown) { openDropdown.remove(); openDropdown = null; openTitle = null; }
    }

    function renderMenuItems(container, menu_h) {
        const items = _menus.get(menu_h);
        if (!items) return;
        for (const item of items) {
            if (item.type === "separator") {
                const sep = document.createElement("div");
                sep.style.cssText = "height:1px;background:#ccc;margin:4px 0;";
                container.appendChild(sep);
            } else if (item.type === "submenu") {
                const mi = document.createElement("div");
                mi.style.cssText = "padding:4px 24px 4px 16px;cursor:pointer;display:flex;justify-content:space-between;white-space:nowrap;position:relative;";
                mi.innerHTML = `<span>${item.title}</span><span style="margin-left:16px;color:#999;">▸</span>`;
                mi.addEventListener("mouseenter", () => {
                    mi.style.background = "#0066ff"; mi.style.color = "#fff";
                    // Show submenu
                    let sub = mi.querySelector(".perry-submenu");
                    if (!sub) {
                        sub = document.createElement("div");
                        sub.className = "perry-submenu";
                        sub.style.cssText = "position:absolute;left:100%;top:0;background:#fff;border:1px solid #ccc;border-radius:4px;box-shadow:0 2px 8px rgba(0,0,0,0.15);padding:4px 0;min-width:120px;color:#000;";
                        renderMenuItems(sub, item.submenu);
                        mi.appendChild(sub);
                    }
                    sub.style.display = "block";
                });
                mi.addEventListener("mouseleave", () => {
                    mi.style.background = ""; mi.style.color = "";
                    const sub = mi.querySelector(".perry-submenu");
                    if (sub) sub.style.display = "none";
                });
                container.appendChild(mi);
            } else {
                const mi = document.createElement("div");
                mi.style.cssText = "padding:4px 24px 4px 16px;cursor:pointer;display:flex;justify-content:space-between;white-space:nowrap;";
                const label = document.createElement("span");
                label.textContent = item.title;
                mi.appendChild(label);
                if (item.shortcut) {
                    const sc = document.createElement("span");
                    sc.textContent = item.shortcut;
                    sc.style.cssText = "margin-left:24px;color:#999;font-size:12px;";
                    mi.appendChild(sc);
                }
                mi.addEventListener("mouseenter", () => { mi.style.background = "#0066ff"; mi.style.color = "#fff"; });
                mi.addEventListener("mouseleave", () => { mi.style.background = ""; mi.style.color = ""; });
                mi.addEventListener("click", () => { dismissAll(); if (typeof item.callback === "function") item.callback(); });
                container.appendChild(mi);
            }
        }
    }

    for (const { title, menu_h } of bar.menus) {
        const titleEl = document.createElement("div");
        titleEl.textContent = title;
        titleEl.style.cssText = "padding:4px 10px;cursor:pointer;";
        titleEl.addEventListener("mouseenter", () => {
            titleEl.style.background = "#ddd";
            if (openDropdown && openTitle !== titleEl) {
                dismissAll();
                showDropdown(titleEl, menu_h);
            }
        });
        titleEl.addEventListener("mouseleave", () => { if (openTitle !== titleEl) titleEl.style.background = ""; });
        titleEl.addEventListener("click", (e) => {
            e.stopPropagation();
            if (openTitle === titleEl) { dismissAll(); titleEl.style.background = ""; return; }
            dismissAll();
            showDropdown(titleEl, menu_h);
        });
        barEl.appendChild(titleEl);

        function showDropdown(el, mh) {
            const dd = document.createElement("div");
            dd.style.cssText = "position:absolute;top:100%;background:#fff;border:1px solid #ccc;border-radius:4px;box-shadow:0 2px 8px rgba(0,0,0,0.15);padding:4px 0;min-width:180px;z-index:10001;";
            dd.style.left = el.offsetLeft + "px";
            renderMenuItems(dd, mh);
            barEl.appendChild(dd);
            openDropdown = dd;
            openTitle = el;
            el.style.background = "#ddd";
        }
    }

    document.addEventListener("click", () => {
        dismissAll();
        barEl.querySelectorAll(":scope > div").forEach(d => d.style.background = "");
    });

    // Insert at top of body, make body a flex column so menubar + root share viewport
    document.body.style.display = "flex";
    document.body.style.flexDirection = "column";
    barEl.style.flex = "0 0 auto";
    const root = document.getElementById("perry-root");
    if (root) root.style.flex = "1 1 0%";
    document.body.insertBefore(barEl, document.body.firstChild);
}

function perry_ui_widget_set_context_menu(widget_h, menu_h) {
    const el = getHandle(widget_h);
    const items = _menus.get(menu_h);
    if (!el || !items) return;
    el.addEventListener("contextmenu", (e) => {
        e.preventDefault();
        const menu = document.createElement("div");
        menu.style.cssText = "position:fixed;background:#fff;border:1px solid #ccc;border-radius:4px;box-shadow:0 2px 8px rgba(0,0,0,0.15);z-index:9999;padding:4px 0;";
        menu.style.left = e.clientX + "px"; menu.style.top = e.clientY + "px";
        for (const item of items) {
            if (item.type === "separator") {
                const sep = document.createElement("div");
                sep.style.cssText = "height:1px;background:#ccc;margin:4px 0;";
                menu.appendChild(sep);
                continue;
            }
            const mi = document.createElement("div");
            mi.textContent = item.title;
            mi.style.cssText = "padding:4px 16px;cursor:pointer;";
            mi.addEventListener("mouseenter", () => mi.style.background = "#f0f0f0");
            mi.addEventListener("mouseleave", () => mi.style.background = "");
            mi.addEventListener("click", () => { menu.remove(); if (typeof item.callback === "function") item.callback(); });
            menu.appendChild(mi);
        }
        document.body.appendChild(menu);
        const dismiss = () => { menu.remove(); document.removeEventListener("click", dismiss); };
        setTimeout(() => document.addEventListener("click", dismiss), 0);
    });
}

// --- Clipboard ---
function perry_ui_clipboard_read() {
    // Clipboard API is async; return empty for now
    return "";
}

function perry_ui_clipboard_write(text) {
    if (navigator.clipboard) navigator.clipboard.writeText(text);
}

// --- Dialogs ---
function perry_ui_open_file_dialog(callback) {
    const input = document.createElement("input");
    input.type = "file";
    input.addEventListener("change", () => {
        if (input.files.length > 0 && typeof callback === "function") callback(input.files[0].name);
    });
    input.click();
}

function perry_ui_save_file_dialog(callback, defaultName) {
    const name = prompt("Save as:", defaultName || "file.txt");
    if (name && typeof callback === "function") callback(name);
}

function perry_ui_alert(title, message, buttons, callback) {
    const result = window.confirm(title + "\n\n" + message);
    if (typeof callback === "function") callback(result ? 0 : 1);
}

// --- Keyboard Shortcuts ---
function perry_ui_add_keyboard_shortcut(key, modifiers, callback) {
    if (typeof callback !== "function") return;
    document.addEventListener("keydown", (e) => {
        const wantMeta = (modifiers & 1) !== 0;
        const wantShift = (modifiers & 2) !== 0;
        const wantAlt = (modifiers & 4) !== 0;
        if (e.key.toLowerCase() === key.toLowerCase() &&
            e.metaKey === wantMeta && e.shiftKey === wantShift && e.altKey === wantAlt) {
            e.preventDefault(); callback();
        }
    });
}

// --- Sheet (Modal) ---
const _sheets = new Map();
let _nextSheetId = 1;

function perry_ui_sheet_create(width, height, title) {
    const id = _nextSheetId++;
    const overlay = document.createElement("div");
    overlay.style.cssText = "position:fixed;top:0;left:0;right:0;bottom:0;background:rgba(0,0,0,0.4);display:none;z-index:2000;justify-content:center;align-items:center;";
    const sheet = document.createElement("div");
    sheet.style.cssText = `background:#fff;border-radius:12px;box-shadow:0 8px 32px rgba(0,0,0,0.25);width:${width}px;min-height:${height}px;padding:16px;`;
    overlay.appendChild(sheet);
    document.body.appendChild(overlay);
    _sheets.set(id, { overlay, sheet });
    return id;
}

function perry_ui_sheet_present(sheet_h) {
    const s = _sheets.get(sheet_h);
    if (s) s.overlay.style.display = "flex";
}

function perry_ui_sheet_dismiss(sheet_h) {
    const s = _sheets.get(sheet_h);
    if (s) s.overlay.style.display = "none";
}

// --- Toolbar ---
const _toolbars = new Map();
let _nextToolbarId = 1;

function perry_ui_toolbar_create() {
    const id = _nextToolbarId++;
    const bar = document.createElement("div");
    bar.style.cssText = "display:flex;gap:8px;padding:8px;background:#f5f5f5;border-bottom:1px solid #ddd;";
    _toolbars.set(id, bar);
    return id;
}

function perry_ui_toolbar_add_item(toolbar_h, label, icon, callback) {
    const bar = _toolbars.get(toolbar_h);
    if (!bar) return;
    const btn = document.createElement("button");
    btn.textContent = label || icon || "";
    btn.style.cssText = "padding:4px 12px;border:1px solid #ccc;border-radius:4px;background:#fff;cursor:pointer;font:inherit;";
    if (typeof callback === "function") btn.addEventListener("click", callback);
    bar.appendChild(btn);
}

function perry_ui_toolbar_attach(toolbar_h) {
    const bar = _toolbars.get(toolbar_h);
    if (bar) { const root = getRoot(); root.insertBefore(bar, root.firstChild); }
}

// --- System: Keychain (localStorage) ---
function perry_system_keychain_save(key, value) {
    localStorage.setItem("perry_keychain_" + key, value);
}

function perry_system_keychain_get(key) {
    return localStorage.getItem("perry_keychain_" + key) || "";
}

function perry_system_keychain_delete(key) {
    localStorage.removeItem("perry_keychain_" + key);
}

// --- System: Notifications ---
function perry_system_notification_send(title, body) {
    if ("Notification" in window && Notification.permission === "granted") {
        new Notification(title, { body: body });
    } else if ("Notification" in window) {
        Notification.requestPermission().then(p => { if (p === "granted") new Notification(title, { body: body }); });
    }
}

// --- Run App ---
function perry_ui_app_run() {
    // In browser, the app is already "running" once DOM is ready.
    // This is a no-op.
}

// --- Timer Functions ---
function perry_set_timeout(callback, ms) {
    return setTimeout(callback, ms);
}

function perry_set_interval(callback, ms) {
    return setInterval(callback, ms);
}

function perry_clear_timeout(id) {
    clearTimeout(id);
}

function perry_clear_interval(id) {
    clearInterval(id);
}

// --- Path Helpers (simplified browser versions) ---
const __path = {
    join: function(...parts) {
        return parts.join("/").replace(/\/+/g, "/");
    },
    dirname: function(p) {
        const i = p.lastIndexOf("/");
        return i >= 0 ? p.substring(0, i) : ".";
    },
    basename: function(p) {
        const i = p.lastIndexOf("/");
        return i >= 0 ? p.substring(i + 1) : p;
    },
    extname: function(p) {
        const b = __path.basename(p);
        const i = b.lastIndexOf(".");
        return i > 0 ? b.substring(i) : "";
    },
    resolve: function(...parts) {
        return __path.join(...parts);
    },
    isAbsolute: function(p) {
        return p.startsWith("/");
    }
};

// ==========================================================================
// Hone Editor FFI — DOM-based implementations of the 28 native FFI functions
// ==========================================================================

const _honeEditors = new Map(); // handle → editor state
let _honeEditorNextHandle = 1;
let _honeEditorCSSInjected = false;

function _injectEditorCSS() {
    if (_honeEditorCSSInjected) return;
    _honeEditorCSSInjected = true;
    const s = document.createElement("style");
    s.textContent = `
.hone-editor { position: relative; overflow: hidden; contain: strict; outline: none;
  font-variant-ligatures: contextual; -webkit-font-smoothing: antialiased;
  cursor: text; }
.hone-editor-lines { position: absolute; top: 0; left: 0; right: 0; bottom: 0; }
.hone-editor-line { position: absolute; left: 0; right: 0; white-space: pre;
  pointer-events: none; }
.hone-editor-cursor-el { position: absolute; width: 2px; pointer-events: none;
  animation: hone-blink 1s step-end infinite; }
.hone-editor-selection-rect { position: absolute; pointer-events: none; }
.hone-editor-ghost { position: absolute; pointer-events: none; white-space: pre; }
.hone-editor-decoration { position: absolute; pointer-events: none; }
@keyframes hone-blink { 0%,100% { opacity: 1; } 50% { opacity: 0; } }
`;
    document.head.appendChild(s);
}

function hone_editor_create(width, height) {
    _injectEditorCSS();
    const h = _honeEditorNextHandle++;
    const root = document.createElement("div");
    root.className = "hone-editor";
    root.tabIndex = 0;
    root.style.width = width + "px";
    root.style.height = height + "px";
    root.style.flex = "1 1 0%";

    const linesContainer = document.createElement("div");
    linesContainer.className = "hone-editor-lines";
    root.appendChild(linesContainer);

    // Measurement canvas (hidden, for text metrics)
    const canvas = document.createElement("canvas");
    const ctx = canvas.getContext("2d");

    // Cursor element
    const cursorEl = document.createElement("div");
    cursorEl.className = "hone-editor-cursor-el";
    cursorEl.style.background = "#d4d4d4";
    root.appendChild(cursorEl);

    // Selection container
    const selContainer = document.createElement("div");
    selContainer.style.position = "absolute";
    selContainer.style.top = "0";
    selContainer.style.left = "0";
    selContainer.style.right = "0";
    selContainer.style.bottom = "0";
    selContainer.style.pointerEvents = "none";
    root.insertBefore(selContainer, linesContainer);

    const editor = {
        root: root,
        linesContainer: linesContainer,
        canvas: canvas,
        ctx: ctx,
        cursorEl: cursorEl,
        selContainer: selContainer,
        linePool: [],       // reusable line <div> elements
        activeLines: {},    // lineNum → <div>
        fontFamily: "JetBrains Mono, Menlo, Monaco, Courier New, monospace",
        fontSize: 14,
        charWidth: 8.4,
        lineHeight: 21,
        gutterWidth: 0,
        scrollOffsetY: 0,
        pendingEvents: [],
        cursorEls: [cursorEl],
        ghostEl: null,
        batchMode: false,
    };

    // Set initial font on canvas for measurement
    ctx.font = editor.fontSize + "px " + editor.fontFamily;
    editor.charWidth = ctx.measureText("M").width;

    // Set up event listeners
    _setupEditorEvents(root, editor);

    _honeEditors.set(h, editor);
    // Also store in the handle system for embedNSView
    handles.set(h + 100000, root);
    return h;
}

function _setupEditorEvents(root, editor) {
    root.addEventListener("keydown", function(e) {
        if (e.key.length === 1 && !e.ctrlKey && !e.metaKey) {
            // Printable character → TEXT event
            editor.pendingEvents.push({ type: 1, char: e.key.charCodeAt(0), action: 0, x: 0, y: 0 });
        } else {
            var actionId = _mapKeyToAction(e.key, e.ctrlKey, e.shiftKey, e.metaKey, e.altKey);
            if (actionId > 0) {
                editor.pendingEvents.push({ type: 2, char: 0, action: actionId, x: 0, y: 0 });
            }
        }
        e.preventDefault();
    });

    root.addEventListener("wheel", function(e) {
        editor.pendingEvents.push({ type: 3, char: 0, action: 0, x: e.deltaX, y: e.deltaY });
        e.preventDefault();
    }, { passive: false });

    root.addEventListener("mousedown", function(e) {
        var rect = root.getBoundingClientRect();
        editor.pendingEvents.push({ type: 4, char: 0, action: 0, x: e.clientX - rect.left, y: e.clientY - rect.top });
        root.focus();
    });

    // Handle paste
    root.addEventListener("paste", function(e) {
        var text = (e.clipboardData || window.clipboardData).getData("text");
        for (var i = 0; i < text.length; i++) {
            var ch = text.charCodeAt(i);
            if (ch === 10) {
                editor.pendingEvents.push({ type: 2, char: 0, action: 9, x: 0, y: 0 }); // Enter
            } else if (ch !== 13) {
                editor.pendingEvents.push({ type: 1, char: ch, action: 0, x: 0, y: 0 });
            }
        }
        e.preventDefault();
    });
}

function _mapKeyToAction(key, ctrl, shift, meta, alt) {
    // cmd/ctrl key combos
    var mod = meta || ctrl;
    if (key === "ArrowLeft" && shift) return 15;
    if (key === "ArrowRight" && shift) return 16;
    if (key === "ArrowUp" && shift) return 17;
    if (key === "ArrowDown" && shift) return 18;
    if (key === "ArrowLeft" && alt) return 13;
    if (key === "ArrowRight" && alt) return 14;
    if (key === "ArrowLeft") return 1;
    if (key === "ArrowRight") return 2;
    if (key === "ArrowUp") return 3;
    if (key === "ArrowDown") return 4;
    if (key === "Home" && shift) return 19;
    if (key === "End" && shift) return 20;
    if (key === "Home") return 5;
    if (key === "End") return 6;
    if (key === "Backspace" && alt) return 27;
    if (key === "Backspace") return 10;
    if (key === "Delete") return 11;
    if (key === "Enter") return 9;
    if (key === "Tab") return 12;
    if (key === "PageUp") return 28;
    if (key === "PageDown") return 29;
    if (mod && key === "a") return 21;
    if (mod && key === "x") return 22;
    if (mod && key === "c") return 23;
    if (mod && key === "v") return 24;
    if (mod && key === "z" && shift) return 26;
    if (mod && key === "z") return 25;
    return 0;
}

function hone_editor_destroy(h) {
    var ed = _honeEditors.get(h);
    if (ed && ed.root.parentNode) ed.root.parentNode.removeChild(ed.root);
    _honeEditors.delete(h);
    handles.delete(h + 100000);
}

function hone_editor_attach_to_view(h, parentId) {
    var ed = _honeEditors.get(h);
    var parent = getHandle(parentId);
    if (ed && parent) parent.appendChild(ed.root);
}

function hone_editor_set_font(h, family, size) {
    var ed = _honeEditors.get(h);
    if (!ed) return;
    if (typeof family === "string" && family.length > 0) {
        ed.fontFamily = family + ", Menlo, Monaco, Courier New, monospace";
    }
    ed.fontSize = size;
    ed.lineHeight = Math.round(size * 1.5);
    ed.ctx.font = size + "px " + ed.fontFamily;
    ed.charWidth = ed.ctx.measureText("M").width;
    ed.root.style.fontFamily = ed.fontFamily;
    ed.root.style.fontSize = size + "px";
    ed.root.style.lineHeight = ed.lineHeight + "px";
}

function hone_editor_measure_text(h, text) {
    var ed = _honeEditors.get(h);
    if (!ed) return 0;
    if (typeof text !== "string") return 0;
    return ed.ctx.measureText(text).width;
}

function hone_editor_render_line(h, lineNumber, text, tokensJson, yOffset) {
    var ed = _honeEditors.get(h);
    if (!ed) return;

    var lineEl = ed.activeLines[lineNumber];
    if (!lineEl) {
        lineEl = ed.linePool.length > 0 ? ed.linePool.pop() : document.createElement("div");
        lineEl.className = "hone-editor-line";
        lineEl.style.fontFamily = ed.fontFamily;
        lineEl.style.fontSize = ed.fontSize + "px";
        lineEl.style.lineHeight = ed.lineHeight + "px";
        ed.linesContainer.appendChild(lineEl);
        ed.activeLines[lineNumber] = lineEl;
    }

    lineEl.style.top = yOffset + "px";
    lineEl.innerHTML = "";

    // Parse tokens JSON and render spans
    var tokens = null;
    if (typeof tokensJson === "string" && tokensJson.length > 2) {
        try { tokens = JSON.parse(tokensJson); } catch(e) {}
    }

    if (tokens && tokens.length > 0) {
        for (var i = 0; i < tokens.length; i++) {
            var tok = tokens[i];
            var span = document.createElement("span");
            if (tok.color) span.style.color = tok.color;
            if (tok.italic) span.style.fontStyle = "italic";
            if (tok.bold) span.style.fontWeight = "bold";
            var startPos = tok.start || 0;
            var endPos = tok.end || text.length;
            span.textContent = text.substring(startPos, endPos);
            lineEl.appendChild(span);
        }
    } else {
        lineEl.textContent = text;
    }
}

function hone_editor_set_cursor(h, x, y, cursorStyle) {
    var ed = _honeEditors.get(h);
    if (!ed) return;
    var el = ed.cursorEl;
    el.style.left = x + "px";
    el.style.top = y + "px";
    el.style.height = ed.lineHeight + "px";
    if (cursorStyle === 1) {
        // Block cursor
        el.style.width = ed.charWidth + "px";
        el.style.background = "rgba(212, 212, 212, 0.4)";
    } else if (cursorStyle === 2) {
        // Underline cursor
        el.style.width = ed.charWidth + "px";
        el.style.height = "2px";
        el.style.top = (y + ed.lineHeight - 2) + "px";
        el.style.background = "#d4d4d4";
    } else {
        // Line cursor (default)
        el.style.width = "2px";
        el.style.background = "#d4d4d4";
    }
}

function hone_editor_set_cursors(h, cursorsJson) {
    var ed = _honeEditors.get(h);
    if (!ed) return;
    var cursors = null;
    try { cursors = JSON.parse(cursorsJson); } catch(e) { return; }
    if (!cursors) return;

    // Ensure we have enough cursor elements
    while (ed.cursorEls.length < cursors.length) {
        var cel = document.createElement("div");
        cel.className = "hone-editor-cursor-el";
        cel.style.background = "#d4d4d4";
        ed.root.appendChild(cel);
        ed.cursorEls.push(cel);
    }
    // Hide extra cursors
    for (var i = cursors.length; i < ed.cursorEls.length; i++) {
        ed.cursorEls[i].style.display = "none";
    }
    // Position visible cursors
    for (var j = 0; j < cursors.length; j++) {
        var c = cursors[j];
        var el = ed.cursorEls[j];
        el.style.display = "";
        el.style.left = c.x + "px";
        el.style.top = c.y + "px";
        el.style.height = ed.lineHeight + "px";
        el.style.width = "2px";
    }
}

function hone_editor_set_selection(h, regionsJson) {
    var ed = _honeEditors.get(h);
    if (!ed) return;
    var regions = null;
    if (typeof regionsJson === "string" && regionsJson.length > 2) {
        try { regions = JSON.parse(regionsJson); } catch(e) {}
    }

    ed.selContainer.innerHTML = "";
    if (!regions || regions.length === 0) return;

    for (var i = 0; i < regions.length; i++) {
        var r = regions[i];
        var rect = document.createElement("div");
        rect.className = "hone-editor-selection-rect";
        rect.style.left = r.x + "px";
        rect.style.top = r.y + "px";
        rect.style.width = r.w + "px";
        rect.style.height = r.h + "px";
        rect.style.background = "rgba(38, 79, 122, 0.3)";
        ed.selContainer.appendChild(rect);
    }
}

function hone_editor_scroll(h, offsetY) {
    var ed = _honeEditors.get(h);
    if (!ed) return;
    ed.scrollOffsetY = offsetY;
    ed.linesContainer.style.transform = "translateY(" + (-offsetY) + "px)";
}

function hone_editor_begin_frame(h) {
    var ed = _honeEditors.get(h);
    if (!ed) return;
    ed.batchMode = true;
    // Clear stale lines at the start of a new frame
    for (var ln in ed.activeLines) {
        var lineEl = ed.activeLines[ln];
        lineEl.style.display = "none";
        ed.linePool.push(lineEl);
    }
    ed.activeLines = {};
}

function hone_editor_end_frame(h) {
    var ed = _honeEditors.get(h);
    if (!ed) return;
    ed.batchMode = false;
    // Show all active lines
    for (var ln in ed.activeLines) {
        ed.activeLines[ln].style.display = "";
    }
}

function hone_editor_invalidate(h) {
    // No-op on web — rendering is immediate via DOM manipulation
}

function hone_editor_render_ghost_text(h, text, x, y, color) {
    var ed = _honeEditors.get(h);
    if (!ed) return;
    if (!ed.ghostEl) {
        ed.ghostEl = document.createElement("div");
        ed.ghostEl.className = "hone-editor-ghost";
        ed.root.appendChild(ed.ghostEl);
    }
    ed.ghostEl.style.left = x + "px";
    ed.ghostEl.style.top = y + "px";
    ed.ghostEl.style.color = typeof color === "string" ? color : "rgba(128,128,128,0.5)";
    ed.ghostEl.style.fontFamily = ed.fontFamily;
    ed.ghostEl.style.fontSize = ed.fontSize + "px";
    ed.ghostEl.textContent = text;
    ed.ghostEl.style.display = "";
}

function hone_editor_render_decorations(h, decorationsJson) {
    var ed = _honeEditors.get(h);
    if (!ed) return;
    // Remove existing decorations
    var existing = ed.root.querySelectorAll(".hone-editor-decoration");
    for (var i = 0; i < existing.length; i++) existing[i].remove();

    var decorations = null;
    try { decorations = JSON.parse(decorationsJson); } catch(e) { return; }
    if (!decorations) return;

    for (var j = 0; j < decorations.length; j++) {
        var d = decorations[j];
        var el = document.createElement("div");
        el.className = "hone-editor-decoration";
        el.style.left = d.x + "px";
        el.style.top = d.y + "px";
        el.style.width = d.w + "px";
        el.style.height = d.h + "px";
        if (d.background) el.style.background = d.background;
        if (d.borderBottom) el.style.borderBottom = d.borderBottom;
        ed.root.appendChild(el);
    }
}

function hone_editor_set_gutter_width(h, width) {
    var ed = _honeEditors.get(h);
    if (!ed) return;
    ed.gutterWidth = width;
    ed.linesContainer.style.paddingLeft = width + "px";
}

function hone_editor_set_ts_mode(h, mode) {
    // On web, TypeScript always handles all state — this is a no-op
}

function hone_editor_set_event_callback(h, cb) {
    // Not used in Perry poll mode — no-op on web
}

function hone_editor_nsview(h) {
    // On web, return a handle ID that embed_ns_view can look up
    return h + 100000;
}

function hone_editor_pending_event_count(h) {
    var ed = _honeEditors.get(h);
    return ed ? ed.pendingEvents.length : 0;
}

function hone_editor_get_event_type(h, i) {
    var ed = _honeEditors.get(h);
    return (ed && i < ed.pendingEvents.length) ? ed.pendingEvents[i].type : 0;
}

function hone_editor_get_event_char(h, i) {
    var ed = _honeEditors.get(h);
    return (ed && i < ed.pendingEvents.length) ? ed.pendingEvents[i].char : 0;
}

function hone_editor_get_event_action(h, i) {
    var ed = _honeEditors.get(h);
    return (ed && i < ed.pendingEvents.length) ? ed.pendingEvents[i].action : 0;
}

function hone_editor_get_event_x(h, i) {
    var ed = _honeEditors.get(h);
    return (ed && i < ed.pendingEvents.length) ? ed.pendingEvents[i].x : 0;
}

function hone_editor_get_event_y(h, i) {
    var ed = _honeEditors.get(h);
    return (ed && i < ed.pendingEvents.length) ? ed.pendingEvents[i].y : 0;
}

function hone_editor_clear_events(h) {
    var ed = _honeEditors.get(h);
    if (ed) ed.pendingEvents.length = 0;
}

// --- Platform FFI stubs (web implementations) ---
function perry_get_screen_width()  { return window.innerWidth; }
function perry_get_screen_height() { return window.innerHeight; }
function perry_get_scale_factor()  { return window.devicePixelRatio || 1; }
function perry_has_hardware_keyboard() { return true; }
function perry_get_platform() { return "web"; }
function perry_get_orientation() { return window.innerWidth > window.innerHeight ? "landscape" : "portrait"; }
function perry_on_resize(cb) {
    window.addEventListener("resize", function() { cb(window.innerWidth, window.innerHeight); });
}
function perry_on_orientation_change(cb) {
    if (window.matchMedia) {
        window.matchMedia("(orientation: portrait)").addEventListener("change", function(e) {
            cb(e.matches ? "portrait" : "landscape");
        });
    }
}

// --- File System Web Cache ---
// Files are cached when a folder is opened via perry_ui_open_folder_dialog.
// readFileSync/readdirSync/isDirectory serve from this cache.
if (!window.__perryFileCache) window.__perryFileCache = {};

function fs_readFileSync(path) {
    const entry = window.__perryFileCache[path];
    if (entry && entry.content !== null) return entry.content;
    if (entry && entry.handle && entry.handle.getFile) {
        // Async read - but we need sync. Return empty and prefetch.
        _prefetchFile(path, entry.handle);
        return entry.content || "";
    }
    return "";
}

function fs_readdirSync(path) {
    const entry = window.__perryFileCache[path];
    if (entry && entry.children) return entry.children;
    return [];
}

function fs_isDirectory(path) {
    const entry = window.__perryFileCache[path];
    if (entry) return entry.isDir ? 1 : 0;
    return 0;
}

function fs_existsSync(path) {
    return window.__perryFileCache[path] ? true : false;
}

// Pre-fetch file content asynchronously and cache it
async function _prefetchFile(path, handle) {
    try {
        const file = await handle.getFile();
        const text = await file.text();
        if (window.__perryFileCache[path]) {
            window.__perryFileCache[path].content = text;
        }
    } catch(e) {
        console.warn("Failed to read file:", path, e);
    }
}

// Pre-fetch all files in the cache that haven't been loaded yet
async function _prefetchAllFiles() {
    for (const path in window.__perryFileCache) {
        const entry = window.__perryFileCache[path];
        if (!entry.isDir && entry.content === null && entry.handle) {
            await _prefetchFile(path, entry.handle);
        }
    }
}

// --- Expose API ---
window.__perry = {
    // Handle system
    allocHandle, getHandle,
    // State
    stateCreate, stateGet, stateSet, stateSubscribe,
    // UI widgets
    perry_ui_app_create,
    perry_ui_vstack_create,
    perry_ui_hstack_create,
    perry_ui_zstack_create,
    perry_ui_text_create,
    perry_ui_button_create,
    perry_ui_textfield_create,
    perry_ui_securefield_create,
    perry_ui_toggle_create,
    perry_ui_slider_create,
    perry_ui_scrollview_create,
    perry_ui_spacer_create,
    perry_ui_divider_create,
    perry_ui_progressview_create,
    perry_ui_image_create,
    perry_ui_picker_create,
    perry_ui_form_create,
    perry_ui_section_create,
    perry_ui_navigationstack_create,
    perry_ui_canvas_create,
    perry_ui_lazyvstack_create,
    perry_ui_lazyvstack_update,
    perry_ui_table_create,
    perry_ui_table_set_column_header,
    perry_ui_table_set_column_width,
    perry_ui_table_update_row_count,
    perry_ui_table_set_on_row_select,
    perry_ui_table_get_selected_row,
    // Child management
    perry_ui_widget_add_child,
    perry_ui_widget_add_child_at,
    perry_ui_widget_remove_all_children,
    // Styling
    perry_ui_set_background,
    perry_ui_set_foreground,
    perry_ui_set_font_size,
    perry_ui_set_font_weight,
    perry_ui_set_font_family,
    perry_ui_set_padding,
    perry_ui_set_frame,
    perry_ui_set_corner_radius,
    perry_ui_set_border,
    perry_ui_set_opacity,
    perry_ui_set_enabled,
    perry_ui_set_tooltip,
    perry_ui_set_control_size,
    perry_ui_set_widget_hidden,
    perry_ui_widget_set_background_gradient,
    perry_ui_widget_set_context_menu,
    // Animations
    perry_ui_animate_opacity,
    perry_ui_animate_position,
    // Events
    perry_ui_set_on_click,
    perry_ui_set_on_hover,
    perry_ui_set_on_double_click,
    // State system
    perry_ui_state_create,
    perry_ui_state_get,
    perry_ui_state_set,
    perry_ui_state_bind_textfield,
    // State bindings
    perry_ui_state_bind_text,
    perry_ui_state_bind_text_numeric,
    perry_ui_state_bind_slider,
    perry_ui_state_bind_toggle,
    perry_ui_state_bind_visibility,
    perry_ui_state_bind_foreach,
    perry_ui_state_on_change,
    // Text / Button / TextField ops
    perry_ui_text_set_string,
    perry_ui_text_set_selectable,
    perry_ui_button_set_bordered,
    perry_ui_button_set_title,
    perry_ui_button_set_text_color,
    perry_ui_button_set_image,
    perry_ui_button_set_content_tint_color,
    perry_ui_textfield_focus,
    perry_ui_textfield_set_string,
    // ScrollView
    perry_ui_scrollview_set_child,
    perry_ui_scrollview_scroll_to,
    perry_ui_scrollview_get_offset,
    perry_ui_scrollview_set_offset,
    // Layout
    perry_ui_vstack_create_with_insets,
    perry_ui_hstack_create_with_insets,
    // Navigation
    perry_ui_navstack_push,
    perry_ui_navstack_pop,
    // Picker
    perry_ui_picker_add_item,
    perry_ui_picker_set_selected,
    perry_ui_picker_get_selected,
    // Image
    perry_ui_image_create_symbol,
    perry_ui_image_set_size,
    perry_ui_image_set_tint,
    // ProgressView
    perry_ui_progressview_set_value,
    // System
    perry_system_open_url,
    perry_system_is_dark_mode,
    perry_system_preferences_get,
    perry_system_preferences_set,
    perry_system_keychain_save,
    perry_system_keychain_get,
    perry_system_keychain_delete,
    perry_system_notification_send,
    // Canvas
    perry_ui_canvas_fill_rect,
    perry_ui_canvas_stroke_rect,
    perry_ui_canvas_clear_rect,
    perry_ui_canvas_set_fill_color,
    perry_ui_canvas_set_stroke_color,
    perry_ui_canvas_begin_path,
    perry_ui_canvas_move_to,
    perry_ui_canvas_line_to,
    perry_ui_canvas_arc,
    perry_ui_canvas_close_path,
    perry_ui_canvas_fill,
    perry_ui_canvas_stroke,
    perry_ui_canvas_set_line_width,
    perry_ui_canvas_fill_text,
    perry_ui_canvas_set_font,
    perry_ui_canvas_fill_gradient,
    // Menu
    perry_ui_menu_create,
    perry_ui_menu_add_item,
    perry_ui_menu_add_item_with_shortcut,
    perry_ui_menu_add_separator,
    perry_ui_menu_add_submenu,
    perry_ui_menubar_create,
    perry_ui_menubar_add_menu,
    perry_ui_menubar_attach,
    // Clipboard
    perry_ui_clipboard_read,
    perry_ui_clipboard_write,
    // Dialogs
    perry_ui_open_file_dialog,
    perry_ui_open_folder_dialog,
    perry_ui_save_file_dialog,
    perry_ui_alert,
    // Keyboard
    perry_ui_add_keyboard_shortcut,
    // Sheets
    perry_ui_sheet_create,
    perry_ui_sheet_present,
    perry_ui_sheet_dismiss,
    // Toolbar
    perry_ui_toolbar_create,
    perry_ui_toolbar_add_item,
    perry_ui_toolbar_attach,
    // Windows
    perry_ui_window_create,
    perry_ui_window_set_body,
    perry_ui_window_show,
    perry_ui_window_close,
    // App lifecycle
    perry_ui_app_run,
    perry_ui_app_set_body,
    perry_ui_app_set_min_size,
    perry_ui_app_set_max_size,
    perry_ui_app_on_activate,
    perry_ui_app_on_terminate,
    perry_ui_app_set_timer,
    // Widget layout
    perry_ui_widget_set_width,
    perry_ui_widget_set_hugging,
    perry_ui_embed_ns_view,
    // Timers
    perry_set_timeout,
    perry_set_interval,
    perry_clear_timeout,
    perry_clear_interval,
    // Path
    path: __path,
    // Platform FFI
    perry_get_screen_width,
    perry_get_screen_height,
    perry_get_scale_factor,
    perry_has_hardware_keyboard,
    perry_get_platform,
    perry_get_orientation,
    perry_on_resize,
    perry_on_orientation_change,
    // File System (web cache)
    fs_readFileSync,
    fs_readdirSync,
    fs_isDirectory,
    fs_existsSync,
};

// Expose platform FFI functions as globals (compiled code calls them as bare function names)
window.perry_get_screen_width = perry_get_screen_width;
window.perry_get_screen_height = perry_get_screen_height;
window.perry_get_scale_factor = perry_get_scale_factor;
window.perry_has_hardware_keyboard = perry_has_hardware_keyboard;
window.perry_get_platform = perry_get_platform;
window.perry_get_orientation = perry_get_orientation;
window.perry_on_resize = perry_on_resize;
window.perry_on_orientation_change = perry_on_orientation_change;

// Expose editor FFI functions as globals
window.hone_editor_create = hone_editor_create;
window.hone_editor_destroy = hone_editor_destroy;
window.hone_editor_attach_to_view = hone_editor_attach_to_view;
window.hone_editor_set_font = hone_editor_set_font;
window.hone_editor_measure_text = hone_editor_measure_text;
window.hone_editor_render_line = hone_editor_render_line;
window.hone_editor_set_cursor = hone_editor_set_cursor;
window.hone_editor_set_cursors = hone_editor_set_cursors;
window.hone_editor_set_selection = hone_editor_set_selection;
window.hone_editor_scroll = hone_editor_scroll;
window.hone_editor_begin_frame = hone_editor_begin_frame;
window.hone_editor_end_frame = hone_editor_end_frame;
window.hone_editor_invalidate = hone_editor_invalidate;
window.hone_editor_render_ghost_text = hone_editor_render_ghost_text;
window.hone_editor_render_decorations = hone_editor_render_decorations;
window.hone_editor_set_gutter_width = hone_editor_set_gutter_width;
window.hone_editor_set_ts_mode = hone_editor_set_ts_mode;
window.hone_editor_set_event_callback = hone_editor_set_event_callback;
window.hone_editor_nsview = hone_editor_nsview;
window.hone_editor_pending_event_count = hone_editor_pending_event_count;
window.hone_editor_get_event_type = hone_editor_get_event_type;
window.hone_editor_get_event_char = hone_editor_get_event_char;
window.hone_editor_get_event_action = hone_editor_get_event_action;
window.hone_editor_get_event_x = hone_editor_get_event_x;
window.hone_editor_get_event_y = hone_editor_get_event_y;
window.hone_editor_clear_events = hone_editor_clear_events;

})();
