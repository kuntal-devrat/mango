// WPT Suite 1: DOM & Tree Operations (dom/nodes/*, dom/events/*)

test(function() {
    assert_equals(document.nodeType, 9, "Document nodeType must be 9");
    assert_equals(document.nodeName, "#document", "Document nodeName must be #document");
    assert_true(document.body !== null, "document.body should exist");
    assert_true(document.documentElement !== null, "document.documentElement should exist");
}, "WPT DOM: Document interface basic properties");

test(function() {
    const div = document.createElement("div");
    assert_equals(div.nodeType, 1, "Element nodeType must be 1");
    assert_equals(div.nodeName, "DIV", "Element nodeName must be uppercase DIV");
    assert_equals(div.tagName, "DIV", "Element tagName must be uppercase DIV");
    assert_false(div.hasChildNodes(), "Newly created div should have no children");
}, "WPT DOM: document.createElement and basic Element properties");

test(function() {
    const parent = document.createElement("div");
    const child1 = document.createElement("span");
    const child2 = document.createElement("p");

    parent.appendChild(child1);
    assert_equals(parent.childNodes.length, 1, "childNodes length after appendChild");
    assert_equals(parent.firstChild, child1, "firstChild should match child1");
    assert_equals(parent.lastChild, child1, "lastChild should match child1");
    assert_equals(child1.parentNode, parent, "child1 parentNode should be parent");

    parent.appendChild(child2);
    assert_equals(parent.childNodes.length, 2, "childNodes length after second append");
    assert_equals(parent.firstChild, child1, "firstChild should still be child1");
    assert_equals(parent.lastChild, child2, "lastChild should be child2");
    assert_true(parent.contains(child1), "parent should contain child1");
    assert_true(parent.contains(child2), "parent should contain child2");

    parent.removeChild(child1);
    assert_equals(parent.childNodes.length, 1, "childNodes length after removeChild");
    assert_equals(parent.firstChild, child2, "firstChild after removal should be child2");
    assert_false(parent.contains(child1), "parent should not contain child1 after remove");
}, "WPT DOM: Node tree manipulation (appendChild, removeChild, contains)");

test(function() {
    const parent = document.createElement("div");
    const existing = document.createElement("p");
    parent.appendChild(existing);

    const inserted = document.createElement("span");
    parent.insertBefore(inserted, existing);

    assert_equals(parent.childNodes.length, 2, "childNodes length after insertBefore");
    assert_equals(parent.firstChild, inserted, "inserted should be firstChild");
    assert_equals(parent.lastChild, existing, "existing should be lastChild");

    const replacement = document.createElement("h1");
    parent.replaceChild(replacement, existing);
    assert_equals(parent.lastChild, replacement, "replaceChild should place replacement at target position");
    assert_false(parent.contains(existing), "replaced element no longer in tree");
}, "WPT DOM: Node insertBefore and replaceChild");

test(function() {
    const container = document.createElement("div");
    container.innerHTML = "<div class='outer' id='out'><span class='inner'>Hello</span></div>";

    const clonedShallow = container.cloneNode(false);
    assert_equals(clonedShallow.childNodes.length, 0, "Shallow clone has no children");

    const clonedDeep = container.cloneNode(true);
    assert_equals(clonedDeep.childNodes.length, 1, "Deep clone has children");
    const inner = clonedDeep.querySelector(".inner");
    assert_true(inner !== null, "Deep clone preserves subtree query target");
    assert_equals(inner.textContent, "Hello", "Deep clone preserves text content");
}, "WPT DOM: Node.prototype.cloneNode shallow and deep");

test(function() {
    const el = document.createElement("div");
    assert_false(el.hasAttribute("data-test"), "Initial hasAttribute should be false");
    el.setAttribute("data-test", "sample_val");
    assert_true(el.hasAttribute("data-test"), "hasAttribute should be true after setAttribute");
    assert_equals(el.getAttribute("data-test"), "sample_val", "getAttribute value mismatch");

    el.removeAttribute("data-test");
    assert_false(el.hasAttribute("data-test"), "hasAttribute false after removeAttribute");
    assert_equals(el.getAttribute("data-test"), null, "getAttribute should be null after remove");

    el.toggleAttribute("hidden", true);
    assert_true(el.hasAttribute("hidden"), "toggleAttribute(hidden, true)");
    el.toggleAttribute("hidden", false);
    assert_false(el.hasAttribute("hidden"), "toggleAttribute(hidden, false)");
}, "WPT DOM: Element attribute methods (setAttribute, getAttribute, hasAttribute, toggleAttribute)");

