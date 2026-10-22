use mango_html::parse_html;

use crate::runtime::JsRuntime;

#[test]
fn test_document_advanced_apis() {
    let doc = parse_html("<html><body><div id='box'></div></body></html>");
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let script = r#"
        // document.implementation
        var inert = document.implementation.createHTMLDocument("inert");
        if (!inert || inert.title !== "inert") throw new Error("createHTMLDocument failed");

        // document factories
        var svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
        if (!svg || (svg.tagName !== "svg" && svg.tagName !== "SVG")) throw new Error("createElementNS failed");

        var b = document.body;
        var tw = document.createTreeWalker(b, NodeFilter.SHOW_ALL, null);
        if (!tw || tw.root !== b) throw new Error("createTreeWalker failed");

        var evt = document.createEvent("Event");
        evt.initEvent("custom", true, true);
        if (evt.type !== "custom" || !evt.bubbles) throw new Error("createEvent failed");

        if (document.readyState !== "complete") throw new Error("readyState not complete");
    "#;
    if let Err(e) = rt.execute_script(script) {
        panic!("Document test failed: {}", e);
    }
}

#[test]
fn test_html_iframe_element_bindings() {
    let doc = parse_html("<!DOCTYPE html><html><body><div id='container'></div></body></html>");
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let script = r#"
        // Prototype hierarchy
        if (!(new HTMLIFrameElement() instanceof HTMLElement)) throw new Error("HTMLIFrameElement not HTMLElement");
        if (!(new HTMLIFrameElement() instanceof Element)) throw new Error("HTMLIFrameElement not Element");

        // Creation via document.createElement
        var frame = document.createElement("iframe");
        if (!(frame instanceof HTMLIFrameElement)) throw new Error("Created iframe not instanceof HTMLIFrameElement");

        // Attributes
        frame.src = "https://mango-browser.internal/frame";
        if (frame.src !== "https://mango-browser.internal/frame") throw new Error("iframe.src getter/setter failed");

        frame.srcdoc = "<h1>Nested</h1>";
        if (frame.srcdoc !== "<h1>Nested</h1>") throw new Error("iframe.srcdoc getter/setter failed");

        frame.width = "600";
        frame.height = "400";
        if (frame.width !== "600" || frame.height !== "400") throw new Error("iframe dimensions failed");

        // Sandbox DOMTokenList
        frame.sandbox.add("allow-scripts");
        frame.sandbox.add("allow-same-origin");
        if (!frame.sandbox.contains("allow-scripts")) throw new Error("sandbox.contains('allow-scripts') failed");
        if (!frame.sandbox.contains("allow-same-origin")) throw new Error("sandbox.contains('allow-same-origin') failed");
        if (frame.sandbox.contains("allow-popups")) throw new Error("sandbox should not contain allow-popups");

        // contentDocument and contentWindow
        var cDoc = frame.contentDocument;
        if (!cDoc) throw new Error("iframe.contentDocument missing");
        if (cDoc.readyState !== "complete") throw new Error("contentDocument readyState invalid");

        var cWin = frame.contentWindow;
        if (!cWin) throw new Error("iframe.contentWindow missing");
        if (cWin.frameElement !== frame) throw new Error("contentWindow.frameElement mismatch");

        // Append to DOM
        var container = document.getElementById("container");
        container.appendChild(frame);
        if (container.children.length !== 1) throw new Error("appendChild failed");
    "#;

    if let Err(e) = rt.execute_script(script) {
        panic!("HTMLIFrameElement test failed: {}", e);
    }
}

