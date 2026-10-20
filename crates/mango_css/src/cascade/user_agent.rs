//! Built-in HTML5 default User-Agent stylesheet.

use crate::parser::{Stylesheet, parse_stylesheet};

/// The built-in HTML5 default User-Agent stylesheet.
pub fn user_agent_stylesheet() -> &'static Stylesheet {
    static UA_STYLESHEET: std::sync::OnceLock<Stylesheet> = std::sync::OnceLock::new();
    UA_STYLESHEET.get_or_init(|| {
        let css = r#"
            head, title, style, script, link, meta, noscript, template {
                display: none;
            }
            slot {
                display: contents;
            }
            [hidden] {
                display: none;
            }
            html, body, div, p, h1, h2, h3, h4, h5, h6,
            ul, ol, dl, dt, dd, blockquote, pre, form,
            header, footer, nav, section, article,
            aside, main, figure, figcaption,
            center, address, details, summary,
            ytd-app, ytd-masthead, ytd-page-manager, ytd-browse,
            ytd-two-column-browse-results-renderer, ytd-rich-grid-renderer {
                display: block;
            }
            details:not([open]) > :not(summary) {
                display: none;
            }
            body {
                margin: 8px;
                color: black;
                background-color: white;
                font-size: 16px;
                font-family: serif;
                font-weight: normal;
                font-style: normal;
            }
            code, kbd, samp, pre, tt {
                font-family: monospace;
            }
            h1 {
                font-size: 2em;
                font-weight: bold;
                margin: 0.67em 0px;
            }
            h2 {
                font-size: 1.5em;
                font-weight: bold;
                margin: 0.83em 0px;
            }
            h3 {
                font-size: 1.17em;
                font-weight: bold;
                margin: 1em 0px;
            }
            h4 {
                font-size: 1em;
                font-weight: bold;
                margin: 1.33em 0px;
            }
            h5 {
                font-size: 0.83em;
                font-weight: bold;
                margin: 1.67em 0px;
            }
            h6 {
                font-size: 0.67em;
                font-weight: bold;
                margin: 2.33em 0px;
            }
            p {
                margin: 1em 0px;
            }
            b, strong {
                font-weight: bold;
            }
            i, em {
                font-style: italic;
            }
            a {
                color: #0000ee;
                text-decoration: underline;
                cursor: pointer;
            }
            sup {
                vertical-align: super;
                font-size: 0.8em;
            }
            sub {
                vertical-align: sub;
                font-size: 0.8em;
            }
            ul {
                display: block;
                list-style-type: disc;
                margin: 1em 0px;
                padding-left: 40px;
            }
            ol {
                display: block;
                list-style-type: decimal;
                margin: 1em 0px;
                padding-left: 40px;
            }
            li {
                display: list-item;
            }
            center {
                display: block;
                text-align: center;
            }
            center > table {
                margin-left: auto;
                margin-right: auto;
            }
            table {
                display: table;
                margin: 8px 0px;
                text-align: left;
            }
            caption {
                display: table-caption;
                text-align: center;
            }
            colgroup {
                display: table-column-group;
            }
            col {
                display: table-column;
            }
            thead {
                display: table-header-group;
            }
            tbody {
                display: table-row-group;
            }
            tfoot {
                display: table-footer-group;
            }
            tr {
                display: table-row;
            }
            td, th {
                display: table-cell;
                padding: 4px 8px;
            }
            th {
                font-weight: bold;
                text-align: center;
            }
            form {
                display: block;
                margin: 0px;
            }
            fieldset {
                display: block;
                margin: 8px 2px;
                padding: 6px 10px;
                border: 1px solid #c0c0c0;
                border-radius: 4px;
            }
            legend {
                display: block;
                padding: 0px 4px;
                font-weight: bold;
            }
            label {
                display: inline-block;
                cursor: default;
            }
            input, button, select, textarea {
                display: inline-block;
                font-family: sans-serif;
                font-size: 13px;
                box-sizing: border-box;
            }
            input, select, textarea {
                background-color: #ffffff;
                border: 1px solid #767676;
                border-radius: 3px;
                padding: 3px 6px;
            }
            iframe {
                display: inline-block;
                width: 300px;
                height: 150px;
                border: 2px inset #767676;
                box-sizing: border-box;
            }
            video {
                display: inline-block;
                width: 300px;
                height: 150px;
                box-sizing: border-box;
            }
            canvas {
                display: inline-block;
                width: 300px;
                height: 150px;
                box-sizing: border-box;
            }
            audio {
                display: none;
                box-sizing: border-box;
            }
            audio[controls] {
                display: inline-block;
                width: 300px;
                height: 36px;
            }
            :focus-visible {
                outline: 2px solid #0078d7;
            }
            input[type="checkbox"], input[type="radio"] {
                border-top-width: 0px;
                border-right-width: 0px;
                border-bottom-width: 0px;
                border-left-width: 0px;
                border-top-style: none;
                border-right-style: none;
                border-bottom-style: none;
                border-left-style: none;
                background-color: transparent;
                padding: 0px;
            }
            input[type="hidden"] {
                display: none;
            }
            button, input[type="submit"], input[type="button"], input[type="reset"] {
                background-color: #f0f0f0;
                border: 1px solid #767676;
                border-radius: 4px;
                padding: 4px 12px;
                cursor: pointer;
                text-align: center;
                color: #000000;
            }
            img {
                display: inline-block;
            }
            svg {
                display: inline-block;
                vertical-align: middle;
            }
            svg:not(:root) {
                overflow: hidden;
            }
            hr {
                display: block;
                margin: 8px 0px;
                border-top-width: 1px;
                border-top-style: solid;
                border-top-color: gray;
            }
            dialog {
                display: none;
                position: absolute;
                left: 0px;
                right: 0px;
                top: 0px;
                bottom: 0px;
                margin: auto;
                border: 1px solid black;
                padding: 16px;
                background-color: white;
                color: black;
                box-sizing: border-box;
            }
            dialog[open] {
                display: block;
            }
            [popover] {
                display: none;
                position: fixed;
                left: 0px;
                right: 0px;
                top: 0px;
                bottom: 0px;
                margin: auto;
                border: 1px solid black;
                padding: 4px;
                background-color: white;
                color: black;
            }
            :popover-open, [popover]:popover-open {
                display: block;
            }
            dialog:not([open])[popover]:popover-open {
                display: block;
            }
            datalist {
                display: none;
            }
            optgroup {
                display: block;
                font-weight: bold;
                font-style: italic;
            }
            optgroup > option {
                font-weight: normal;
                font-style: normal;
                padding-left: 12px;
            }
            select[multiple], select[size] {
                vertical-align: top;
            }
            progress {
                display: inline-block;
                vertical-align: -0.2em;
                width: 160px;
                height: 16px;
                box-sizing: border-box;
            }
            meter {
                display: inline-block;
                vertical-align: -0.2em;
                width: 80px;
                height: 16px;
                box-sizing: border-box;
            }
            output {
                display: inline;
            }
            map {
                display: inline;
            }
            area {
                display: none;
            }
            object, embed {
                display: inline-block;
                width: 300px;
                height: 150px;
                box-sizing: border-box;
            }
            wbr {
                display: inline;
            }
            ruby {
                display: inline;
            }
            rt {
                font-size: 50%;
                line-height: 1;
                vertical-align: super;
            }
            rp {
                display: none;
            }
            bdi {
                unicode-bidi: isolate;
            }
            bdo {
                unicode-bidi: bidi-override;
            }
            [dir="rtl"] {
                direction: rtl;
            }
            [dir="ltr"] {
                direction: ltr;
            }
            bdo[dir="rtl"] {
                direction: rtl;
                unicode-bidi: bidi-override;
            }
            bdo[dir="ltr"] {
                direction: ltr;
                unicode-bidi: bidi-override;
            }
            #masthead-logo a:first-of-type,
            ytd-masthead #masthead-logo a:first-of-type {
                display: flex !important;
                align-items: center;
            }
        "#;
        parse_stylesheet(css)
    })
}
