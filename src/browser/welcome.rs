//! Built-in landing pages, test benches, and page content pre-processors.

/// Pre-populates Wikipedia Vector 2022 appearance controls (Text size, Width, Color)
/// if the server HTML contains an empty `#vector-appearance` container.
pub fn preprocess_wikipedia_appearance_html(html: &mut String, url: &str) {
    // Only apply Wikipedia appearance preprocessing to legitimate Wikipedia origins (H-09)
    let is_wikipedia = mango_net::Url::parse(url)
        .ok()
        .and_then(|u| u.host)
        .map(|h| {
            let h = h.to_ascii_lowercase();
            h == "wikipedia.org" || h.ends_with(".wikipedia.org")
        })
        .unwrap_or(false);

    if !is_wikipedia {
        return;
    }

    *html = html.replacen("client-nojs", "client-js", 1);
    if html.contains("vector-appearance") && !html.contains("vector-appearance-content") {
        let content_html = r#"<div id="vector-appearance-content" class="vector-appearance-content" style="padding: 4px 0; font-family: sans-serif; color: #202122; font-size: 14px;">
    <div style="padding: 12px 0 8px 0; border-bottom: 1px solid #eaecf0;">
        <div style="color: #54595d; font-size: 13px; margin-bottom: 8px;">Text</div>
        <div style="display: flex; align-items: center; margin: 6px 0; gap: 8px; cursor: pointer;">
            <input type="radio" id="text-size-small" name="text-size" value="small" style="margin: 0; cursor: pointer;">
            <label for="text-size-small" style="font-size: 14px; cursor: pointer;">Small</label>
        </div>
        <div style="display: flex; align-items: center; margin: 6px 0; gap: 8px; cursor: pointer;">
            <input type="radio" id="text-size-standard" name="text-size" value="standard" checked style="margin: 0; cursor: pointer;">
            <label for="text-size-standard" style="font-size: 14px; cursor: pointer;">Standard</label>
        </div>
        <div style="display: flex; align-items: center; margin: 6px 0; gap: 8px; cursor: pointer;">
            <input type="radio" id="text-size-large" name="text-size" value="large" style="margin: 0; cursor: pointer;">
            <label for="text-size-large" style="font-size: 14px; cursor: pointer;">Large</label>
        </div>
    </div>
    <div style="padding: 12px 0 8px 0; border-bottom: 1px solid #eaecf0;">
        <div style="color: #54595d; font-size: 13px; margin-bottom: 8px;">Width</div>
        <div style="display: flex; align-items: center; margin: 6px 0; gap: 8px; cursor: pointer;">
            <input type="radio" id="width-standard" name="page-width" value="standard" checked style="margin: 0; cursor: pointer;">
            <label for="width-standard" style="font-size: 14px; cursor: pointer;">Standard</label>
        </div>
        <div style="display: flex; align-items: center; margin: 6px 0; gap: 8px; cursor: pointer;">
            <input type="radio" id="width-wide" name="page-width" value="wide" style="margin: 0; cursor: pointer;">
            <label for="width-wide" style="font-size: 14px; cursor: pointer;">Wide</label>
        </div>
    </div>
    <div style="padding: 12px 0 8px 0;">
        <div style="color: #54595d; font-size: 13px; margin-bottom: 8px;">Color</div>
        <div style="display: flex; align-items: center; margin: 6px 0; gap: 8px; cursor: pointer;">
            <input type="radio" id="color-auto" name="color-scheme" value="auto" style="margin: 0; cursor: pointer;">
            <label for="color-auto" style="font-size: 14px; cursor: pointer;">Automatic</label>
        </div>
        <div style="display: flex; align-items: center; margin: 6px 0; gap: 8px; cursor: pointer;">
            <input type="radio" id="color-light" name="color-scheme" value="light" checked style="margin: 0; cursor: pointer;">
            <label for="color-light" style="font-size: 14px; cursor: pointer;">Light</label>
        </div>
        <div style="display: flex; align-items: center; margin: 6px 0; gap: 8px; cursor: pointer;">
            <input type="radio" id="color-dark" name="color-scheme" value="dark" style="margin: 0; cursor: pointer;">
            <label for="color-dark" style="font-size: 14px; cursor: pointer;">Dark</label>
        </div>
    </div>
</div>"#;

        let insert_pos = html
            .find("vector-appearance-pinnable-header")
            .and_then(|pos| {
                html[pos..].find("hide</button>").and_then(|hide_pos| {
                    html[pos + hide_pos..]
                        .find("</div>")
                        .map(|close_div| pos + hide_pos + close_div + 6)
                })
            });
        if let Some(insert_pos) = insert_pos {
            html.insert_str(insert_pos, content_html);
        }
    }
}