#[test]
fn test_html_video_and_audio_element_bindings() {
    let html = r#"<!DOCTYPE html>
<html>
<body>
    <div id="player-container">
        <video id="vid1" src="https://example.com/movie.mp4" poster="movie.jpg" controls autoplay muted></video>
        <audio id="aud1" src="https://example.com/song.mp3" controls></audio>
    </div>
</body>
</html>"#;
    let doc = parse_html(html);
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let script = r#"
        // 1. Prototype & Constants Verification
        if (HTMLMediaElement.HAVE_ENOUGH_DATA !== 4) throw new Error("HTMLMediaElement.HAVE_ENOUGH_DATA mismatch");
        if (HTMLMediaElement.NETWORK_IDLE !== 1) throw new Error("HTMLMediaElement.NETWORK_IDLE mismatch");

        var vDirect = new HTMLVideoElement();
        if (!(vDirect instanceof HTMLMediaElement)) throw new Error("HTMLVideoElement not instanceof HTMLMediaElement");
        if (!(vDirect instanceof HTMLElement)) throw new Error("HTMLVideoElement not instanceof HTMLElement");

        var aDirect = new HTMLAudioElement();
        if (!(aDirect instanceof HTMLMediaElement)) throw new Error("HTMLAudioElement not instanceof HTMLMediaElement");

        // 2. createElement('video') & Property Getters/Setters
        var vNew = document.createElement("video");
        if (!(vNew instanceof HTMLVideoElement)) throw new Error("createElement('video') not HTMLVideoElement");
        if (!(vNew instanceof HTMLMediaElement)) throw new Error("createElement('video') not HTMLMediaElement");

        vNew.src = "promo.webm";
        if (vNew.src !== "promo.webm" || vNew.currentSrc !== "promo.webm") throw new Error("vNew.src mismatch");

        vNew.poster = "thumb.png";
        if (vNew.poster !== "thumb.png") throw new Error("vNew.poster mismatch");

        vNew.controls = true;
        if (!vNew.controls) throw new Error("vNew.controls should be true");
        vNew.controls = false;
        if (vNew.controls) throw new Error("vNew.controls should be false");

        vNew.muted = true;
        if (!vNew.muted) throw new Error("vNew.muted should be true");

        vNew.currentTime = 42.5;
        if (vNew.currentTime !== 42.5) throw new Error("vNew.currentTime should be 42.5, got " + vNew.currentTime);

        vNew.volume = 0.8;
        if (vNew.volume !== 0.8) throw new Error("vNew.volume should be 0.8, got " + vNew.volume);

        if (vNew.canPlayType("video/mp4") !== "probably") throw new Error("canPlayType('video/mp4') failed");
        if (vNew.canPlayType("audio/mp3") !== "probably") throw new Error("canPlayType('audio/mp3') failed");

        // 3. play() and pause() methods
        vNew.play();
        if (vNew.paused) throw new Error("vNew should not be paused after play()");
        vNew.pause();
        if (!vNew.paused) throw new Error("vNew should be paused after pause()");

        // 4. new Audio(src) Constructor
        var snd = new Audio("alert.wav");
        if (!(snd instanceof HTMLAudioElement)) throw new Error("new Audio not instanceof HTMLAudioElement");
        if (!(snd instanceof HTMLMediaElement)) throw new Error("new Audio not instanceof HTMLMediaElement");
        if (snd.src !== "alert.wav") throw new Error("new Audio src mismatch: " + snd.src);
        snd.play();
        if (snd.paused) throw new Error("snd should not be paused after play()");

        // 5. Existing DOM Video & Audio Elements
        var vid1 = document.getElementById("vid1");
        if (!vid1) throw new Error("vid1 not found in DOM");
        if (!(vid1 instanceof HTMLVideoElement)) throw new Error("vid1 not HTMLVideoElement");
        if (!vid1.controls) throw new Error("vid1 should have controls");
        if (!vid1.autoplay) throw new Error("vid1 should have autoplay");
        if (!vid1.muted) throw new Error("vid1 should be muted");
        if (vid1.poster !== "movie.jpg") throw new Error("vid1 poster mismatch: " + vid1.poster);

        var aud1 = document.getElementById("aud1");
        if (!aud1) throw new Error("aud1 not found in DOM");
        if (!(aud1 instanceof HTMLAudioElement)) throw new Error("aud1 not HTMLAudioElement");
        if (!aud1.controls) throw new Error("aud1 should have controls");
        if (aud1.src !== "https://example.com/song.mp3") throw new Error("aud1.src mismatch: " + aud1.src);
    "#;

    if let Err(e) = rt.execute_script(script) {
        panic!("test_html_video_and_audio_element_bindings failed: {}", e);
    }
}

