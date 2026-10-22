use mango_html::parse_html;

use crate::runtime::JsRuntime;

#[test]
fn test_dom_get_element_and_modify_text() {
    let html = r#"<html><body><h1 id="title">Old Title</h1></body></html>"#;
    let doc = parse_html(html);
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let script = r#"
        var el = document.getElementById("title");
        console.log("Found title:", el.textContent);
        el.textContent = "New Dynamic Title!";
    "#;
    assert!(rt.execute_script(script).is_ok());
    assert!(rt.is_dom_dirty());

    let updated_doc = rt.document_snapshot();
    let root = updated_doc.root();
    let h1_node = updated_doc.find_element_by_tag(root, "h1").unwrap();
    assert_eq!(updated_doc.text_content(h1_node), "New Dynamic Title!");
}

#[test]
fn test_dom_create_element_and_append() {
    let html = r#"<html><body><div id="container"></div></body></html>"#;
    let doc = parse_html(html);
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let script = r#"
        var container = document.getElementById("container");
        var p = document.createElement("p");
        p.textContent = "Inserted Paragraph";
        p.setAttribute("class", "lead");
        container.appendChild(p);
    "#;
    assert!(rt.execute_script(script).is_ok());
    assert!(rt.is_dom_dirty());

    let updated_doc = rt.document_snapshot();
    let root = updated_doc.root();
    let p_node = updated_doc.find_element_by_tag(root, "p").unwrap();
    assert_eq!(updated_doc.text_content(p_node), "Inserted Paragraph");
    if let mango_html::dom::NodeData::Element(ref elem) = updated_doc.get(p_node).unwrap().data {
        assert_eq!(elem.get_attribute("class"), Some("lead"));
    } else {
        panic!("Expected element node");
    }
}

#[test]
fn test_dom_selectors_matches_closest_and_compat_mode() {
    let html = r##"<!DOCTYPE html>
<html>
<body>
    <div id="wrapper" class="container main-content">
        <ul id="list">
            <li class="item active"><a href="#1" class="link">Link 1</a></li>
            <li class="item"><a href="#2" class="link">Link 2</a></li>
        </ul>
    </div>
</body>
</html>"##;
    let doc = parse_html(html);
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let script = r##"
        if (document.compatMode !== "CSS1Compat") {
            throw new Error("compatMode should be CSS1Compat, got " + document.compatMode);
        }

        // querySelector with complex selector
        var activeLink = document.querySelector("#list > li.active a.link");
        if (!activeLink) throw new Error("querySelector activeLink not found");
        if (activeLink.textContent !== "Link 1") throw new Error("wrong textContent: " + activeLink.textContent);

        // matches
        if (!activeLink.matches("a.link")) throw new Error("activeLink should match a.link");
        if (activeLink.matches("div")) throw new Error("activeLink should not match div");

        // closest
        var parentLi = activeLink.closest("li.item");
        if (!parentLi) throw new Error("closest li.item not found");
        if (!parentLi.classList.contains("active")) {
            throw new Error("closest returned wrong li");
        }
        var container = activeLink.closest("#wrapper");
        if (!container || container.id !== "wrapper") throw new Error("closest #wrapper failed");

        // querySelectorAll
        var allItems = document.querySelectorAll("ul > li.item");
        if (allItems.length !== 2) throw new Error("querySelectorAll returned length " + allItems.length);

        // Tree traversal
        var list = document.getElementById("list");
        if (!list) throw new Error("list not found");
        var firstLi = list.firstElementChild;
        if (!firstLi || !firstLi.classList.contains("active")) throw new Error("firstElementChild failed");
        var nextLi = firstLi.nextElementSibling;
        if (!nextLi || nextLi.classList.contains("active")) throw new Error("nextElementSibling failed");
        var prevLi = nextLi.previousElementSibling;
        if (!prevLi || prevLi._nodeId !== firstLi._nodeId) throw new Error("previousElementSibling failed");
        if (firstLi.parentElement._nodeId !== list._nodeId) throw new Error("parentElement failed");
    "##;
    if let Err(e) = rt.execute_script(script) {
        panic!("Selector & navigation test failed: {}", e);
    }

    // Quirks mode document (missing DOCTYPE)
    let quirks_doc = parse_html("<html><body><p>Hello</p></body></html>");
    let mut quirks_rt = JsRuntime::new(quirks_doc, 800.0, 600.0);
    let quirks_script = r#"
        if (document.compatMode !== "BackCompat") {
            throw new Error("compatMode in quirks mode should be BackCompat, got " + document.compatMode);
        }
    "#;
    if let Err(e) = quirks_rt.execute_script(quirks_script) {
        panic!("Quirks compatMode test failed: {}", e);
    }
}