pub fn resolve_navigation_input(input: &str) -> String {
    if input.starts_with('<') {
        input.to_string()
    } else if let Some(html) = input.strip_prefix("data:text/html,") {
        html.to_string()
    } else if input == "about:blank" {
        "<!DOCTYPE html><html><body></body></html>".to_string()
    } else if input == "test:forms" {
        forms_test_html()
    } else if input == "test:svg" {
        svg_test_html()
    } else if input == "test:tables" {
        tables_test_html()
    } else if input == "test:css" {
        css_test_html()
    } else if input == "test:box" {
        box_model_test_html()
    } else if input == "test:inline" {
        inline_test_html()
    } else if input == "test:fonts" {
        fonts_test_html()
    } else if input == "test:image" {
        image_test_html()
    } else if input == "test:net" {
        test_net_html()
    } else {
        welcome_page_html()
    }
}

pub fn test_net_html() -> String {
    format!(
        r#"
        <!DOCTYPE html>
        <html>
        <head>
            <title>Networking Test Bench</title>
            <style>
                body {{ background-color: #f8fafc; margin: 20px; color: #1e293b; }}
                .hero {{
                    background-color: #ffa136;
                    color: #ffffff;
                    padding: 16px;
                    margin-bottom: 14px;
                }}
                h1 {{ font-size: 24px; color: #ffffff; margin: 0px 0px 6px 0px; }}
                .subtitle {{ font-size: 13px; color: #ffffff; margin: 0px; }}
                .card {{
                    background-color: #ffffff;
                    padding: 16px;
                    margin-bottom: 14px;
                    border-top-width: 1px;
                    border-right-width: 1px;
                    border-bottom-width: 1px;
                    border-left-width: 1px;
                    border-top-color: #e2e8f0;
                    border-right-color: #e2e8f0;
                    border-bottom-color: #e2e8f0;
                    border-left-color: #e2e8f0;
                }}
                h2 {{ font-size: 17px; color: #ffa136; margin: 0px 0px 8px 0px; }}
                p {{ font-size: 13px; color: #334155; margin: 4px 0px; }}
                .code {{ color: #0284c7; font-size: 13px; margin: 4px 0px; }}
            </style>
        </head>
        <body>
            <div class="hero">
                <h1>Networking & Resource Engine</h1>
                <p class="subtitle">Pure Rust HTTP/1.1 client, TLS 1.3/1.2 via rustls & webpki-roots, DNS cache & LRU cache</p>
            </div>
            <div class="card">
                <h2>Active Networking Engine</h2>
                <p>Backend: {}</p>
                <p>URL Parser: RFC 3986 with auto-scheme inference</p>
                <p>DNS Cache: Thread-safe TTL cache</p>
                <p>Resource Cache: In-memory LRU cache with HTTP Cache-Control support</p>
            </div>
            <div class="card">
                <h2>Real Websites to Test Live</h2>
                <p class="code">https://httpbin.org/html</p>
                <p class="code">https://example.com</p>
                <p class="code">https://info.cern.ch</p>
            </div>
        </body>
        </html>
        "#,
        mango_net::tls_backend_info()
    )
}

pub fn welcome_page_html() -> String {
    r##"
    <!DOCTYPE html>
    <html>
    <head>
        <title>Mango Browser</title>
        <style>
            body {
                background-color: #ffffff;
                margin: 0;
                padding: 0;
                color: #202124;
                font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
                text-align: center;
            }
            .header-accent {
                height: 4px;
                background-color: #ffa136;
                width: 100%;
            }
            .container {
                max-width: 650px;
                margin: 0 auto;
                padding: 48px 24px 24px 24px;
            }
            .logo {
                font-size: 52px;
                font-weight: bold;
                color: #202124;
                margin-bottom: 4px;
                letter-spacing: -1px;
            }
            .logo-accent {
                color: #ffa136;
            }
            .tagline {
                font-size: 15px;
                color: #5f6368;
                margin-top: 0;
                margin-bottom: 28px;
            }
            .search-box {
                display: block;
                margin: 0 auto 20px auto;
                width: 100%;
            }
            input[type="text"] {
                width: 100%;
                max-width: 520px;
                padding: 12px 20px;
                font-size: 16px;
                color: #202124;
                background-color: #ffffff;
                border: 1px solid #dfe1e5;
                border-radius: 24px;
                box-sizing: border-box;
                outline: none;
            }
            input[type="text"]:focus {
                border-color: #ffa136;
                outline: none;
            }
            .buttons-row {
                margin-top: 18px;
                margin-bottom: 36px;
            }
            button {
                background-color: #f8f9fa;
                color: #3c4043;
                border: 1px solid #dadce0;
                border-radius: 4px;
                padding: 8px 18px;
                font-size: 14px;
                font-weight: 500;
                margin: 0 4px;
                cursor: pointer;
            }
            .btn-primary {
                background-color: #ffa136;
                color: #ffffff;
                border: 1px solid #e08b26;
                font-weight: bold;
            }
            .shortcuts {
                margin-top: 24px;
                padding-top: 20px;
                border-top: 1px solid #eeeeee;
            }
            .shortcuts-title {
                font-size: 13px;
                font-weight: bold;
                color: #70757a;
                text-transform: uppercase;
                letter-spacing: 0.5px;
                margin-bottom: 14px;
            }
            .links-grid {
                margin: 10px 0;
            }
            .link-pill {
                display: inline-block;
                background-color: #f1f3f4;
                color: #1a73e8;
                padding: 8px 16px;
                margin: 4px;
                border-radius: 16px;
                font-size: 13px;
                font-weight: 500;
                text-decoration: none;
            }
            .footer {
                margin-top: 40px;
                font-size: 12px;
                color: #9aa0a6;
            }
        </style>
    </head>
    <body>
        <div class="header-accent"></div>
        <div class="container">
            <div class="logo">
                <svg width="42" height="42" viewBox="0 0 32 32" style="vertical-align: middle; margin-right: 6px;">
                    <path d="M16 3 C9 3, 4 10, 6 19 C8 27, 17 31, 24 26 C29 22, 28 13, 23 7 C20 4, 18 3, 16 3 Z" fill="#ffa136"/>
                    <path d="M17 3 C18 1, 22 1, 23 4 C21 5, 19 5, 17 3 Z" fill="#16a34a"/>
                </svg>
                <span class="logo-accent">Mango</span>
            </div>
            <p class="tagline">Pure Rust Modern Web Browser</p>

            <form action="https://html.duckduckgo.com/html/" method="GET">
                <div class="search-box">
                    <input type="text" name="q" placeholder="Search with DuckDuckGo or type a URL..." autofocus="true" />
                </div>
                <div class="buttons-row">
                    <button type="submit" class="btn-primary">DuckDuckGo Search</button>
                    <button type="button" onclick="window.location.href='https://en.wikipedia.org/wiki/Special:Random'">I'm Feeling Lucky</button>
                </div>
            </form>

            <div class="shortcuts">
                <div class="shortcuts-title">Quick Navigation</div>
                <div class="links-grid">
                    <p>
                        <a class="link-pill" href="https://html.duckduckgo.com/html/">DuckDuckGo</a>
                        <a class="link-pill" href="https://en.wikipedia.org/wiki/Main_Page">Wikipedia</a>
                        <a class="link-pill" href="https://www.rust-lang.org/">Rust Lang</a>
                        <a class="link-pill" href="http://info.cern.ch/hypertext/WWW/TheProject.html">First Website (CERN)</a>
                    </p>
                    <p>
                        <a class="link-pill" href="test:forms">Form Controls</a>
                        <a class="link-pill" href="test:svg">Vector SVG</a>
                        <a class="link-pill" href="test:tables">CSS Tables</a>
                        <a class="link-pill" href="test:fonts">Typography</a>
                        <a class="link-pill" href="test:net">Net Benchmark</a>
                    </p>
                </div>
            </div>

            <div class="footer">
                Built with 100% Pure Safe Rust • Zero C/C++ • Servo & Google Inspired UI
            </div>
        </div>
        <script>
            console.log("Mango welcome page script executed successfully!");
        </script>
    </body>
    </html>
    "##.to_string()
}

pub fn forms_test_html() -> String {
    r##"
    <!DOCTYPE html>
    <html>
    <head>
        <title>Forms & Input Controls - Mango</title>
        <style>
            body { background-color: #f1f5f9; color: #0f172a; font-family: sans-serif; margin: 24px; }
            .card { background-color: #ffffff; border: 1px solid #cbd5e1; border-radius: 8px; padding: 20px; margin-bottom: 20px; }
            h1 { font-size: 24px; color: #0f172a; margin-top: 0; }
            .field { margin-bottom: 14px; }
            label { display: block; font-weight: bold; margin-bottom: 4px; }
            input[type="text"], input[type="password"] { width: 320px; padding: 6px 10px; border: 1px solid #94a3b8; border-radius: 4px; font-size: 14px; }
            textarea { width: 320px; height: 60px; padding: 6px 10px; border: 1px solid #94a3b8; border-radius: 4px; font-size: 13px; }
            select { width: 260px; padding: 4px 8px; border: 1px solid #94a3b8; border-radius: 4px; }
            button { background-color: #ffa136; color: #ffffff; border: 1px solid #d97706; border-radius: 4px; padding: 6px 16px; cursor: pointer; font-weight: bold; }
            .btn-icon { background-color: #0284c7; border-color: #0369a1; }
        </style>
    </head>
    <body>
        <div class="card">
            <h1>Interactive Form Controls & Buttons</h1>
            <p>Click any input to type or test buttons with embedded SVG icons:</p>
            <div class="field">
                <label>User Name:</label>
                <input type="text" value="MangoUser" placeholder="Enter username"/>
            </div>
            <div class="field">
                <label>Password:</label>
                <input type="password" value="secret123" placeholder="Password"/>
            </div>
            <div class="field">
                <label>Comments:</label>
                <textarea placeholder="Write feedback here...">Fast pure-Rust rendering!</textarea>
            </div>
            <div class="field">
                <label><input type="checkbox" checked="true"/> Remember login session</label>
            </div>
            <div class="field">
                <label><input type="radio" checked="true"/> Enable High Performance JIT</label>
            </div>
            <div class="field">
                <label>Rendering Engine:</label>
                <select>
                    <option selected="true">Vector Anti-Aliasing (tiny-skia)</option>
                    <option>Standard Mode</option>
                </select>
            </div>
            <div class="field">
                <button onclick="alert('Submitted successfully!')">
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="#ffffff">
                        <path d="M9 16.2L4.8 12l-1.4 1.4L9 19 21 7l-1.4-1.4L9 16.2z"/>
                    </svg>
                    Submit Form
                </button>
                <button class="btn-icon">
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="#ffffff">
                        <path d="M15.5 14h-.79l-.28-.27C15.41 12.59 16 11.11 16 9.5 16 5.91 13.09 3 9.5 3S3 5.91 3 9.5 5.91 16 9.5 16c1.61 0 3.09-.59 4.23-1.57l.27.28v.79l5 4.99L20.49 19l-4.99-5zm-6 0C7.01 14 5 11.99 5 9.5S7.01 5 9.5 5 14 7.01 14 9.5 14z"/>
                    </svg>
                    Search
                </button>
            </div>
        </div>
    </body>
    </html>
    "##.to_string()
}

pub fn tables_test_html() -> String {
    r#"
    <!DOCTYPE html>
    <html>
    <head>
        <title>Table Formatting Model - Mango</title>
        <style>
            body { background-color: #f8fafc; color: #0f172a; font-family: sans-serif; margin: 24px; }
            .card { background-color: #ffffff; border: 1px solid #e2e8f0; border-radius: 8px; padding: 20px; }
            table { width: 100%; border: 1px solid #cbd5e1; margin-top: 12px; }
            th { background-color: #f1f5f9; border-bottom: 2px solid #cbd5e1; padding: 8px 12px; font-weight: bold; }
            td { border-bottom: 1px solid #e2e8f0; padding: 8px 12px; }
        </style>
    </head>
    <body>
        <div class="card">
            <h1>CSS Table Formatting Model</h1>
            <table>
                <tr>
                    <th>Component</th>
                    <th>Implementation</th>
                    <th>Status</th>
                </tr>
                <tr>
                    <td>HTML5 Parser</td>
                    <td>Pure Rust Tokenizer & Tree Builder</td>
                    <td>100% Passed</td>
                </tr>
                <tr>
                    <td>CSS3 Cascade & Selector</td>
                    <td>Specificity + Property Expansion</td>
                    <td>100% Passed</td>
                </tr>
                <tr>
                    <td>JavaScript Engine</td>
                    <td>Boa Engine + Web API Polyfills</td>
                    <td>100% Passed</td>
                </tr>
                <tr>
                    <td>Table Layout</td>
                    <td>Proportional Column Sizing</td>
                    <td>100% Passed</td>
                </tr>
            </table>
        </div>
    </body>
    </html>
    "#.to_string()
}

pub fn css_test_html() -> String {
    r#"
    <!DOCTYPE html>
    <html>
    <head>
        <title>CSS Enhancements - Mango</title>
        <style>
            body { background-color: #f8fafc; color: #1e293b; font-family: sans-serif; margin: 24px; }
            .rounded-card { background-color: #ffffff; border: 2px solid #ffa136; border-radius: 12px; padding: 18px; margin-bottom: 16px; }
            .opacity-box { background-color: #0284c7; color: #ffffff; opacity: 0.8; padding: 12px; border-radius: 6px; }
            ol { list-style-type: decimal; }
            ul { list-style-type: square; }
        </style>
    </head>
    <body>
        <div class="rounded-card">
            <h1>CSS Enhancements & Styling</h1>
            <p>This card demonstrates border-radius: 12px with custom orange borders.</p>
            <div class="opacity-box">
                Opacity 80% Alpha Blended Overlay
            </div>
            <h3>Ordered List:</h3>
            <ol>
                <li>Item Alpha</li>
                <li>Item Beta</li>
                <li>Item Gamma</li>
            </ol>
            <h3>Unordered List:</h3>
            <ul>
                <li>Bullet One</li>
                <li>Bullet Two</li>
            </ul>
        </div>
    </body>
    </html>
    "#.to_string()
}

pub fn box_model_test_html() -> String {
    r#"
    <!DOCTYPE html>
    <html>
    <head>
        <title>Box Model Test</title>
        <style>
            body { background-color: #ffffff; margin: 16px; }
            h1 { font-size: 24px; color: #ffa136; margin: 0px 0px 12px 0px; }
            .box1 {
                background-color: #ffe0b2;
                padding: 12px;
                margin: 10px 0px 25px 0px;
                border-top-width: 2px;
                border-bottom-width: 2px;
                border-top-color: #ffa136;
                border-bottom-color: #ffa136;
            }
            .box2 {
                background-color: #e0f2fe;
                padding: 12px;
                margin: 25px 0px 10px 0px;
                border-top-width: 2px;
                border-bottom-width: 2px;
                border-top-color: #0284c7;
                border-bottom-color: #0284c7;
            }
            p { font-size: 14px; margin: 4px 0px; }
        </style>
    </head>
    <body>
        <h1>Box Model & Margin Collapsing</h1>
        <div class="box1">
            <p>Box 1: margin-bottom is 25px, padding is 12px.</p>
        </div>
        <div class="box2">
            <p>Box 2: margin-top is 25px. Sibling margins collapse to 25px.</p>
        </div>
    </body>
    </html>
    "#
    .to_string()
}

pub fn inline_test_html() -> String {
    r#"
    <!DOCTYPE html>
    <html>
    <head>
        <title>Inline Formatting Test</title>
        <style>
            body { background-color: #ffffff; margin: 20px; }
            h1 { font-size: 22px; color: #1e1e1e; margin: 0px 0px 12px 0px; }
            p { font-size: 14px; margin: 10px 0px; color: #333333; }
            .highlight { color: #ffa136; }
            .blue { color: #0000ee; }
        </style>
    </head>
    <body>
        <h1>Inline Formatting Context</h1>
        <p>This paragraph demonstrates automatic line breaking and inline formatting in Mango.</p>
        <p>Text wraps naturally across lines based on available container width.</p>
        <p>Different styled spans like <span class="highlight">orange highlight</span> and <span class="blue">blue links</span> are rendered smoothly.</p>
    </body>
    </html>
    "#.to_string()
}

pub fn fonts_test_html() -> String {
    r#"
    <!DOCTYPE html>
    <html>
    <head>
        <title>Font Rendering & Text Styles - Mango</title>
        <style>
            body { background-color: #ffffff; margin: 24px; color: #1e1e1e; font-family: sans-serif; }
            h1 { font-size: 26px; color: #ffa136; margin: 0px 0px 8px 0px; }
            h2 { font-size: 18px; color: #1e293b; margin: 16px 0px 6px 0px; }
            p { font-size: 14px; margin: 6px 0px; color: #334155; }
            .italic { font-style: italic; }
            .oblique { font-style: oblique; }
            .bold { font-weight: bold; }
            .bold-italic { font-weight: bold; font-style: italic; }
            .underline { text-decoration: underline; }
            .line-through { text-decoration: line-through; }
            .overline { text-decoration: overline; }
            .serif { font-family: serif; }
            .mono { font-family: monospace; }
            .box {
                background-color: #f8fafc;
                padding: 14px;
                margin: 10px 0px;
                border-left-width: 4px;
                border-left-color: #ffa136;
                border-top-width: 1px;
                border-right-width: 1px;
                border-bottom-width: 1px;
                border-top-color: #e2e8f0;
                border-right-color: #e2e8f0;
                border-bottom-color: #e2e8f0;
                border-radius: 4px;
            }
        </style>
    </head>
    <body>
        <h1>Font Styles & Text Decorations</h1>
        <p>Pure Rust font rendering with fontdue glyph rasterization, faux-italic shearing, and decorative strokes.</p>
        
        <h2>1. Font Styles (Italic & Oblique)</h2>
        <div class="box">
            <p>Normal style: The quick brown fox jumps over the lazy dog.</p>
            <p class="italic">Italic style: The quick brown fox jumps over the lazy dog.</p>
            <p class="oblique">Oblique style: The quick brown fox jumps over the lazy dog.</p>
            <p class="bold-italic">Bold + Italic: The quick brown fox jumps over the lazy dog.</p>
        </div>

        <h2>2. Text Decorations</h2>
        <div class="box">
            <p class="underline">Underline: Important links and highlighted headings.</p>
            <p class="line-through">Line-Through: Discounted price from $99 down to $49.</p>
            <p class="overline">Overline: Mathematical notation or decorative headers.</p>
        </div>

        <h2>3. Font Families</h2>
        <div class="box">
            <p>Sans-Serif: Modern clean typography for UI elements.</p>
            <p class="serif">Serif: Classic editorial typography for literature and articles.</p>
            <p class="mono">Monospace: fn render_svg() -> Result&lt;Pixmap, Error&gt;</p>
        </div>
    </body>
    </html>
    "#.to_string()
}

pub fn svg_test_html() -> String {
    r##"
    <!DOCTYPE html>
    <html>
    <head>
        <title>Vector SVG Rendering - Mango</title>
        <style>
            body { background-color: #f8fafc; color: #1e293b; font-family: sans-serif; margin: 24px; }
            .card { background-color: #ffffff; border: 1px solid #e2e8f0; border-radius: 8px; padding: 20px; margin-bottom: 18px; }
            h1 { font-size: 24px; color: #ffa136; margin-top: 0; }
            h2 { font-size: 18px; color: #334155; margin-top: 14px; margin-bottom: 8px; }
            p { font-size: 13px; color: #475569; margin: 4px 0px; }
            .icon-row { margin: 12px 0; }
            button { background-color: #ffa136; color: #ffffff; border: 1px solid #d97706; border-radius: 4px; padding: 6px 14px; font-weight: bold; }
        </style>
    </head>
    <body>
        <div class="card">
            <h1>Pure Rust Vector SVG Rendering (tiny-skia)</h1>
            <p>Mango dynamically parses and rasterizes inline vector SVG elements with anti-aliasing.</p>

            <h2>1. Common Vector Icons</h2>
            <div class="icon-row">
                <!-- Search Icon -->
                <svg width="28" height="28" viewBox="0 0 24 24" fill="#0284c7">
                    <path d="M15.5 14h-.79l-.28-.27C15.41 12.59 16 11.11 16 9.5 16 5.91 13.09 3 9.5 3S3 5.91 3 9.5 5.91 16 9.5 16c1.61 0 3.09-.59 4.23-1.57l.27.28v.79l5 4.99L20.49 19l-4.99-5zm-6 0C7.01 14 5 11.99 5 9.5S7.01 5 9.5 5 14 7.01 14 9.5 14z"/>
                </svg>
                <!-- Checkmark Icon -->
                <svg width="28" height="28" viewBox="0 0 24 24" fill="#10b981">
                    <path d="M9 16.2L4.8 12l-1.4 1.4L9 19 21 7l-1.4-1.4L9 16.2z"/>
                </svg>
                <!-- Star Icon -->
                <svg width="28" height="28" viewBox="0 0 24 24" fill="#f59e0b">
                    <path d="M12 17.27L18.18 21l-1.64-7.03L22 9.24l-7.19-.61L12 2 9.19 8.63 2 9.24l5.46 4.73L5.82 21z"/>
                </svg>
                <!-- Home Icon -->
                <svg width="28" height="28" viewBox="0 0 24 24" fill="#6366f1">
                    <path d="M10 20v-6h4v6h5v-8h3L12 3 2 12h3v8z"/>
                </svg>
            </div>

            <h2>2. Basic SVG Shapes</h2>
            <div class="icon-row">
                <svg width="120" height="40" viewBox="0 0 120 40">
                    <rect x="5" y="5" width="30" height="30" fill="#3b82f6" rx="4"/>
                    <circle cx="60" cy="20" r="15" fill="#ef4444"/>
                    <line x1="85" y1="35" x2="115" y2="5" stroke="#10b981" stroke-width="4"/>
                </svg>
            </div>

            <h2>3. SVG Icons Nested in Buttons</h2>
            <div class="icon-row">
                <button>
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="#ffffff">
                        <path d="M19 13h-6v6h-2v-6H5v-2h6V5h2v6h6v2z"/>
                    </svg>
                    New Item
                </button>
            </div>
        </div>
    </body>
    </html>
    "##.to_string()
}

pub fn image_test_html() -> String {
    r#"
    <!DOCTYPE html>
    <html>
    <head>
        <title>Image Rendering Test</title>
        <style>
            body { background-color: #ffffff; margin: 20px; color: #1e1e1e; }
            h1 { font-size: 24px; color: #ffa136; margin: 0px 0px 10px 0px; }
            p { font-size: 14px; margin: 8px 0px; color: #333333; }
            .card {
                background-color: #f8fafc;
                padding: 14px;
                margin: 10px 0px;
                border-top-width: 1px;
                border-right-width: 1px;
                border-bottom-width: 1px;
                border-left-width: 1px;
                border-top-color: #e2e8f0;
                border-right-color: #e2e8f0;
                border-bottom-color: #e2e8f0;
                border-left-color: #e2e8f0;
            }
        </style>
    </head>
    <body>
        <h1>Image Decoding & Rendering</h1>
        <p>Supports PNG, JPEG, GIF, and WebP via data: URIs and graceful placeholder fallback.</p>

        <div class="card">
            <p><b>1. Valid Inline Base64 PNG Image (Scaled 48x48):</b></p>
            <img src="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==" width="48" height="48">
        </div>

        <div class="card">
            <p><b>2. Missing Image Graceful Fallback Placeholder:</b></p>
            <img src="https://example.com/network_image.png" width="64" height="64">
        </div>
    </body>
    </html>
    "#.to_string()
}