#[test]
fn test_missing_html_element_bindings() {
    let html = r##"<!DOCTYPE html>
<html>
<body>
    <dialog id="dlg">Dialog content</dialog>
    <progress id="prog" value="50" max="100"></progress>
    <progress id="indet_prog"></progress>
    <meter id="met" value="0.7" min="0" max="1" low="0.25" high="0.75" optimum="0.8"></meter>
    <output id="out" for="a b">Initial</output>
    <input id="inp" list="dl">
    <datalist id="dl">
        <option value="Apple">
        <option value="Banana">
    </datalist>
    <map id="m" name="map1">
        <area shape="rect" coords="0,0,10,10" href="#rect" alt="RectArea">
    </map>
    <object id="obj" data="test.svg" type="image/svg+xml"></object>
    <embed id="emb" src="player.swf" type="application/x-shockwave-flash" width="300" height="200">
</body>
</html>"##;
    let doc = parse_html(html);
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let script = r#"
        // 1. HTMLDialogElement
        var dlg = document.getElementById("dlg");
        if (!dlg || !(dlg instanceof HTMLDialogElement)) throw new Error("dlg not HTMLDialogElement");
        if (dlg.open !== false) throw new Error("dlg should initially be closed");
        dlg.show();
        if (dlg.open !== true) throw new Error("dlg.show() should open dialog");
        if (!dlg.hasAttribute("open")) throw new Error("dlg open attribute missing");

        var closeEventFired = false;
        dlg.addEventListener("close", function(e) {
            closeEventFired = true;
        });
        dlg.close("resultVal");
        if (dlg.open !== false) throw new Error("dlg should be closed after close()");
        if (dlg.returnValue !== "resultVal") throw new Error("dlg.returnValue mismatch: " + dlg.returnValue);
        if (!closeEventFired) throw new Error("close event not fired");

        dlg.showModal();
        if (dlg.open !== true) throw new Error("dlg should be open after showModal()");
        // Calling showModal() when already open should throw InvalidStateError
        var threw = false;
        try {
            dlg.showModal();
        } catch (e) {
            threw = true;
        }
        if (!threw) throw new Error("showModal() should throw when already open");
        dlg.close();

        // 2. HTMLProgressElement
        var prog = document.getElementById("prog");
        if (!prog || !(prog instanceof HTMLProgressElement)) throw new Error("prog not HTMLProgressElement");
        if (prog.value !== 50) throw new Error("prog.value should be 50, got " + prog.value);
        if (prog.max !== 100) throw new Error("prog.max should be 100, got " + prog.max);
        if (Math.abs(prog.position - 0.5) > 0.001) throw new Error("prog.position should be 0.5, got " + prog.position);

        var indet = document.getElementById("indet_prog");
        if (indet.position !== -1) throw new Error("indeterminate progress position should be -1, got " + indet.position);
        indet.value = 25;
        if (indet.value !== 25) throw new Error("indet value setter failed");

        // 3. HTMLMeterElement
        var met = document.getElementById("met");
        if (!met || !(met instanceof HTMLMeterElement)) throw new Error("met not HTMLMeterElement");
        if (Math.abs(met.value - 0.7) > 0.001) throw new Error("met.value should be 0.7, got " + met.value);
        if (met.min !== 0) throw new Error("met.min should be 0, got " + met.min);
        if (met.max !== 1) throw new Error("met.max should be 1, got " + met.max);
        if (Math.abs(met.low - 0.25) > 0.001) throw new Error("met.low should be 0.25, got " + met.low);
        if (Math.abs(met.high - 0.75) > 0.001) throw new Error("met.high should be 0.75, got " + met.high);
        if (Math.abs(met.optimum - 0.8) > 0.001) throw new Error("met.optimum should be 0.8, got " + met.optimum);
        met.value = 0.9;
        if (Math.abs(met.value - 0.9) > 0.001) throw new Error("met.value setter failed");

        // 4. HTMLOutputElement
        var out = document.getElementById("out");
        if (!out || !(out instanceof HTMLOutputElement)) throw new Error("out not HTMLOutputElement");
        if (out.type !== "output") throw new Error("out.type should be 'output'");
        if (out.value !== "Initial") throw new Error("out.value should be 'Initial'");
        if (out.defaultValue !== "Initial") throw new Error("out.defaultValue should be 'Initial'");
        out.value = "Updated";
        if (out.value !== "Updated") throw new Error("out.value setter failed");
        if (out.htmlFor !== "a b") throw new Error("out.htmlFor mismatch: " + out.htmlFor);

        // 5. HTMLDataListElement and input.list
        var dl = document.getElementById("dl");
        if (!dl || !(dl instanceof HTMLDataListElement)) throw new Error("dl not HTMLDataListElement");
        if (!dl.options || dl.options.length !== 2) throw new Error("dl.options length should be 2, got " + (dl.options ? dl.options.length : null));
        var inp = document.getElementById("inp");
        if (!inp.list || inp.list !== dl) throw new Error("inp.list should point to dl");

        // 6. HTMLMapElement & HTMLAreaElement
        var m = document.getElementById("m");
        if (!m || !(m instanceof HTMLMapElement)) throw new Error("m not HTMLMapElement");
        if (!m.areas || m.areas.length !== 1) throw new Error("m.areas length should be 1, got " + (m.areas ? m.areas.length : null));
        var area = m.areas[0];
        if (!(area instanceof HTMLAreaElement)) throw new Error("area not HTMLAreaElement");
        if (area.shape !== "rect") throw new Error("area.shape mismatch: " + area.shape);
        if (area.coords !== "0,0,10,10") throw new Error("area.coords mismatch: " + area.coords);
        if (area.alt !== "RectArea") throw new Error("area.alt mismatch: " + area.alt);

        // 7. HTMLObjectElement & HTMLEmbedElement
        var obj = document.getElementById("obj");
        if (!obj || !(obj instanceof HTMLObjectElement)) throw new Error("obj not HTMLObjectElement");
        if (obj.data !== "test.svg") throw new Error("obj.data mismatch: " + obj.data);
        if (obj.type !== "image/svg+xml") throw new Error("obj.type mismatch: " + obj.type);

        var emb = document.getElementById("emb");
        if (!emb || !(emb instanceof HTMLEmbedElement)) throw new Error("emb not HTMLEmbedElement");
        if (emb.src !== "player.swf") throw new Error("emb.src mismatch: " + emb.src);
        if (emb.width !== "300" || emb.height !== "200") throw new Error("emb dims mismatch");
    "#;

    if let Err(e) = rt.execute_script(script) {
        panic!("test_missing_html_element_bindings failed: {}", e);
    }
}