#[test]
fn test_document_character_set_and_content_type() {
    let mut doc = parse_html(
        "<!DOCTYPE html><html><head><meta charset=\"windows-1252\"></head><body></body></html>",
    );
    assert_eq!(doc.character_set, "windows-1252");
    doc.content_type = "text/html".to_string();

    let mut rt = JsRuntime::new(doc, 800.0, 600.0);
    let script = r#"
        if (document.characterSet !== "windows-1252") {
            throw new Error("expected windows-1252, got " + document.characterSet);
        }
        if (document.charset !== "windows-1252") {
            throw new Error("expected charset windows-1252, got " + document.charset);
        }
        if (document.inputEncoding !== "windows-1252") {
            throw new Error("expected inputEncoding windows-1252, got " + document.inputEncoding);
        }
        if (document.contentType !== "text/html") {
            throw new Error("expected contentType text/html, got " + document.contentType);
        }
    "#;
    if let Err(e) = rt.execute_script(script) {
        panic!("document characterSet test failed: {}", e);
    }
}

#[test]
fn test_dom_api_completeness() {
    let html = r#"<!DOCTYPE html>
<html>
<head>
    <title>Section 5.5 Test</title>
</head>
<body>
    <form id="f1" name="formOne"><input name="user" value="alice"></form>
    <img id="img1" src="pic.png" alt="Pic">
    <a id="l1" href="https://example.com">Link 1</a>
    <a id="l2">Anchor Without Href</a>
    <map name="map1"><area id="ar1" href="https://example.com/area" shape="rect" coords="0,0,10,10"></map>
    <div id="container" data-initial-state="ready" data-count-num="42"><p id="p1">First</p><!-- A comment -->Middle Text<p id="p2">Second</p></div>
</body>
</html>"#;
    let doc = parse_html(html);
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let script = r##"
        // 1. document.documentElement, document.head, document.body
        if (!document.documentElement || document.documentElement.tagName.toLowerCase() !== 'html') {
            throw new Error("document.documentElement failed: " + (document.documentElement ? document.documentElement.tagName : null));
        }
        if (!document.head || document.head.tagName.toLowerCase() !== 'head') {
            throw new Error("document.head failed");
        }
        if (!document.body || document.body.tagName.toLowerCase() !== 'body') {
            throw new Error("document.body failed");
        }

        // 2. document.createTextNode()
        var tn = document.createTextNode("Hello World");
        if (!tn || tn.nodeType !== 3) throw new Error("createTextNode nodeType should be 3");
        if (tn.nodeValue !== "Hello World") throw new Error("createTextNode nodeValue mismatch: " + tn.nodeValue);
        if (tn.textContent !== "Hello World") throw new Error("createTextNode textContent mismatch: " + tn.textContent);

        // 3. document.createDocumentFragment()
        var frag = document.createDocumentFragment();
        if (!frag || frag.nodeType !== 11) throw new Error("createDocumentFragment nodeType should be 11");
        var fChild1 = document.createElement("span");
        fChild1.id = "fc1";
        fChild1.textContent = "Frag 1";
        var fChild2 = document.createElement("span");
        fChild2.id = "fc2";
        fChild2.textContent = "Frag 2";
        frag.appendChild(fChild1);
        frag.appendChild(fChild2);
        if (frag.childNodes.length !== 2) throw new Error("frag childNodes length should be 2");

        // 4. element.children vs element.childNodes
        var container = document.getElementById("container");
        if (!container) throw new Error("container not found");
        // children should be element-only HTMLCollection (p1, p2)
        if (container.children.length !== 2) throw new Error("container.children.length should be 2, got: " + container.children.length);
        if (container.children[0].id !== "p1" || container.children[1].id !== "p2") throw new Error("container.children elements mismatch");
        if (container.children.item(0).id !== "p1") throw new Error("container.children.item(0) mismatch");
        // childNodes contains elements, comments, text nodes
        if (container.childNodes.length !== 4) throw new Error("container.childNodes.length should be 4, got: " + container.childNodes.length);
        if (container.firstChild.id !== "p1") throw new Error("container.firstChild should be p1");
        if (container.lastChild.id !== "p2") throw new Error("container.lastChild should be p2");

        // 5. Node navigation (nextSibling, previousSibling, firstElementChild, etc.)
        var p1 = document.getElementById("p1");
        if (!p1.nextSibling || p1.nextSibling.nodeType !== 8) throw new Error("p1.nextSibling should be Comment (8)");
        if (p1.nextElementSibling.id !== "p2") throw new Error("p1.nextElementSibling should be p2");
        var p2 = document.getElementById("p2");
        if (!p2.previousSibling || p2.previousSibling.nodeType !== 3) throw new Error("p2.previousSibling should be Text (3)");
        if (p2.previousElementSibling.id !== "p1") throw new Error("p2.previousElementSibling should be p1");

        // 6. element.insertBefore() with regular node and DocumentFragment
        var ins1 = document.createElement("b");
        ins1.id = "ins1";
        container.insertBefore(ins1, p2);
        if (ins1.nextSibling !== p2 && ins1.nextElementSibling !== p2) throw new Error("ins1 not inserted before p2");

        // Insert fragment into container before p1
        container.insertBefore(frag, p1);
        if (document.getElementById("fc1") === null || document.getElementById("fc2") === null) {
            throw new Error("fragment children fc1/fc2 not in document");
        }
        if (container.firstElementChild.id !== "fc1") throw new Error("first child after frag insertion should be fc1");
        // Fragment should be emptied per spec
        if (frag.childNodes.length !== 0) throw new Error("frag childNodes should be 0 after insertion, got: " + frag.childNodes.length);

        // 7. node.replaceChild()
        var repNew = document.createElement("i");
        repNew.id = "repNew";
        var oldChild = container.replaceChild(repNew, ins1);
        if (oldChild.id !== "ins1") throw new Error("replaceChild return value should be oldChild");
        if (document.getElementById("ins1") !== null) throw new Error("oldChild should no longer be in document");
        if (document.getElementById("repNew") === null) throw new Error("repNew should be in document");

        // replaceChild with DocumentFragment
        var repFrag = document.createDocumentFragment();
        var rf1 = document.createElement("u"); rf1.id = "rf1";
        var rf2 = document.createElement("u"); rf2.id = "rf2";
        repFrag.appendChild(rf1);
        repFrag.appendChild(rf2);
        container.replaceChild(repFrag, repNew);
        if (document.getElementById("repNew") !== null) throw new Error("repNew should be replaced");
        if (!document.getElementById("rf1") || !document.getElementById("rf2")) throw new Error("repFrag children missing");

        // 8. element.insertAdjacentElement()
        var target = document.getElementById("rf1");
        var bb = document.createElement("span"); bb.id = "adj_bb";
        var ab = document.createElement("span"); ab.id = "adj_ab";
        var be = document.createElement("span"); be.id = "adj_be";
        var ae = document.createElement("span"); ae.id = "adj_ae";

        target.insertAdjacentElement("beforebegin", bb);
        target.insertAdjacentElement("afterbegin", ab);
        target.insertAdjacentElement("beforeend", be);
        target.insertAdjacentElement("afterend", ae);

        if (target.previousElementSibling.id !== "adj_bb") throw new Error("insertAdjacentElement beforebegin failed");
        if (target.firstElementChild.id !== "adj_ab") throw new Error("insertAdjacentElement afterbegin failed");
        if (target.lastElementChild.id !== "adj_be") throw new Error("insertAdjacentElement beforeend failed");
        if (target.nextElementSibling.id !== "adj_ae") throw new Error("insertAdjacentElement afterend failed");

        // 9. element.insertAdjacentHTML()
        target.insertAdjacentHTML("beforebegin", "<em id='html_bb'>HBB</em>");
        target.insertAdjacentHTML("afterbegin", "<em id='html_ab'>HAB</em>");
        target.insertAdjacentHTML("beforeend", "<em id='html_be'>HBE</em>");
        target.insertAdjacentHTML("afterend", "<em id='html_ae'>HAE</em>");

        if (!document.getElementById("html_bb") || !document.getElementById("html_ab") ||
            !document.getElementById("html_be") || !document.getElementById("html_ae")) {
            throw new Error("insertAdjacentHTML elements missing");
        }

        // 10. element.innerHTML (get)
        var p1Html = p1.innerHTML;
        if (p1Html !== "First") throw new Error("p1.innerHTML get mismatch: " + p1Html);

        // 11. element.outerHTML (get & set)
        var p1Outer = p1.outerHTML;
        if (p1Outer.indexOf('<p id="p1">First</p>') === -1 && p1Outer.indexOf("<p id='p1'>First</p>") === -1) {
            throw new Error("p1.outerHTML get mismatch: " + p1Outer);
        }
        // Set outerHTML and verify in-place replacement
        p1.outerHTML = "<section id='sec1'>Section 1</section>";
        if (document.getElementById("p1") !== null) throw new Error("p1 should be removed by outerHTML setter");
        var sec1 = document.getElementById("sec1");
        if (!sec1 || sec1.textContent !== "Section 1") throw new Error("sec1 not found or text mismatch after outerHTML setter");

        // 12. element.dataset
        if (container.dataset.initialState !== "ready") throw new Error("dataset get initial-state failed: " + container.dataset.initialState);
        if (container.dataset.countNum !== "42") throw new Error("dataset get count-num failed: " + container.dataset.countNum);
        container.dataset.newProp = "testValue";
        if (container.getAttribute("data-new-prop") !== "testValue") throw new Error("dataset set new-prop failed: " + container.getAttribute("data-new-prop"));
        if (!('newProp' in container.dataset)) throw new Error("'newProp' in dataset should be true");
        delete container.dataset.newProp;
        if (container.hasAttribute("data-new-prop")) throw new Error("delete dataset property should remove attribute");

        // 13. NodeList / HTMLCollection live collections
        var pTags = document.getElementsByTagName("p");
        var initialPLength = pTags.length;
        var newP = document.createElement("p");
        document.body.appendChild(newP);
        if (pTags.length !== initialPLength + 1) throw new Error("getElementsByTagName should be live: " + pTags.length + " vs " + (initialPLength + 1));
        // Iteration
        var countIter = 0;
        for (var pItem of pTags) {
            if (pItem && pItem.tagName.toLowerCase() === 'p') countIter++;
        }
        if (countIter !== pTags.length) throw new Error("HTMLCollection iteration count mismatch");

        // 14. document.forms, document.images, document.links
        if (!document.forms || document.forms.length !== 1 || document.forms[0].id !== "f1") {
            throw new Error("document.forms mismatch: " + (document.forms ? document.forms.length : null));
        }
        if (document.forms.namedItem("formOne") !== document.forms[0]) {
            throw new Error("document.forms.namedItem mismatch");
        }
        if (!document.images || document.images.length !== 1 || document.images[0].id !== "img1") {
            throw new Error("document.images mismatch: " + (document.images ? document.images.length : null));
        }
        // document.links: only <a> and <area> WITH href (l1 and ar1, NOT l2)
        if (!document.links || document.links.length !== 2) {
            throw new Error("document.links length should be 2 (l1 and ar1), got: " + (document.links ? document.links.length : null));
        }
        var linkIds = [document.links[0].id, document.links[1].id];
        if (linkIds.indexOf("l1") === -1 || linkIds.indexOf("ar1") === -1 || linkIds.indexOf("l2") !== -1) {
            throw new Error("document.links items incorrect: " + JSON.stringify(linkIds));
        }

        // 15. node.cloneNode(deep)
        var cloneShallow = container.cloneNode(false);
        if (!cloneShallow || cloneShallow.id !== "container") throw new Error("cloneNode(false) id mismatch");
        if (cloneShallow.childNodes.length !== 0) throw new Error("cloneNode(false) should have no children, got: " + cloneShallow.childNodes.length);

        var cloneDeep = container.cloneNode(true);
        if (!cloneDeep || cloneDeep.childNodes.length === 0) throw new Error("cloneNode(true) should have children");
        if (cloneDeep.querySelector("#sec1") === null) throw new Error("cloneNode(true) subtree missing #sec1");

        // Clone <template> with content
        var tmpl = document.createElement("template");
        var spanInside = document.createElement("span");
        spanInside.id = "insideTemplate";
        tmpl.content.appendChild(spanInside);
        var tmplCloned = tmpl.cloneNode(true);
        if (!tmplCloned.content || tmplCloned.content.childNodes.length !== 1) {
            throw new Error("tmpl.cloneNode(true) content not cloned properly");
        }
        var qRes = tmplCloned.content.querySelector("#insideTemplate");
        if (!qRes || qRes.id !== "insideTemplate") {
            throw new Error("tmpl.cloneNode(true) cloned content missing querySelector #insideTemplate");
        }
    "##;

    if let Err(e) = rt.execute_script(script) {
        panic!("test_dom_api_completeness failed: {}", e);
    }
}

