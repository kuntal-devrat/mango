use super::*;
use mango_html::dom::Document;
use mango_html::parse_html;

#[test]
fn test_dom_manipulation() {
    let html = r##"
        <html>
        <head></head>
        <body>
          <div id="container">
            <p id="target">Middle</p>
          </div>
          <div id="replace-container">
            <span id="old1">One</span>
            <span id="old2">Two</span>
          </div>
          <ul id="list">
            <li id="item2">Two</li>
          </ul>
          <div id="removable">To be removed</div>
          <select id="my-select">
            <option value="1">First</option>
            <option value="2">Second</option>
            <option value="3">Third</option>
          </select>
          <div id="range-test">
            <span id="r1">Hello</span> <span id="r2">World</span>
          </div>
          <input type="text" id="my-input" />
          <button id="my-btn">Click Me</button>
          <input type="checkbox" id="my-check" />
          <button id="disabled-btn" disabled>Disabled</button>
        </body>
        </html>
    "##;
    let doc = parse_html(html);
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let script = r##"
        // 1. insertAdjacentHTML
        var target = document.getElementById("target");
        target.insertAdjacentHTML("beforebegin", "<span id='before-b'>BB</span>");
        target.insertAdjacentHTML("afterbegin", "<span id='after-b'>AB</span>");
        target.insertAdjacentHTML("beforeend", "<span id='before-e'>BE</span>");
        target.insertAdjacentHTML("afterend", "<span id='after-e'>AE</span>");

        var container = document.getElementById("container");
        var bb = document.getElementById("before-b");
        var ab = document.getElementById("after-b");
        var be = document.getElementById("before-e");
        var ae = document.getElementById("after-e");

        if (!bb || !ab || !be || !ae) throw new Error("insertAdjacentHTML: nodes not found");
        if (target.previousElementSibling !== bb) throw new Error("beforebegin failed");
        if (target.firstElementChild !== ab) throw new Error("afterbegin failed");
        if (target.lastElementChild !== be) throw new Error("beforeend failed");
        if (target.nextElementSibling !== ae) throw new Error("afterend failed");

        var syntaxErrorCaught = false;
        try {
            target.insertAdjacentHTML("invalid_pos", "<span>fail</span>");
        } catch (e) {
            if (e.name === "SyntaxError") syntaxErrorCaught = true;
        }
        if (!syntaxErrorCaught) throw new Error("insertAdjacentHTML did not throw SyntaxError on invalid position");

        // 2. replaceChildren
        var repCont = document.getElementById("replace-container");
        if (repCont.children.length !== 2) throw new Error("replaceChildren setup invalid");

        var newChild1 = document.createElement("b");
        newChild1.textContent = "Bold";
        repCont.replaceChildren(newChild1, "TextString");
        if (repCont.childNodes.length !== 2) throw new Error("replaceChildren length mismatch");
        if (repCont.childNodes[0].tagName.toLowerCase() !== "b") throw new Error("replaceChildren first child mismatch");
        if (repCont.childNodes[1].textContent !== "TextString") throw new Error("replaceChildren string node mismatch");

        repCont.replaceChildren();
        if (repCont.childNodes.length !== 0) throw new Error("replaceChildren() should clear children");

        // 3. append, prepend, before, after, replaceWith
        var list = document.getElementById("list");
        var item2 = document.getElementById("item2");

        var item1 = document.createElement("li");
        item1.id = "item1";
        item1.textContent = "One";
        list.prepend(item1);
        if (list.firstElementChild.id !== "item1") throw new Error("prepend failed");

        var item4 = document.createElement("li");
        item4.id = "item4";
        item4.textContent = "Four";
        list.append(item4, "end-text");
        if (list.lastChild.textContent !== "end-text") throw new Error("append string failed");
        if (item4.previousElementSibling.id !== "item2") throw new Error("append element failed");

        var itemBefore = document.createElement("li");
        itemBefore.id = "item-before";
        item2.before(itemBefore);
        if (item2.previousElementSibling.id !== "item-before") throw new Error("before() failed");

        var itemAfter = document.createElement("li");
        itemAfter.id = "item-after";
        item2.after(itemAfter);
        if (item2.nextElementSibling.id !== "item-after") throw new Error("after() failed");

        var itemReplaced = document.createElement("li");
        itemReplaced.id = "item-replaced";
        itemBefore.replaceWith(itemReplaced);
        if (document.getElementById("item-before") !== null) throw new Error("replaceWith old node still exists");
        if (item2.previousElementSibling.id !== "item-replaced") throw new Error("replaceWith new node not in place");

        // 4. remove()
        var removable = document.getElementById("removable");
        removable.remove();
        if (document.getElementById("removable") !== null) throw new Error("remove() element failed");

        var select = document.getElementById("my-select");
        if (select.children.length !== 3) throw new Error("select setup invalid");
        select.remove(1);
        if (select.children.length !== 2) throw new Error("select.remove(1) length mismatch");
        if (select.children[1].value !== "3") throw new Error("select.remove(1) wrong option removed");

        // 5. Range API & document.createRange()
        var range = document.createRange();
        if (!range) throw new Error("document.createRange returned falsy");
        if (Range.START_TO_START !== 0 || Range.START_TO_END !== 1 || Range.END_TO_END !== 2 || Range.END_TO_START !== 3) {
            throw new Error("Range constants invalid");
        }
        if (range.START_TO_START !== 0) throw new Error("Range.prototype constants invalid");

        var rTest = document.getElementById("range-test");
        var r1 = document.getElementById("r1");
        var r2 = document.getElementById("r2");

        range.selectNodeContents(rTest);
        if (range.startContainer !== rTest || range.endContainer !== rTest) {
            throw new Error("selectNodeContents container mismatch");
        }
        if (range.collapsed) throw new Error("selectNodeContents range should not be collapsed");

        range.collapse(true);
        if (!range.collapsed) throw new Error("range.collapse(true) failed");

        range.setStart(rTest, 0);
        range.setEnd(rTest, 1);
        if (range.commonAncestorContainer !== rTest) throw new Error("commonAncestorContainer mismatch");

        var frag = range.cloneContents();
        if (!frag || frag.nodeType !== 11) throw new Error("cloneContents should return DocumentFragment");

        var ctxFrag = range.createContextualFragment("<span id='ctx-elem'>Context</span>");
        if (!ctxFrag || ctxFrag.nodeType !== 11) throw new Error("createContextualFragment failed");
        if (!ctxFrag.querySelector("#ctx-elem")) throw new Error("createContextualFragment child missing");

        var clonedRange = range.cloneRange();
        if (clonedRange.startContainer !== range.startContainer || clonedRange.endOffset !== range.endOffset) {
            throw new Error("cloneRange failed");
        }

        var comp = range.compareBoundaryPoints(Range.START_TO_START, clonedRange);
        if (comp !== 0) throw new Error("compareBoundaryPoints should be 0 for identical ranges");

        range.setStartBefore(r2);
        range.setEndAfter(r2);
        if (!range.intersectsNode(r2)) throw new Error("range should intersect r2");
        if (!range.isPointInRange(range.startContainer, range.startOffset)) throw new Error("range isPointInRange failed");
        if (range.comparePoint(range.startContainer, range.startOffset) !== 0) throw new Error("range comparePoint failed");

        var rRect = range.getBoundingClientRect();
        if (typeof rRect.width !== "number" || typeof rRect.height !== "number") {
            throw new Error("range.getBoundingClientRect() invalid");
        }

        var insertedSpan = document.createElement("span");
        insertedSpan.id = "inserted-into-range";
        range.collapse(true);
        range.insertNode(insertedSpan);
        if (document.getElementById("inserted-into-range") === null) {
            throw new Error("insertNode failed to insert element into DOM");
        }

        var extRange = document.createRange();
        extRange.selectNode(insertedSpan);
        var extractedFrag = extRange.extractContents();
        if (document.getElementById("inserted-into-range") !== null) {
            throw new Error("extractContents failed to remove element from DOM");
        }
        if (!extractedFrag || extractedFrag.childNodes.length === 0) {
            throw new Error("extractContents failed to return fragment with nodes");
        }

        var surroundWrapper = document.createElement("div");
        surroundWrapper.id = "surround-wrapper";
        var surRange = document.createRange();
        surRange.selectNode(r1);
        surRange.surroundContents(surroundWrapper);
        if (r1.parentElement.id !== "surround-wrapper") {
            throw new Error("surroundContents failed to wrap target node");
        }

        var delRange = document.createRange();
        delRange.selectNode(surroundWrapper);
        delRange.deleteContents();
        if (document.getElementById("surround-wrapper") !== null) {
            throw new Error("deleteContents failed to delete nodes from DOM");
        }

        // 6. Selection API & window.getSelection()
        var sel = window.getSelection();
        if (!sel) throw new Error("window.getSelection returned falsy");
        if (document.getSelection() !== sel) throw new Error("document.getSelection !== window.getSelection");

        sel.removeAllRanges();
        if (sel.rangeCount !== 0) throw new Error("removeAllRanges failed");

        sel.addRange(range);
        if (sel.rangeCount !== 1) throw new Error("addRange failed, rangeCount !== 1");
        if (sel.getRangeAt(0) !== range) throw new Error("getRangeAt(0) mismatch");

        sel.selectAllChildren(rTest);
        if (sel.anchorNode !== rTest) throw new Error("selectAllChildren anchorNode mismatch");
        if (!sel.containsNode(r2, true)) throw new Error("containsNode should be true for child r2");

        sel.setBaseAndExtent(rTest, 0, rTest, 1);
        if (sel.anchorOffset !== 0 || sel.focusOffset !== 1) {
            throw new Error("setBaseAndExtent offsets mismatch");
        }

        sel.collapse(r2, 0);
        if (!sel.isCollapsed) throw new Error("Selection should be collapsed after collapse()");

        sel.extend(rTest, 1);
        if (sel.focusNode !== rTest || sel.focusOffset !== 1) {
            throw new Error("extend failed");
        }

        sel.empty();
        if (sel.rangeCount !== 0) throw new Error("empty() failed to clear ranges");

        var testFrag = document.createDocumentFragment();
        testFrag.append("FragItem1", "FragItem2");
        if (testFrag.childNodes.length !== 2) throw new Error("Fragment.append failed");
        testFrag.replaceChildren("NewFragItem");
        if (testFrag.childNodes.length !== 1 || testFrag.childNodes[0].textContent !== "NewFragItem") {
            throw new Error("Fragment.replaceChildren failed");
        }

        // 7. element.focus() and element.blur()
        var input = document.getElementById("my-input");
        var focusEvents = [];
        input.addEventListener("focus", function(e) { focusEvents.push("focus"); });
        input.addEventListener("focusin", function(e) { focusEvents.push("focusin"); });
        input.addEventListener("blur", function(e) { focusEvents.push("blur"); });
        input.addEventListener("focusout", function(e) { focusEvents.push("focusout"); });

        if (document.activeElement !== document.body) {
            throw new Error("Initial activeElement should be document.body");
        }

        input.focus();
        if (document.activeElement !== input) {
            throw new Error("activeElement should be input after focus()");
        }
        if (focusEvents.indexOf("focus") === -1 || focusEvents.indexOf("focusin") === -1) {
            throw new Error("focus/focusin events not dispatched on focus()");
        }

        input.blur();
        if (document.activeElement !== document.body) {
            throw new Error("activeElement should revert to body after blur()");
        }
        if (focusEvents.indexOf("blur") === -1 || focusEvents.indexOf("focusout") === -1) {
            throw new Error("blur/focusout events not dispatched on blur()");
        }

        // 8. element.click()
        var btn = document.getElementById("my-btn");
        var btnClicked = false;
        var clickDetail = 0;
        var clickBubbles = false;
        var clickCancelable = false;
        btn.addEventListener("click", function(e) {
            btnClicked = true;
            clickDetail = e.detail;
            clickBubbles = e.bubbles;
            clickCancelable = e.cancelable;
        });
        btn.click();
        if (!btnClicked) throw new Error("btn.click() did not trigger click listener");
        if (!clickBubbles) throw new Error("synthetic click event should bubble");
        if (!clickCancelable) throw new Error("synthetic click event should be cancelable");

        var check = document.getElementById("my-check");
        var checkChanged = false;
        check.addEventListener("change", function(e) {
            checkChanged = true;
        });
        if (check.checked) throw new Error("checkbox should start unchecked");
        check.click();
        if (!check.checked) throw new Error("checkbox should be checked after click()");
        if (!checkChanged) throw new Error("checkbox change event should fire on click()");

        var disabledBtn = document.getElementById("disabled-btn");
        var disabledClicked = false;
        disabledBtn.addEventListener("click", function() {
            disabledClicked = true;
        });
        disabledBtn.click();
        if (disabledClicked) throw new Error("Disabled button should not fire click");
    "##;

    rt.execute_script(script)
        .expect("Section 7.5 DOM manipulation test failed");
}