#[test]
fn test_form_elements_and_validation() {
    let html = r##"<!DOCTYPE html>
<html>
<body>
    <form id="f1" action="/test">
        <fieldset id="fs1">
            <legend id="leg1">Personal Info</legend>
            <input id="num" type="number" min="10" max="100" step="5" value="20" required>
            <input id="email" type="email" value="bad-email">
            <input id="url" type="url" value="not-a-url">
            <input id="pat" type="text" pattern="[A-Z]{3}" value="abc">
            <input id="dt" type="date" min="2026-01-01" max="2026-12-31" value="2026-06-15">
            <input id="tm" type="time" value="14:30">
            <input id="clr" type="color" value="#ff5500">
            <input id="rng" type="range" min="0" max="10" step="1" value="5">
            <input id="hid" type="hidden" value="secret">
        </fieldset>
        <select id="sel_single">
            <option id="opt_a" value="apple">Apple</option>
            <option id="opt_b" value="banana" selected>Banana</option>
        </select>
        <select id="sel_multi" multiple size="4">
            <optgroup id="og1" label="Citrus">
                <option id="opt_orange" value="orange" selected>Orange</option>
                <option id="opt_lemon" value="lemon">Lemon</option>
            </optgroup>
            <option id="opt_grape" value="grape" selected>Grape</option>
        </select>
    </form>
</body>
</html>"##;

    let dom = mango_html::parse_html(html);
    let mut rt = JsRuntime::new(dom, 1024.0, 768.0);

    let script = r#"
        // 1. Prototype and subclass validation
        var f = document.getElementById("f1");
        if (!f || !(f instanceof HTMLFormElement)) throw new Error("f not HTMLFormElement");
        var fs = document.getElementById("fs1");
        if (!fs || !(fs instanceof HTMLFieldSetElement)) throw new Error("fs not HTMLFieldSetElement");
        var leg = document.getElementById("leg1");
        if (!leg || !(leg instanceof HTMLLegendElement)) throw new Error("leg not HTMLLegendElement");
        var selS = document.getElementById("sel_single");
        if (!selS || !(selS instanceof HTMLSelectElement)) throw new Error("selS not HTMLSelectElement");
        var selM = document.getElementById("sel_multi");
        if (!selM || !(selM instanceof HTMLSelectElement)) throw new Error("selM not HTMLSelectElement");
        var og = document.getElementById("og1");
        if (!og || !(og instanceof HTMLOptGroupElement)) throw new Error("og not HTMLOptGroupElement");
        var optB = document.getElementById("opt_b");
        if (!optB || !(optB instanceof HTMLOptionElement)) throw new Error("optB not HTMLOptionElement");

        // 2. Hierarchy and references (.form, .elements, .length)
        if (leg.form !== f) throw new Error("leg.form should point to enclosing form");
        if (fs.form !== f) throw new Error("fs.form should point to enclosing form");
        var num = document.getElementById("num");
        if (num.form !== f) throw new Error("num.form should point to form");
        if (!f.elements || f.elements.length < 10) throw new Error("f.elements length mismatch: " + (f.elements ? f.elements.length : null));
        if (f.elements['num'] !== num && f.elements.num !== num) throw new Error("f.elements named item access failed");
        if (f.length !== f.elements.length) throw new Error("f.length mismatch");

        // 3. Number input & stepUp / stepDown
        if (num.value !== "20") throw new Error("num initial value mismatch: " + num.value);
        if (!num.checkValidity()) throw new Error("num=20 should be valid");
        num.stepUp(2); // 20 + 2*5 = 30
        if (num.value !== "30") throw new Error("num after stepUp(2) should be 30, got: " + num.value);
        num.stepDown(1); // 30 - 5 = 25
        if (num.value !== "25") throw new Error("num after stepDown(1) should be 25, got: " + num.value);

        // 4. Form validation API: rangeUnderflow, rangeOverflow, stepMismatch
        num.value = "5"; // Below min=10
        if (num.checkValidity()) throw new Error("num=5 should be invalid (min=10)");
        if (!num.validity.rangeUnderflow || num.validity.valid) throw new Error("num.validity.rangeUnderflow expected");

        num.value = "105"; // Above max=100
        if (num.checkValidity()) throw new Error("num=105 should be invalid (max=100)");
        if (!num.validity.rangeOverflow || num.validity.valid) throw new Error("num.validity.rangeOverflow expected");

        num.value = "22"; // Not multiple of step=5 from min=10
        if (num.checkValidity()) throw new Error("num=22 should be invalid (step=5)");
        if (!num.validity.stepMismatch || num.validity.valid) throw new Error("num.validity.stepMismatch expected");

        num.value = "25"; // Valid again
        if (!num.checkValidity() || !num.validity.valid) throw new Error("num=25 should be valid");

        // 5. Email & URL & Pattern validation
        var email = document.getElementById("email");
        if (email.checkValidity() || !email.validity.typeMismatch) throw new Error("bad-email should trigger typeMismatch");
        email.value = "hello@rust-lang.org";
        if (!email.checkValidity() || email.validity.typeMismatch) throw new Error("valid email rejected");

        var url = document.getElementById("url");
        if (url.checkValidity() || !url.validity.typeMismatch) throw new Error("not-a-url should trigger typeMismatch");
        url.value = "https://www.rust-lang.org/";
        if (!url.checkValidity() || url.validity.typeMismatch) throw new Error("valid url rejected");

        var pat = document.getElementById("pat");
        if (pat.checkValidity() || !pat.validity.patternMismatch) throw new Error("pat='abc' should fail [A-Z]{3}");
        pat.value = "XYZ";
        if (!pat.checkValidity() || pat.validity.patternMismatch) throw new Error("pat='XYZ' should match [A-Z]{3}");

        // Custom validity
        pat.setCustomValidity("Three uppercase letters required!");
        if (pat.checkValidity() || !pat.validity.customError) throw new Error("customError flag not set");
        if (pat.validationMessage !== "Three uppercase letters required!") throw new Error("validationMessage mismatch: " + pat.validationMessage);
        pat.setCustomValidity("");
        if (!pat.checkValidity() || pat.validity.customError) throw new Error("customError should be cleared");

        // Hidden element does not participate in constraint validation
        var hid = document.getElementById("hid");
        if (hid.willValidate) throw new Error("hidden input willValidate should be false");

        // 6. Select (single) & Option
        if (selS.multiple !== false) throw new Error("selS.multiple should be false");
        if (selS.type !== "select-one") throw new Error("selS.type should be select-one");
        if (selS.value !== "banana") throw new Error("selS.value should be banana, got: " + selS.value);
        if (selS.selectedIndex !== 1) throw new Error("selS.selectedIndex should be 1, got: " + selS.selectedIndex);
        selS.value = "apple";
        if (selS.value !== "apple") throw new Error("selS.value should be apple");
        if (selS.selectedIndex !== 0) throw new Error("selS.selectedIndex should be 0");

        // 7. Select (multiple) & OptGroup
        if (selM.multiple !== true) throw new Error("selM.multiple should be true");
        if (selM.type !== "select-multiple") throw new Error("selM.type should be select-multiple");
        if (og.label !== "Citrus") throw new Error("og.label mismatch: " + og.label);
        if (!selM.options || selM.options.length !== 3) throw new Error("selM.options length should be 3, got: " + (selM.options ? selM.options.length : null));
        if (!selM.selectedOptions || selM.selectedOptions.length !== 2) throw new Error("selM.selectedOptions length should be 2, got: " + (selM.selectedOptions ? selM.selectedOptions.length : null));

        // Select add & remove
        var newOpt = document.createElement("option");
        newOpt.value = "lime";
        newOpt.text = "Lime";
        selM.add(newOpt);
        if (selM.options.length !== 4) throw new Error("selM.options.length should be 4 after add()");
        selM.remove(3);
        if (selM.options.length !== 3) throw new Error("selM.options.length should be 3 after remove(3)");

        // 8. Form checkValidity & reset
        if (!f.checkValidity()) throw new Error("form should be valid when all controls valid");
        num.value = "5"; // make invalid
        if (f.checkValidity()) throw new Error("form should be invalid when num=5");
        f.reset();
        if (num.value !== "20") throw new Error("f.reset() should restore num to defaultValue 20, got: " + num.value);
    "#;

    if let Err(e) = rt.execute_script(script) {
        panic!("test_form_elements_and_validation failed: {}", e);
    }
}