#[test]
fn test_selectors_level_4_dom_bindings() {
    let html = r##"<!DOCTYPE html>
<html>
<head><title>Selectors 4 Test</title></head>
<body>
  <div id="target-elem" data-mango-target="true">Target Section</div>
  <div id="card1" class="card">
    <h2 class="title">Heading 1</h2>
    <p class="desc">First description</p>
    <span class="badge">New</span>
  </div>
  <div id="card2" class="card">
    <h2 class="title">Heading 2</h2>
    <!-- Card with empty box -->
    <div id="empty-box">   <!-- comment --> </div>
  </div>

  <ul id="items">
    <li class="item featured" id="it1">Item 1</li>
    <li class="item" id="it2">Item 2</li>
    <li class="item featured" id="it3">Item 3</li>
    <li class="item" id="it4">Item 4</li>
    <li class="item featured" id="it5">Item 5</li>
  </ul>

  <div id="solo-box">
    <span id="only-span">Only child span</span>
  </div>

  <form id="test-form">
    <input type="text" id="uname" required placeholder="Your name" value="">
    <input type="email" id="uemail" required value="invalid-email">
    <input type="text" id="filled" placeholder="Nickname" value="mango_user">
    <input type="checkbox" id="chk" checked>
    <input type="text" id="dis" disabled value="locked">
    <input type="text" id="ro" readonly value="read only text">
  </form>
</body>
</html>"##;
    let doc = parse_html(html);
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let script = r##"
        // 1. :is() and :where()
        var h = document.querySelector(":is(.card, .box) .title");
        if (!h || h.textContent !== "Heading 1") throw new Error(":is() querySelector failed");

        var w = document.querySelector(":where(#card1, #card2) .desc");
        if (!w || w.textContent !== "First description") throw new Error(":where() querySelector failed");

        // 2. :not() with complex/compound selector
        var nonFeatured = document.querySelectorAll("ul#items > li:not(.featured)");
        if (nonFeatured.length !== 2) throw new Error(":not(.featured) length: " + nonFeatured.length);
        if (nonFeatured[0].id !== "it2" || nonFeatured[1].id !== "it4") {
            throw new Error(":not(.featured) elements incorrect");
        }

        // 3. :has() relative child, sibling, and descendant
        var cardWithBadge = document.querySelector(".card:has(> span.badge)");
        if (!cardWithBadge || cardWithBadge.id !== "card1") throw new Error(":has(> span.badge) failed");

        var headingWithNextP = document.querySelector("h2:has(+ p.desc)");
        if (!headingWithNextP || headingWithNextP.textContent !== "Heading 1") {
            throw new Error("h2:has(+ p.desc) failed");
        }

        var cardWithoutP = document.querySelector(".card:has(#empty-box)");
        if (!cardWithoutP || cardWithoutP.id !== "card2") throw new Error(".card:has(#empty-box) failed");

        // 4. :nth-child(An+B of S)
        var secondFeatured = document.querySelector("li:nth-child(2 of .featured)");
        if (!secondFeatured || secondFeatured.id !== "it3") throw new Error("nth-child(2 of .featured) failed: " + (secondFeatured ? secondFeatured.id : "null"));

        var lastFeatured = document.querySelector("li:nth-last-child(1 of .featured)");
        if (!lastFeatured || lastFeatured.id !== "it5") throw new Error("nth-last-child(1 of .featured) failed: " + (lastFeatured ? lastFeatured.id : "null"));

        // 5. :only-child, :first-of-type, :last-of-type
        var onlySpan = document.querySelector("#solo-box > span:only-child");
        if (!onlySpan || onlySpan.id !== "only-span") throw new Error(":only-child failed");

        var firstLi = document.querySelector("#items > li:first-of-type");
        if (!firstLi || firstLi.id !== "it1") throw new Error(":first-of-type failed");

        // 6. :empty
        var emptyBox = document.querySelector("#empty-box:empty");
        if (!emptyBox) throw new Error(":empty failed for whitespace/comment-only node");

        // 7. :target
        var targetElem = document.querySelector("#target-elem:target");
        if (!targetElem) throw new Error(":target failed");

        // 8. :placeholder-shown
        var emptyInput = document.querySelector("input:placeholder-shown");
        if (!emptyInput || emptyInput.id !== "uname") throw new Error(":placeholder-shown failed: " + (emptyInput ? emptyInput.id : "null"));

        // 9. :checked, :disabled, :enabled
        var chk = document.querySelector("input:checked");
        if (!chk || chk.id !== "chk") throw new Error(":checked failed");

        var dis = document.querySelector("input:disabled");
        if (!dis || dis.id !== "dis") throw new Error(":disabled failed");

        var en = document.querySelectorAll("form input:enabled");
        if (en.length !== 5) throw new Error(":enabled count: " + en.length);

        // 10. :required, :optional, :invalid
        var req = document.querySelectorAll("input:required");
        if (req.length !== 2) throw new Error(":required count: " + req.length);

        var opt = document.querySelectorAll("form input:optional");
        if (opt.length !== 4) throw new Error(":optional count: " + opt.length);

        var inv = document.querySelectorAll("input:invalid");
        if (inv.length !== 2) throw new Error(":invalid count: " + inv.length);
        if (inv[0].id !== "uname" || inv[1].id !== "uemail") throw new Error(":invalid elements incorrect");

        // 11. :read-only, :read-write
        var ro = document.querySelectorAll("form input:read-only");
        if (ro.length !== 3) throw new Error("input:read-only count: " + ro.length);
        if (ro[0].id !== "chk" || ro[1].id !== "dis" || ro[2].id !== "ro") {
            throw new Error("input:read-only elements incorrect: " + ro[0].id + ", " + ro[1].id + ", " + ro[2].id);
        }

        var rw = document.querySelectorAll("form input:read-write");
        if (rw.length !== 3) throw new Error("input:read-write count: " + rw.length);
        if (rw[0].id !== "uname" || rw[1].id !== "uemail" || rw[2].id !== "filled") {
            throw new Error("input:read-write elements incorrect");
        }

        // 12. element.matches()
        if (!cardWithBadge.matches(".card:has(.badge)")) throw new Error("element.matches(:has) failed");
        if (!secondFeatured.matches(":nth-child(2 of .featured)")) throw new Error("element.matches(nth-child of S) failed");
        if (cardWithBadge.matches(":empty")) throw new Error("cardWithBadge should not match :empty");

        // 13. element.closest()
        var desc = document.querySelector(".desc");
        var foundCard = desc.closest(".card:has(.badge)");
        if (!foundCard || foundCard.id !== "card1") throw new Error("element.closest(:has) failed");
    "##;

    if let Err(e) = rt.execute_script(script) {
        panic!("test_selectors_level_4_dom_bindings failed: {}", e);
    }
}

