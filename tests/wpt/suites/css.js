// WPT Suite 3: CSS OM, Constructable Stylesheets & Modern Selectors (css/*)

test(function() {
    assert_true(typeof CSSStyleSheet === "function", "CSSStyleSheet constructor should exist");
    const sheet = new CSSStyleSheet();
    assert_true(sheet instanceof CSSStyleSheet, "sheet should be an instance of CSSStyleSheet");

    sheet.replaceSync(".wpt-test { color: red; font-size: 16px; }");
    assert_true(sheet.cssRules.length >= 1, "cssRules should contain parsed rules");
    assert_true(sheet.cssRules[0].cssText.indexOf(".wpt-test") !== -1, "cssText should reflect rule");

    assert_true(Array.isArray(document.adoptedStyleSheets), "document.adoptedStyleSheets should be an array");
    document.adoptedStyleSheets = [sheet];
    assert_equals(document.adoptedStyleSheets.length, 1, "adoptedStyleSheets length after setting");
    document.adoptedStyleSheets = [];
}, "WPT CSS: Constructable Stylesheets (new CSSStyleSheet, replaceSync, adoptedStyleSheets)");

test(function() {
    const root = document.createElement("div");
    root.innerHTML = `
        <article class="card featured">
            <h2 class="title">Headline</h2>
            <p class="body">Content</p>
        </article>
        <article class="card standard">
            <h2 class="title">Other</h2>
        </article>
    `;
    document.body.appendChild(root);

    // CSS Relational :has() selector
    const cardWithBody = root.querySelector(".card:has(p.body)");
    assert_true(cardWithBody !== null, ":has(p.body) should find matching card");
    assert_true(cardWithBody.classList.contains("featured"), "featured card matched :has()");

    // CSS :is() and :where()
    const headlines = root.querySelectorAll(":is(h1, h2, h3).title");
    assert_equals(headlines.length, 2, ":is(h1, h2, h3) should match all titles");

    // CSS :not()
    const nonFeatured = root.querySelectorAll(".card:not(.featured)");
    assert_equals(nonFeatured.length, 1, ":not(.featured) should match standard card");

    document.body.removeChild(root);
}, "WPT CSS: Modern Selectors (:has(), :is(), :not())");

test(function() {
    const el = document.createElement("div");
    el.style.backgroundColor = "oklch(0.65 0.25 140)";
    assert_true(el.style.backgroundColor.length > 0, "OKLCH color should be set and retained");

    el.style.color = "rgb(120, 200, 255)";
    assert_true(el.style.color.indexOf("120") !== -1 || el.style.color.indexOf("rgb") !== -1, "RGB color retained");
}, "WPT CSS: CSS Color 4 & 5 OKLCH and functional notation");