#[test]
fn test_html_dom_parity() {
    let html = r#"
        <html>
            <body>
                <div id="editable-parent" contenteditable="true">
                    <span id="editable-child">Editable Child</span>
                    <div id="non-editable-child" contenteditable="false">Non Editable</div>
                </div>
                <div id="drag-item">Drag Me</div>
            </body>
        </html>
    "#;
    let doc = parse_html(html);
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let script = r#"
        (function() {
            var parent = document.getElementById('editable-parent');
            var child = document.getElementById('editable-child');
            var nonEdChild = document.getElementById('non-editable-child');

            // 1. contentEditable & isContentEditable
            if (parent.contentEditable !== 'true') throw new Error("parent contentEditable mismatch: " + parent.contentEditable);
            if (parent.isContentEditable !== true) throw new Error("parent isContentEditable mismatch");
            if (child.contentEditable !== 'inherit') throw new Error("child contentEditable mismatch: " + child.contentEditable);
            if (child.isContentEditable !== true) throw new Error("child isContentEditable should be true via inheritance");
            if (nonEdChild.contentEditable !== 'false') throw new Error("nonEdChild contentEditable mismatch");
            if (nonEdChild.isContentEditable !== false) throw new Error("nonEdChild isContentEditable mismatch");

            child.contentEditable = 'false';
            if (child.isContentEditable !== false) throw new Error("child isContentEditable after setting false mismatch");
            child.contentEditable = 'inherit';
            if (child.isContentEditable !== true) throw new Error("child isContentEditable after restore inherit mismatch");

            // 2. draggable property
            var dragItem = document.getElementById('drag-item');
            if (dragItem.draggable !== false) throw new Error("default draggable mismatch");
            dragItem.draggable = true;
            if (dragItem.draggable !== true) throw new Error("draggable true mismatch");
            if (dragItem.getAttribute('draggable') !== 'true') throw new Error("draggable attribute mismatch");

            // 3. DataTransfer & DragEvent
            var dt = new DataTransfer();
            if (dt.dropEffect !== 'none') throw new Error("dropEffect mismatch");
            dt.setData('text/plain', 'Mango Drag and Drop');
            if (dt.getData('text/plain') !== 'Mango Drag and Drop') throw new Error("getData mismatch");
            if (!dt.types.includes('text/plain')) throw new Error("types mismatch");
            if (dt.items.length !== 1) throw new Error("items.length mismatch");

            var dragEv = new DragEvent('dragstart', { dataTransfer: dt, clientX: 100, clientY: 200 });
            if (!(dragEv instanceof MouseEvent)) throw new Error("DragEvent not instanceof MouseEvent");
            if (!(dragEv instanceof DragEvent)) throw new Error("DragEvent not instanceof DragEvent");
            if (dragEv.clientX !== 100) throw new Error("DragEvent clientX mismatch");
            if (!dragEv.dataTransfer || dragEv.dataTransfer.getData('text/plain') !== 'Mango Drag and Drop') {
                throw new Error("DragEvent dataTransfer mismatch");
            }

            // 4. HTMLMediaElement properties
            var video = document.createElement('video');
            if (typeof video.play !== 'function' || typeof video.pause !== 'function') {
                throw new Error("video play/pause methods missing");
            }
            video.currentTime = 25.5;
            if (video.currentTime !== 25.5) throw new Error("video currentTime mismatch: " + video.currentTime);
            video.volume = 0.75;
            if (video.volume !== 0.75) throw new Error("video volume mismatch: " + video.volume);
            video.muted = true;
            if (video.muted !== true) throw new Error("video muted mismatch");
            video.playbackRate = 2.0;
            if (video.playbackRate !== 2.0) throw new Error("video playbackRate mismatch");

            // 5. HTMLIFrameElement
            var iframe = document.createElement('iframe');
            iframe.src = 'https://example.com/test';
            iframe.srcdoc = '<h1>Hello Mango</h1>';
            iframe.sandbox.add('allow-scripts');
            if (!iframe.sandbox.contains('allow-scripts')) throw new Error("iframe sandbox add mismatch");
            if (!iframe.contentDocument || typeof iframe.contentDocument.createElement !== 'function') {
                throw new Error("iframe contentDocument mismatch");
            }
            if (!iframe.contentWindow || iframe.contentWindow.document !== iframe.contentDocument) {
                throw new Error("iframe contentWindow mismatch");
            }

            return "ok";
        })()
    "#;

    let res = rt.eval(script).expect("eval test script");
    assert_eq!(res, "ok", "Section 8.3 parity assertions failed");
}