#[test]
fn test_compare_document_position() {
    let mut doc = Document::new();
    let body = doc.create_element("body", Vec::new());
    let parent = doc.create_element("div", vec![("id".to_string(), "parent".to_string())]);
    let child1 = doc.create_element("span", vec![("id".to_string(), "child1".to_string())]);
    let child2 = doc.create_element("p", vec![("id".to_string(), "child2".to_string())]);
    doc.append_child(body, parent);
    doc.append_child(parent, child1);
    doc.append_child(parent, child2);
    let root = doc.root();
    doc.append_child(root, body);

    let mut rt = JsRuntime::new(doc, 1024.0, 768.0);
    let script = r#"
        (function() {
            var parent = document.getElementById("parent");
            var child1 = document.getElementById("child1");
            var child2 = document.getElementById("child2");

            if (!parent || !child1 || !child2) throw new Error("Elements not found");

            // Self comparison
            if (parent.compareDocumentPosition(parent) !== 0) {
                throw new Error("Self comparison must return 0");
            }

            // Parent to child: child is contained by parent and follows parent (16 | 4 = 20)
            var p_to_c1 = parent.compareDocumentPosition(child1);
            var expected_contained_by = Node.DOCUMENT_POSITION_CONTAINED_BY | Node.DOCUMENT_POSITION_FOLLOWING;
            if (p_to_c1 !== expected_contained_by) {
                throw new Error("parent -> child1 expected " + expected_contained_by + " (20), got " + p_to_c1);
            }

            // Child to parent: parent contains child and precedes child (8 | 2 = 10)
            var c1_to_p = child1.compareDocumentPosition(parent);
            var expected_contains = Node.DOCUMENT_POSITION_CONTAINS | Node.DOCUMENT_POSITION_PRECEDING;
            if (c1_to_p !== expected_contains) {
                throw new Error("child1 -> parent expected " + expected_contains + " (10), got " + c1_to_p);
            }

            // Sibling order: child1 comes before child2, so child2 follows child1 (4)
            var c1_to_c2 = child1.compareDocumentPosition(child2);
            if (c1_to_c2 !== Node.DOCUMENT_POSITION_FOLLOWING) {
                throw new Error("child1 -> child2 expected FOLLOWING (4), got " + c1_to_c2);
            }

            // Sibling order reverse: child1 precedes child2 (2)
            var c2_to_c1 = child2.compareDocumentPosition(child1);
            if (c2_to_c1 !== Node.DOCUMENT_POSITION_PRECEDING) {
                throw new Error("child2 -> child1 expected PRECEDING (2), got " + c2_to_c1);
            }

            // Disconnected node
            var disconnected = document.createElement("div");
            var disc_pos = parent.compareDocumentPosition(disconnected);
            if ((disc_pos & Node.DOCUMENT_POSITION_DISCONNECTED) === 0) {
                throw new Error("disconnected node must have DISCONNECTED bit set");
            }
            if ((disc_pos & Node.DOCUMENT_POSITION_IMPLEMENTATION_SPECIFIC) === 0) {
                throw new Error("disconnected node must have IMPLEMENTATION_SPECIFIC bit set");
            }

            return "ok";
        })()
    "#;
    let res = rt.eval(script).expect("eval compareDocumentPosition test");
    assert_eq!(res, "ok");
}
