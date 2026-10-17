// WPT Suite 2: HTML, Custom Elements, Shadow DOM & Popovers (html/*)

test(function() {
    assert_true(typeof customElements !== "undefined", "customElements registry should exist");
    assert_true(typeof customElements.define === "function", "customElements.define should be a function");
    assert_true(typeof customElements.get === "function", "customElements.get should be a function");

    class WptWidget extends HTMLElement {
        constructor() {
            super();
            this.initialized = true;
        }
    }
    customElements.define("wpt-widget", WptWidget);

    assert_equals(customElements.get("wpt-widget"), WptWidget, "customElements.get should return defined constructor");

    const widget = document.createElement("wpt-widget");
    assert_true(widget instanceof WptWidget, "Created element should be instance of WptWidget");
    assert_true(widget.initialized, "Constructor should have run");
}, "WPT HTML: Custom Elements registry and autonomous definition");

test(function() {
    let observedChange = null;
    class WptAttrWidget extends HTMLElement {
        static get observedAttributes() {
            return ["theme"];
        }
        attributeChangedCallback(name, oldValue, newValue) {
            observedChange = { name, oldValue, newValue };
        }
    }
    customElements.define("wpt-attr-widget", WptAttrWidget);

    const el = document.createElement("wpt-attr-widget");
    el.setAttribute("theme", "dark");
    assert_true(observedChange !== null, "attributeChangedCallback should be invoked");
    assert_equals(observedChange.name, "theme", "attributeChangedCallback name");
    assert_equals(observedChange.newValue, "dark", "attributeChangedCallback newValue");
}, "WPT HTML: Custom Elements observedAttributes and attributeChangedCallback");

test(function() {
    const host = document.createElement("div");
    assert_true(typeof host.attachShadow === "function", "attachShadow should be defined on Element");

    const shadow = host.attachShadow({ mode: "open" });
    assert_equals(host.shadowRoot, shadow, "shadowRoot should match attached open shadow");
    assert_equals(shadow.host, host, "shadow.host should reference host element");

    const inner = document.createElement("p");
    inner.textContent = "Shadow text";
    shadow.appendChild(inner);

    assert_equals(shadow.childNodes.length, 1, "shadow childNodes length");
    assert_equals(shadow.firstChild.textContent, "Shadow text", "shadow child text content");
}, "WPT HTML: Open Shadow DOM attachment and encapsulation");

test(function() {
    const hostClosed = document.createElement("div");
    const shadowClosed = hostClosed.attachShadow({ mode: "closed" });
    assert_true(shadowClosed !== null, "attachShadow({ mode: 'closed' }) returns ShadowRoot");
    assert_equals(hostClosed.shadowRoot, null, "closed shadow root must hide host.shadowRoot (returns null)");
}, "WPT HTML: Closed Shadow DOM mode privacy");

test(function() {
    const template = document.createElement("template");
    assert_true(template.content !== null, "template.content should be a DocumentFragment");
    template.innerHTML = "<div class='in-template'><span>Inside</span></div>";

    assert_equals(template.childNodes.length, 0, "template element itself should have no direct child nodes");
    assert_equals(template.content.childNodes.length, 1, "template content should hold parsed nodes");

    const clone = template.content.cloneNode(true);
    assert_equals(clone.firstChild.className, "in-template", "Cloned fragment has template structure");
}, "WPT HTML: HTMLTemplateElement and DocumentFragment cloning");

test(function() {
    const popover = document.createElement("div");
    popover.setAttribute("popover", "auto");
    document.body.appendChild(popover);

    assert_false(popover.matches(":popover-open"), "Popover should initially not be open");

    popover.showPopover();
    assert_true(popover.matches(":popover-open"), "matches(':popover-open') should be true after showPopover()");

    popover.hidePopover();
    assert_false(popover.matches(":popover-open"), "matches(':popover-open') should be false after hidePopover()");

    popover.togglePopover();
    assert_true(popover.matches(":popover-open"), "togglePopover should open popover");

    popover.togglePopover();
    assert_false(popover.matches(":popover-open"), "togglePopover should close popover");

    document.body.removeChild(popover);
}, "WPT HTML: HTML5 Popover API showPopover, hidePopover, and togglePopover");
