pub mod console;
pub mod dom;
pub mod web;

use deno_core::extension;

extension!(
    mango_js_runtime,
    ops = [
        // Console
        console::op_mango_console,
        // Timers & Web APIs
        web::op_mango_schedule_timer,
        web::op_mango_cancel_timer,
        web::op_mango_alert,
        web::op_mango_navigate,
        web::op_mango_random_bytes,
        web::op_mango_ls_get,
        web::op_mango_ls_set,
        web::op_mango_ls_remove,
        web::op_mango_ls_clear,
        web::op_mango_ls_keys,
        web::op_mango_ss_get,
        web::op_mango_ss_set,
        web::op_mango_ss_remove,
        web::op_mango_ss_clear,
        web::op_mango_ss_keys,
        web::op_mango_idb_load,
        web::op_mango_idb_save,
        web::op_mango_idb_delete,
        web::op_mango_idb_list,
        web::op_mango_worker_create,
        web::op_mango_worker_post_message,
        web::op_mango_worker_terminate,
        web::op_mango_blob_register,
        web::op_mango_ws_connect,
        web::op_mango_ws_send,
        web::op_mango_ws_close,
        web::op_mango_fetch,
        web::op_mango_start_fetch,
        // DOM Creation & Mutation
        dom::op_mango_create_element,
        dom::op_mango_create_text_node,
        dom::op_mango_create_comment,
        dom::op_mango_create_fragment,
        dom::op_mango_attach_shadow,
        dom::op_mango_clone_node,
        dom::op_mango_append_child,
        dom::op_mango_remove_child,
        dom::op_mango_insert_before,
        dom::op_mango_replace_child,
        // DOM Query
        dom::op_mango_get_element_by_id,
        dom::op_mango_query_selector,
        dom::op_mango_query_selector_all,
        dom::op_mango_get_document_elements,
        // DOM Text & HTML
        dom::op_mango_get_text_content,
        dom::op_mango_set_text_content,
        dom::op_mango_get_inner_html,
        dom::op_mango_set_inner_html,
        dom::op_mango_get_outer_html,
        dom::op_mango_set_outer_html,
        dom::op_mango_get_template_content,
        // DOM Attributes
        dom::op_mango_get_attribute,
        dom::op_mango_set_attribute,
        dom::op_mango_remove_attribute,
        dom::op_mango_has_attribute,
        dom::op_mango_get_attribute_names,
        // DOM Metadata & Traversal
        dom::op_mango_get_tag_name,
        dom::op_mango_get_node_type,
        dom::op_mango_get_node_name,
        dom::op_mango_get_node_value,
        dom::op_mango_set_node_value,
        dom::op_mango_get_parent,
        dom::op_mango_get_children,
        dom::op_mango_get_child_node_ids,
        dom::op_mango_get_first_element_child,
        dom::op_mango_get_last_element_child,
        dom::op_mango_get_next_element_sibling,
        dom::op_mango_get_previous_element_sibling,
        dom::op_mango_get_first_child,
        dom::op_mango_get_last_child,
        dom::op_mango_get_next_sibling,
        dom::op_mango_get_previous_sibling,
        dom::op_mango_get_compat_mode,
        dom::op_mango_get_character_set,
        dom::op_mango_get_content_type,
        // Dataset
        dom::op_mango_get_dataset_value,
        dom::op_mango_set_dataset_value,
        dom::op_mango_remove_dataset_value,
        dom::op_mango_get_dataset_keys,
        // Element Matches & Layout
        dom::op_mango_element_contains,
        dom::op_mango_element_matches,
        dom::op_mango_element_closest,
        dom::op_mango_get_element_rect,
        dom::op_mango_element_from_point,
        dom::op_mango_elements_from_point,
        dom::op_mango_set_focus_state,
        // Cookies
        dom::op_mango_cookie_get,
        dom::op_mango_cookie_set,
        // Canvas 2D
        dom::op_mango_canvas_resize,
        dom::op_mango_canvas_set_fill_style,
        dom::op_mango_canvas_set_stroke_style,
        dom::op_mango_canvas_set_line_width,
        dom::op_mango_canvas_set_line_cap,
        dom::op_mango_canvas_set_line_join,
        dom::op_mango_canvas_set_miter_limit,
        dom::op_mango_canvas_set_global_alpha,
        dom::op_mango_canvas_set_font,
        dom::op_mango_canvas_set_text_align,
        dom::op_mango_canvas_set_text_baseline,
        dom::op_mango_canvas_fill_rect,
        dom::op_mango_canvas_stroke_rect,
        dom::op_mango_canvas_clear_rect,
        dom::op_mango_canvas_begin_path,
        dom::op_mango_canvas_close_path,
        dom::op_mango_canvas_move_to,
        dom::op_mango_canvas_line_to,
        dom::op_mango_canvas_arc,
        dom::op_mango_canvas_fill,
        dom::op_mango_canvas_stroke,
        dom::op_mango_canvas_fill_text,
        dom::op_mango_canvas_stroke_text,
        dom::op_mango_canvas_measure_text,
        dom::op_mango_canvas_save,
        dom::op_mango_canvas_restore,
        dom::op_mango_canvas_translate,
        dom::op_mango_canvas_rotate,
        dom::op_mango_canvas_scale,
        dom::op_mango_canvas_get_image_data,
        dom::op_mango_canvas_put_image_data,
        dom::op_mango_canvas_to_data_url,
    ],
);