#[test]
fn test_scope_computed_style() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head>
        <style>
          :where(main *) {
            background-color: black;
          }
        </style>
        </head>
        <body>
        <main id="main">
          <div class="a"><span>green</span></div>
          <div class="b"><span>not green</span></div>
          <span>not green</span>
        </main>
        </body>
        </html>
    "#;
    let doc = parse_html(html);
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);
    let script = r#"
        var main = document.getElementById("main");
        var b_span = main.querySelector(".b > span");
        var scope_span = main.querySelector(":scope > span");
        if (!b_span) throw new Error("b_span is null");
        if (!scope_span) throw new Error("scope_span is null");
        var b_cs = getComputedStyle(b_span).backgroundColor;
        var scope_cs = getComputedStyle(scope_span).backgroundColor;
        if (b_cs !== "rgb(0, 0, 0)") throw new Error("b_cs: " + b_cs);
        if (scope_cs !== "rgb(0, 0, 0)") throw new Error("scope_cs: " + scope_cs);
    "#;
    if let Err(e) = rt.execute_script(script) {
        panic!("test_scope_computed_style failed: {}", e);
    }
}

#[test]
fn test_scope_template_clone() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head>
        <style>
          :where(main *) {
            background-color: black;
          }
        </style>
        </head>
        <body>
        <main id="main">
        </main>
        <template id="tmpl">
          <style>
            @scope (.a) {
              span { background-color: green; }
            }
          </style>
          <div class="a">
            <span>green</span>
          </div>
          <div class="b">
            <span>not green</span>
          </div>
          <span>not green</span>
        </template>
        </body>
        </html>
    "#;
    let doc = parse_html(html);
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);
    let script = r#"
        var main = document.getElementById("main");
        var tmpl = document.getElementById("tmpl");
        main.append(tmpl.content.cloneNode(true));
        var info = [];
        for (var i = 0; i < main.childNodes.length; i++) {
            var c = main.childNodes[i];
            info.push((c.tagName || c.nodeName) + '#' + (c.id || '') + '.' + (c.className || ''));
        }
        var a_span = main.querySelector(".a > span");
        var b_span = main.querySelector(".b > span");
        var scope_span = main.querySelector(":scope > span");
        if (!scope_span) throw new Error("scope_span is null, main has " + main.childNodes.length + " kids: " + info.join(", ") + "; innerHTML: " + main.innerHTML);
    "#;
    if let Err(e) = rt.execute_script(script) {
        panic!("test_scope_template_clone failed: {}", e);
    }
}