test(function() {
    const el = document.createElement("div");
    el.className = "foo bar";
    assert_true(el.classList.contains("foo"), "classList contains foo");
    assert_true(el.classList.contains("bar"), "classList contains bar");
    assert_false(el.classList.contains("baz"), "classList does not contain baz");

    el.classList.add("baz");
    assert_true(el.classList.contains("baz"), "classList contains baz after add");

    el.classList.remove("bar");
    assert_false(el.classList.contains("bar"), "classList does not contain bar after remove");

    const toggledOn = el.classList.toggle("qux");
    assert_true(toggledOn, "classList toggle on returns true");
    assert_true(el.classList.contains("qux"), "classList has qux after toggle");

    const toggledOff = el.classList.toggle("qux");
    assert_false(toggledOff, "classList toggle off returns false");
    assert_false(el.classList.contains("qux"), "classList no longer has qux");
}, "WPT DOM: Element classList (add, remove, contains, toggle)");

test(function() {
    const container = document.createElement("div");
    container.innerHTML = "<p class='item active' id='first'>One</p><p class='item' id='second'>Two</p>";
    document.body.appendChild(container);

    const first = document.getElementById("first");
    assert_true(first !== null, "getElementById should find element");
    assert_equals(first.textContent.trim(), "One", "textContent of first item");

    const byClass = document.getElementsByClassName("item");
    assert_true(byClass.length >= 2, "getElementsByClassName count");

    const qFirst = container.querySelector(".active");
    assert_equals(qFirst.id, "first", "querySelector matched active item");

    const qAll = container.querySelectorAll(".item");
    assert_equals(qAll.length, 2, "querySelectorAll matched all items");

    // Element matches and closest
    assert_true(first.matches(".item.active"), "Element.matches matching selector");
    assert_false(first.matches(".other"), "Element.matches non-matching selector");
    assert_equals(first.closest("div"), container, "Element.closest finds parent container");

    document.body.removeChild(container);
}, "WPT DOM: Queries and Matching (getElementById, querySelector, matches, closest)");

test(function() {
    const div = document.createElement("div");
    div.innerHTML = "<p>Middle</p>";
    div.firstElementChild.insertAdjacentHTML("beforebegin", "<span>Before</span>");
    div.firstElementChild.insertAdjacentHTML("afterend", "<b>After</b>");

    assert_equals(div.children.length, 3, "insertAdjacentHTML child count");
    assert_equals(div.children[0].tagName, "SPAN", "beforebegin element");
    assert_equals(div.children[1].tagName, "B", "afterend element");
    assert_equals(div.children[2].tagName, "P", "original element");
}, "WPT DOM: Element.prototype.insertAdjacentHTML");

test(function() {
    const target = document.createElement("button");
    let eventHandled = false;
    let eventDetail = null;

    const handler = function(e) {
        eventHandled = true;
        eventDetail = e.detail;
        if (e.cancelable) e.preventDefault();
    };

    target.addEventListener("custom", handler);
    const customEvt = new CustomEvent("custom", { cancelable: true, detail: { score: 99 } });
    const dispatched = target.dispatchEvent(customEvt);

    assert_true(eventHandled, "Event listener should be invoked");
    assert_equals(eventDetail.score, 99, "Event detail score");
    assert_true(customEvt.defaultPrevented, "defaultPrevented should be true after preventDefault()");

    // Remove listener
    eventHandled = false;
    target.removeEventListener("custom", handler);
    target.dispatchEvent(customEvt);
    assert_false(eventHandled, "Event listener should not be invoked after removal");
}, "WPT DOM: EventTarget addEventListener, removeEventListener, and dispatchEvent with preventDefault");