pub fn init_extension() -> deno_core::Extension {
    mango_js_runtime::init()
}

pub const PRELUDE_JS: &str = r##"
(function() {
    const ops = Deno.core.ops;
    globalThis._origCE = (tag) => ops.op_mango_create_element(String(tag));
    globalThis._origCTN = (text) => ops.op_mango_create_text_node(String(text));
    globalThis._origCC = (comment) => ops.op_mango_create_comment(String(comment));
    globalThis._createFragment = () => ops.op_mango_create_fragment();
    globalThis._attachShadow = (id, mode) => { const r = ops.op_mango_attach_shadow(Number(id), String(mode)); return r > 0 ? r : null; };
    globalThis._insertBefore = (p, n, r) => ops.op_mango_insert_before(Number(p), Number(n), r != null ? Number(r) : 0);
    globalThis._replaceChild = (p, n, o) => ops.op_mango_replace_child(Number(p), Number(n), Number(o));
    globalThis._cloneNode = (id, deep) => { const r = ops.op_mango_clone_node(Number(id), !!deep); return r > 0 ? r : null; };
    globalThis._getTextContent = (id) => ops.op_mango_get_text_content(Number(id));
    globalThis._setTextContent = (id, text) => ops.op_mango_set_text_content(Number(id), String(text));
    globalThis._appendChild = (p, c) => ops.op_mango_append_child(Number(p), Number(c));
    globalThis._removeChild = (p, c) => ops.op_mango_remove_child(Number(p), Number(c));
    globalThis._setAttribute = (id, name, val) => ops.op_mango_set_attribute(Number(id), String(name), String(val));
    globalThis._removeAttribute = (id, name) => ops.op_mango_remove_attribute(Number(id), String(name));
    globalThis._getAttribute = (id, name) => ops.op_mango_get_attribute(Number(id), String(name));
    globalThis._hasAttribute = (id, name) => ops.op_mango_has_attribute(Number(id), String(name));
    globalThis._getTagName = (id) => ops.op_mango_get_tag_name(Number(id));
    globalThis._getParent = (id) => { const r = ops.op_mango_get_parent(Number(id)); return r > 0 ? r : null; };
    globalThis._getChildren = (id) => ops.op_mango_get_children(Number(id));
    globalThis._getChildNodeIds = (id) => ops.op_mango_get_child_node_ids(Number(id));
    globalThis._getFirstElementChild = (id) => { const r = ops.op_mango_get_first_element_child(Number(id)); return r > 0 ? r : null; };
    globalThis._getLastElementChild = (id) => { const r = ops.op_mango_get_last_element_child(Number(id)); return r > 0 ? r : null; };
    globalThis._getNextElementSibling = (id) => { const r = ops.op_mango_get_next_element_sibling(Number(id)); return r > 0 ? r : null; };
    globalThis._getPreviousElementSibling = (id) => { const r = ops.op_mango_get_previous_element_sibling(Number(id)); return r > 0 ? r : null; };
    globalThis._getFirstChild = (id) => { const r = ops.op_mango_get_first_child(Number(id)); return r > 0 ? r : null; };
    globalThis._getLastChild = (id) => { const r = ops.op_mango_get_last_child(Number(id)); return r > 0 ? r : null; };
    globalThis._getNextSibling = (id) => { const r = ops.op_mango_get_next_sibling(Number(id)); return r > 0 ? r : null; };
    globalThis._getPreviousSibling = (id) => { const r = ops.op_mango_get_previous_sibling(Number(id)); return r > 0 ? r : null; };
    globalThis._getCompatMode = () => ops.op_mango_get_compat_mode();
    globalThis._getCharacterSet = () => ops.op_mango_get_character_set();
    globalThis._getContentType = () => ops.op_mango_get_content_type();
    globalThis._getInnerHTML = (id) => ops.op_mango_get_inner_html(Number(id));
    globalThis._setInnerHTML = (id, html) => ops.op_mango_set_inner_html(Number(id), String(html));
    globalThis._getOuterHTML = (id) => ops.op_mango_get_outer_html(Number(id));
    globalThis._setOuterHTML = (id, html) => ops.op_mango_set_outer_html(Number(id), String(html));
    globalThis._getTemplateContent = (id) => { const r = ops.op_mango_get_template_content(Number(id)); return r > 0 ? r : null; };
    globalThis._getNodeType = (id) => ops.op_mango_get_node_type(Number(id));
    globalThis._getNodeName = (id) => ops.op_mango_get_node_name(Number(id));
    globalThis._getNodeValue = (id) => ops.op_mango_get_node_value(Number(id));
    globalThis._setNodeValue = (id, val) => ops.op_mango_set_node_value(Number(id), val != null ? String(val) : null);
    globalThis._getAttributeNames = (id) => ops.op_mango_get_attribute_names(Number(id));
    globalThis._getDatasetValue = (id, k) => ops.op_mango_get_dataset_value(Number(id), String(k));
    globalThis._setDatasetValue = (id, k, v) => ops.op_mango_set_dataset_value(Number(id), String(k), String(v));
    globalThis._removeDatasetValue = (id, k) => ops.op_mango_remove_dataset_value(Number(id), String(k));
    globalThis._getDatasetKeys = (id) => ops.op_mango_get_dataset_keys(Number(id));
    globalThis._getDocumentElements = (tag) => ops.op_mango_get_document_elements(String(tag));
    globalThis._setFocusState = (id, f) => ops.op_mango_set_focus_state(Number(id), !!f);
    globalThis._getElementRect = (id) => ops.op_mango_get_element_rect(Number(id));
    globalThis._elementFromPoint = (x, y) => { const r = ops.op_mango_element_from_point(Number(x), Number(y)); return r > 0 ? r : null; };
    globalThis._elementsFromPoint = (x, y) => ops.op_mango_elements_from_point(Number(x), Number(y));
    globalThis._elementContains = (p, c) => ops.op_mango_element_contains(Number(p), Number(c));
    globalThis._elementMatches = (id, sel) => ops.op_mango_element_matches(Number(id), String(sel));
    globalThis._elementClosest = (id, sel) => { const r = ops.op_mango_element_closest(Number(id), String(sel)); return r > 0 ? r : null; };
    globalThis._cookieGet = () => ops.op_mango_cookie_get();
    globalThis._cookieSet = (s) => ops.op_mango_cookie_set(String(s));
    globalThis._getDocumentCookie = () => ops.op_mango_cookie_get();
    globalThis._setDocumentCookie = (s) => ops.op_mango_cookie_set(String(s));

    globalThis._querySelector = (scope, sel) => {
        if (sel === undefined) { sel = scope; scope = 0; }
        const res = ops.op_mango_query_selector(Number(scope || 0), String(sel));
        return res > 0 ? res : null;
    };
    globalThis._querySelectorAll = (scope, sel) => {
        if (sel === undefined) { sel = scope; scope = 0; }
        return ops.op_mango_query_selector_all(Number(scope || 0), String(sel)) || [];
    };

    globalThis.document = {
        _nodeId: 0,
        nodeType: 9,
        nodeName: "#document",
        getElementById: function(id) {
            const elId = ops.op_mango_get_element_by_id(String(id));
            return elId > 0 ? elId : null;
        },
        querySelector: function(sel) {
            const elId = ops.op_mango_query_selector(0, String(sel));
            return elId > 0 ? elId : null;
        },
        querySelectorAll: function(sel) {
            return ops.op_mango_query_selector_all(0, String(sel)) || [];
        },
        createElement: function(tag) {
            return ops.op_mango_create_element(String(tag));
        },
        createTextNode: function(txt) {
            return ops.op_mango_create_text_node(String(txt));
        },
    };
    if (typeof globalThis.window !== 'undefined') {
        globalThis.window.document = globalThis.document;
    }

    // Canvas ops
    globalThis._canvasResize = (id, w, h) => ops.op_mango_canvas_resize(Number(id), Number(w), Number(h));
    globalThis._canvasSetFillStyle = (id, c) => ops.op_mango_canvas_set_fill_style(Number(id), String(c));
    globalThis._canvasSetStrokeStyle = (id, c) => ops.op_mango_canvas_set_stroke_style(Number(id), String(c));
    globalThis._canvasSetLineWidth = (id, w) => ops.op_mango_canvas_set_line_width(Number(id), Number(w));
    globalThis._canvasSetLineCap = (id, c) => ops.op_mango_canvas_set_line_cap(Number(id), String(c));
    globalThis._canvasSetLineJoin = (id, j) => ops.op_mango_canvas_set_line_join(Number(id), String(j));
    globalThis._canvasSetMiterLimit = (id, l) => ops.op_mango_canvas_set_miter_limit(Number(id), Number(l));
    globalThis._canvasSetGlobalAlpha = (id, a) => ops.op_mango_canvas_set_global_alpha(Number(id), Number(a));
    globalThis._canvasSetFont = (id, f) => ops.op_mango_canvas_set_font(Number(id), String(f));
    globalThis._canvasSetTextAlign = (id, a) => ops.op_mango_canvas_set_text_align(Number(id), String(a));
    globalThis._canvasSetTextBaseline = (id, b) => ops.op_mango_canvas_set_text_baseline(Number(id), String(b));
    globalThis._canvasFillRect = (id, x, y, w, h) => ops.op_mango_canvas_fill_rect(Number(id), Number(x), Number(y), Number(w), Number(h));
    globalThis._canvasStrokeRect = (id, x, y, w, h) => ops.op_mango_canvas_stroke_rect(Number(id), Number(x), Number(y), Number(w), Number(h));
    globalThis._canvasClearRect = (id, x, y, w, h) => ops.op_mango_canvas_clear_rect(Number(id), Number(x), Number(y), Number(w), Number(h));
    globalThis._canvasBeginPath = (id) => ops.op_mango_canvas_begin_path(Number(id));
    globalThis._canvasClosePath = (id) => ops.op_mango_canvas_close_path(Number(id));
    globalThis._canvasMoveTo = (id, x, y) => ops.op_mango_canvas_move_to(Number(id), Number(x), Number(y));
    globalThis._canvasLineTo = (id, x, y) => ops.op_mango_canvas_line_to(Number(id), Number(x), Number(y));
    globalThis._canvasArc = (id, x, y, r, sa, ea, ccw) => ops.op_mango_canvas_arc(Number(id), Number(x), Number(y), Number(r), Number(sa), Number(ea), !!ccw);
    globalThis._canvasFill = (id) => ops.op_mango_canvas_fill(Number(id));
    globalThis._canvasStroke = (id) => ops.op_mango_canvas_stroke(Number(id));
    globalThis._canvasFillText = (id, t, x, y) => ops.op_mango_canvas_fill_text(Number(id), String(t), Number(x), Number(y));
    globalThis._canvasStrokeText = (id, t, x, y) => ops.op_mango_canvas_stroke_text(Number(id), String(t), Number(x), Number(y));
    globalThis._canvasMeasureText = (id, t) => ops.op_mango_canvas_measure_text(Number(id), String(t));
    globalThis._canvasSave = (id) => ops.op_mango_canvas_save(Number(id));
    globalThis._canvasRestore = (id) => ops.op_mango_canvas_restore(Number(id));
    globalThis._canvasTranslate = (id, x, y) => ops.op_mango_canvas_translate(Number(id), Number(x), Number(y));
    globalThis._canvasRotate = (id, a) => ops.op_mango_canvas_rotate(Number(id), Number(a));
    globalThis._canvasScale = (id, sx, sy) => ops.op_mango_canvas_scale(Number(id), Number(sx), Number(sy));
    globalThis._canvasGetImageData = (id, x, y, w, h) => ops.op_mango_canvas_get_image_data(Number(id), Number(x), Number(y), Number(w), Number(h));
    globalThis._canvasPutImageData = (id, d, dx, dy, dX, dY, dW, dH, srcW) => ops.op_mango_canvas_put_image_data(Number(id), Array.from(d), Number(dx), Number(dy), Number(dX), Number(dY), Number(dW), Number(dH), Number(srcW));
    globalThis._canvasToDataUrl = (id) => ops.op_mango_canvas_to_data_url(Number(id));

    // Web APIs
    globalThis._mangoScheduleTimer = (id, delay, repeats) => ops.op_mango_schedule_timer(Number(id), Number(delay), !!repeats, false);
    globalThis._mangoCancelTimer = (id) => ops.op_mango_cancel_timer(Number(id));
    globalThis._mangoRandomBytes = (len) => ops.op_mango_random_bytes(Number(len));
    globalThis._mangoStartFetch = (id, url, method, headers, body, origin, mode, creds) => ops.op_mango_start_fetch(Number(id), String(url), String(method), String(headers), String(body), String(origin), String(mode), !!creds);
    globalThis._mangoFetch = (url, method, headers, body, origin, mode, creds) => ops.op_mango_fetch(String(url), String(method), String(headers), body != null ? String(body) : null, String(origin), String(mode), !!creds);
    globalThis._mangoLsGet = (k) => ops.op_mango_ls_get(String(k));
    globalThis._mangoLsSet = (k, v) => ops.op_mango_ls_set(String(k), String(v));
    globalThis._mangoLsRemove = (k) => ops.op_mango_ls_remove(String(k));
    globalThis._mangoLsClear = () => ops.op_mango_ls_clear();
    globalThis._mangoLsKeys = () => ops.op_mango_ls_keys();
    globalThis._mangoSsGet = (k) => ops.op_mango_ss_get(String(k));
    globalThis._mangoSsSet = (k, v) => ops.op_mango_ss_set(String(k), String(v));
    globalThis._mangoSsRemove = (k) => ops.op_mango_ss_remove(String(k));
    globalThis._mangoSsClear = () => ops.op_mango_ss_clear();
    globalThis._mangoSsKeys = () => ops.op_mango_ss_keys();
    globalThis._mangoIdbLoad = (k) => ops.op_mango_idb_load(String(k));
    globalThis._mangoIdbSave = (k, v) => ops.op_mango_idb_save(String(k), String(v));
    globalThis._mangoIdbDelete = (k) => ops.op_mango_idb_delete(String(k));
    globalThis._mangoIdbList = () => ops.op_mango_idb_list();
    globalThis._mangoWorkerCreate = (id, url) => ops.op_mango_worker_create(Number(id), String(url));
    globalThis._mangoWorkerPostMessage = (id, msg) => ops.op_mango_worker_post_message(Number(id), String(msg));
    globalThis._mangoWorkerTerminate = (id) => ops.op_mango_worker_terminate(Number(id));
    globalThis._mangoBlobRegister = (id, data) => ops.op_mango_blob_register(String(id), String(data));
    globalThis._mangoWsConnect = (id, url, protocols) => ops.op_mango_ws_connect(Number(id), String(url), String(protocols || ''));
    globalThis._mangoWsSend = (id, data) => ops.op_mango_ws_send(Number(id), String(data));
    globalThis._mangoWsClose = (id, code, reason) => ops.op_mango_ws_close(Number(id), Number(code), String(reason));
    globalThis._mangoNavigate = (url) => ops.op_mango_navigate(String(url));
    globalThis.alert = (msg) => ops.op_mango_alert(String(msg));

    // Console
    const cLog = (t) => ops.op_mango_console(0, t);
    const cInfo = (t) => ops.op_mango_console(1, t);
    const cWarn = (t) => ops.op_mango_console(2, t);
    const cError = (t) => ops.op_mango_console(3, t);
    globalThis.console = {
        log: (...args) => cLog(args.map(String).join(' ')),
        info: (...args) => cInfo(args.map(String).join(' ')),
        warn: (...args) => cWarn(args.map(String).join(' ')),
        error: (...args) => cError(args.map(String).join(' ')),
    };
})();
"##;
