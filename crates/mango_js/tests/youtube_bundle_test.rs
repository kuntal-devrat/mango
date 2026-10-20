use mango_html::parse_html;
use mango_js::JsRuntime;

#[test]
#[ignore]
fn test_youtube_kevlar_base_execution() {
    let html = "<html><head><title>YouTube</title></head><body><ytd-app></ytd-app></body></html>";
    let doc = parse_html(html);
    let mut rt = JsRuntime::new(doc, 1280.0, 900.0);

    let pre_instrument = r#"
        globalThis._ceCount = 0;
        const origDefine = customElements.define;
        customElements.define = function(name, ctor, opts) {
            globalThis._ceCount++;
            console.log(`[CE Define #${globalThis._ceCount}] ${name}`);
            return origDefine.apply(this, arguments);
        };
        globalThis._createdCount = 0;
        const origCE = document.createElement;
        document.createElement = function(tag) {
            globalThis._createdCount++;
            return origCE.apply(this, arguments);
        };
    "#;
    rt.eval(pre_instrument).unwrap();

    let script_path = "/home/pikasu/.gemini/antigravity/brain/fadd5a55-27b9-4582-a5fd-554f7d0c4800/scratch/youtube_kevlar_base.js";
    let script = match std::fs::read_to_string(script_path) {
        Ok(s) => s,
        Err(e) => {
            println!("Could not read script: {e}");
            return;
        }
    };
    println!("Loaded script of length {}", script.len());
    let t0 = std::time::Instant::now();
    let res = rt.execute_script_with_timeout(&script, std::time::Duration::from_secs(60));
    println!(
        "Execution completed in {:?}, result: {:?}",
        t0.elapsed(),
        res
    );

    let counts = rt
        .eval("`CE Count: ${_ceCount}, Created Count: ${_createdCount}`")
        .unwrap();
    println!("Counts: {}", counts);

    let msgs = rt.take_console_messages();
    println!("Console messages captured: {}", msgs.len());
    for m in msgs.iter().take(30) {
        println!("  {:?}: {}", m.level, m.text);
    }
}

#[test]
#[ignore]
fn test_youtube_full_hydration() {
    let html_path = "/home/pikasu/.gemini/antigravity/brain/fadd5a55-27b9-4582-a5fd-554f7d0c4800/scratch/youtube.html";
    let html = std::fs::read_to_string(html_path).expect("read youtube.html");
    let doc = parse_html(&html);

    fn collect_scripts_from_doc(
        doc: &mango_html::Document,
        nid: mango_html::NodeId,
        list: &mut Vec<(String, String)>,
    ) {
        if let Some(node) = doc.get(nid)
            && let mango_html::NodeData::Element(ref el) = node.data
            && el.tag_name.eq_ignore_ascii_case("script")
        {
            let src = el.get_attribute("src").unwrap_or("").to_string();
            let text = doc.text_content(nid);
            list.push((src, text));
        }
        for child in doc.children(nid) {
            collect_scripts_from_doc(doc, child.id, list);
        }
    }

    let mut script_list = Vec::new();
    collect_scripts_from_doc(&doc, doc.root(), &mut script_list);
    println!("Found {} <script> tags in document", script_list.len());

    let mut rt = JsRuntime::new_with_url(doc, 1280.0, 900.0, "https://www.youtube.com/");

    let _kevlar_code = std::fs::read_to_string("/home/pikasu/.gemini/antigravity/brain/fadd5a55-27b9-4582-a5fd-554f7d0c4800/scratch/youtube_kevlar_base.js")
        .expect("read kevlar_base.js");

    println!("Running scripts from youtube.html in sequence...");
    for (idx, (src, body)) in script_list.into_iter().enumerate() {
        let (name, code) = if !src.is_empty() {
            let filename = src
                .split('?')
                .next()
                .unwrap()
                .split('/')
                .next_back()
                .unwrap();
            let path = if src.contains("kevlar_base_module") {
                "/home/pikasu/.gemini/antigravity/brain/fadd5a55-27b9-4582-a5fd-554f7d0c4800/scratch/youtube_kevlar_base.js".to_string()
            } else {
                format!(
                    "/home/pikasu/.gemini/antigravity/brain/fadd5a55-27b9-4582-a5fd-554f7d0c4800/scratch/{}",
                    filename
                )
            };
            if let Ok(c) = std::fs::read_to_string(&path) {
                (filename.to_string(), c)
            } else {
                println!("  [Script #{idx}] Skipping external: {src} (not found at {path})");
                continue;
            }
        } else {
            (format!("inline-#{idx}"), body)
        };

        let t0 = std::time::Instant::now();
        let code_len = code.len();
        let res = rt.execute_script_with_name_and_timeout(
            &name,
            &code,
            std::time::Duration::from_secs(60),
        );
        println!(
            "  [Script #{idx}] {} ({} bytes) in {:?}: {:?}",
            name,
            code_len,
            t0.elapsed(),
            res
        );
    }

    println!("\nDispatching DOMContentLoaded and load...");
    rt.dispatch_dom_content_loaded().unwrap();
    rt.dispatch_load().unwrap();
    rt.run_jobs();
    for _ in 0..25 {
        std::thread::sleep(std::time::Duration::from_millis(25));
        rt.tick();
        rt.run_jobs();
    }

    let check_script = r##"
        (function() {
            const app = document.querySelector("ytd-app");
            const initialProps = {
                hasInitProps: app ? typeof app._initializeProperties : null,
                hasCreated: app ? typeof app.created : null,
                hasActionRouterNode: app ? !!app.actionRouterNode : null,
                isCustomConstructed: app ? !!app._isCustomConstructed : null,
                isCustomConnected: app ? !!app._isCustomConnected : null,
                isAttached: app ? !!app.isAttached : null,
                hasDisableUpgrade: app ? app.hasAttribute("disable-upgrade") : null,
                isUpgradeDisabled: app ? !!app.__isUpgradeDisabled : null,
            };
            let connErr = null;
            let attachedErr = null;
            if (app) {
                try {
                    if (typeof app.connectedCallback === 'function') app.connectedCallback();
                } catch(e) {
                    connErr = e.toString() + "\n" + (e.stack || "");
                }
                try {
                    if (typeof app.attached === 'function') app.attached();
                } catch(e) {
                    attachedErr = e.toString() + "\n" + (e.stack || "");
                }
            }
            const tmpl = app && app._template;
            const content = tmpl && tmpl.content;
            const pm = app && app.$ && app.$['page-manager'];
            const templateInfo = {
                appHasTemplateProp: app ? ('_template' in app) : null,
                appTemplateVal: app ? !!app._template : null,
                protoTemplate: app ? !!Object.getPrototypeOf(app)._template : null,
                ctorFinalized: app && app.constructor ? !!app.constructor.__finalized : null,
                ctorHasTemplate: app && app.constructor ? !!app.constructor.template : null,
                hasRoot: app ? !!app.root : null,
                rootType: app && app.root ? app.root.nodeType : null,
                rootCtor: app && app.root && app.root.constructor ? app.root.constructor.name : null,
                rootKids: (app && app.root && app.root.childNodes) ? app.root.childNodes.length : null,
                hasShadow: app ? !!app.shadowRoot : null,
                shadowKids: (app && app.shadowRoot && app.shadowRoot.childNodes) ? app.shadowRoot.childNodes.length : null,
                hasDollar: app ? !!app.$ : null,
                dollarKeys: (app && app.$) ? Object.keys(app.$) : [],
                pmPresent: !!pm,
                pmTag: pm ? pm.tagName : null,
                pmCtor: pm && pm.constructor ? pm.constructor.name : null,
                pmConnected: pm ? pm.isConnected : null,
                pmAttached: pm ? !!pm.isAttached : null,
                pmHasConnCb: pm ? typeof pm.connectedCallback : null,
                pmHasAttached: pm ? typeof pm.attached : null,
                pmParent: pm ? (pm.parentNode ? pm.parentNode.nodeName : null) : null,
                pmHasPolyCtrl: pm ? ('polymerController' in pm) : false,
                pmPolyCtrlVal: pm ? !!pm.polymerController : false,
                pmPolyCtrlCtor: pm && pm.polymerController && pm.polymerController.constructor ? pm.polymerController.constructor.name : null,
                pmPolyCtrlAttached: pm && pm.polymerController ? typeof pm.polymerController.attached : null,
                pmPolyCtrlIsAttached: pm && pm.polymerController ? !!pm.polymerController.isAttached : null,
                pmCurrentPage: pm && pm.currentPage ? (pm.currentPage.tagName || pm.currentPage.is) : null,
                pmCtrlCurrentPage: pm && pm.polymerController && pm.polymerController.currentPage ? (pm.polymerController.currentPage.tagName || pm.polymerController.currentPage.is) : null,
                pmChildCount: pm ? pm.childNodes.length : null,
                pmChildTags: pm ? Array.from(pm.childNodes).map(c => c.tagName || c.nodeName) : [],
                pmCtrlData: pm && pm.polymerController ? (pm.polymerController.data ? typeof pm.polymerController.data : 'no data') : null,
                pmData: pm && pm.data ? typeof pm.data : null,
                appData: app && app.data ? typeof app.data : null,
                appPageManagerAttachedPromiseState: app && app.pageManagerAttachedPromise ? {
                    hasPromise: !!app.pageManagerAttachedPromise.promise,
                    hasResolve: typeof app.pageManagerAttachedPromise.resolve === 'function'
                } : null,
                pmUsePatchedLifecycles: pm ? pm.usePatchedLifecycles : null,
                pmLifecycleIsConnected: pm ? pm.lifecycleIsConnected : null,
                pmProps: pm ? Object.getOwnPropertyNames(pm).slice(0, 20) : [],
                pmProtoProps: pm ? Object.getOwnPropertyNames(Object.getPrototypeOf(pm)).slice(0, 20) : [],
                tmplTag: tmpl ? tmpl.tagName : null,
                tmplHasContent: tmpl ? ('content' in tmpl) : null,
                contentNodeType: content ? content.nodeType : null,
                contentKids: (content && content.childNodes) ? content.childNodes.length : null,
                tmplInfoPresent: tmpl ? !!tmpl._templateInfo : null,
                nodeInfoListLen: (tmpl && tmpl._templateInfo && tmpl._templateInfo.nodeInfoList) ? tmpl._templateInfo.nodeInfoList.length : null,
            };
            return JSON.stringify({
                initialProps: initialProps,
                templateInfo: templateInfo,
                hasSkeleton: !!document.querySelector("#home-page-skeleton"),
                hasApp: !!app,
                appTag: app ? app.tagName : null,
                appConnected: app ? app.isConnected : false,
                appParent: app ? (app.parentNode ? app.parentNode.nodeName : null) : null,
                appCtor: app ? (app.constructor ? app.constructor.name : null) : null,
                appIsCustomConnected: app ? !!app._isCustomConnected : false,
                appIsAttached: app ? !!app.isAttached : false,
                connErr: connErr,
                attachedErr: attachedErr,
                createdErr: (function() {
                    try {
                        if (typeof app.created === 'function') {
                            app.created();
                            return "created() success: actionRouterNode=" + !!app.actionRouterNode;
                        }
                        return "no app.created";
                    } catch(e) {
                        return e.toString() + "\n" + (e.stack || "");
                    }
                })(),
                appProps: app ? Object.getOwnPropertyNames(app).slice(0, 30) : [],
                appProtoProps: app ? Object.getOwnPropertyNames(Object.getPrototypeOf(app)).slice(0, 30) : [],
                hasDisableUpgrade: app ? app.hasAttribute("disable-upgrade") : false,
                isUpgradeDisabled: app ? !!app.__isUpgradeDisabled : false,
                hasData: app ? !!app.__data : false,
                hasActionMap: app ? !!app.actionMap : false,
                hasActionRouterNode: app ? !!app.actionRouterNode : false,
                constructErr: (function() {
                    try {
                        const C = customElements.get("ytd-app");
                        if (!C) return "no C";
                        const prev = globalThis._customElementsUpgradeCandidate;
                        globalThis._customElementsUpgradeCandidate = app;
                        try {
                            const res = Reflect.construct(C, [], C);
                            return "success: " + (res ? res.tagName : "null") + ", eq=" + (res === app) + ", resActionMap=" + (res ? !!res.actionMap : false) + ", resRouterNode=" + (res ? !!res.actionRouterNode : false);
                        } finally {
                            globalThis._customElementsUpgradeCandidate = prev;
                        }
                    } catch(e) {
                        return e.toString() + "\n" + (e.stack || "");
                    }
                })(),
                ytdAppDefined: !!(customElements.get("ytd-app")),
                loadInitialDataDefined: typeof window.loadInitialData,
                getInitialDataDefined: typeof window.getInitialData,
                ytInitialDataPresent: !!window.ytInitialData,
                ytdBrowsePresent: !!document.querySelector("ytd-browse"),
                ytdBrowseChildCount: document.querySelector("ytd-browse") ? document.querySelector("ytd-browse").childNodes.length : null,
                richGridCount: document.querySelectorAll("ytd-rich-grid-renderer").length,
                videoRendererCount: document.querySelectorAll("ytd-rich-item-renderer, ytd-video-renderer").length,
                pmManualAttachedResult: (function() {
                    if (pm && pm.polymerController && typeof pm.polymerController.attached === 'function') {
                        try {
                            const wasAttached = pm.polymerController.isAttached;
                            pm.polymerController.attached();
                            return "called attached(), wasAttached=" + wasAttached + ", isAttached=" + pm.polymerController.isAttached;
                        } catch(e) {
                            return "error: " + e.toString() + "\n" + (e.stack || "");
                        }
                    }
                    return "no attached fn";
                })(),
                appChildCount: app ? app.childNodes.length : null,
                appChildTags: app ? Array.from(app.childNodes).map(c => c.tagName || c.nodeName) : []
            });
        })()
    "##;
    let eval_res = rt.eval(check_script);
    println!("\nHydration state: {:?}", eval_res);

    for _ in 0..25 {
        std::thread::sleep(std::time::Duration::from_millis(25));
        rt.tick();
        rt.run_jobs();
    }

    let post_eval = rt.eval(r#"
        JSON.stringify({
            appHasLoadData: typeof (document.querySelector("ytd-app") && document.querySelector("ytd-app").loadData),
            ctrlHasLoadData: typeof (document.querySelector("ytd-app") && document.querySelector("ytd-app").polymerController && document.querySelector("ytd-app").polymerController.loadData),
            appAttachedState: (function() {
                const app = document.querySelector("ytd-app");
                return {
                    isAttached: app ? !!app.isAttached : false,
                    hasResolve: app && app.pageManagerAttachedPromise ? typeof app.pageManagerAttachedPromise.resolve : null
                };
            })(),
            resolvePmPromiseResult: (function() {
                const app = document.querySelector("ytd-app");
                if (app && app.pageManagerAttachedPromise && typeof app.pageManagerAttachedPromise.resolve === 'function') {
                    try {
                        app.pageManagerAttachedPromise.resolve();
                        return "resolved pageManagerAttachedPromise";
                    } catch(e) {
                        return "resolve error: " + e;
                    }
                }
                return "no pageManagerAttachedPromise";
            })(),
            manualLoadDataCall: (function() {
                const app = document.querySelector("ytd-app");
                const fn = app && (app.loadData || (app.polymerController && app.polymerController.loadData));
                if (typeof fn === 'function') {
                    try {
                        const target = app.loadData ? app : app.polymerController;
                        fn.call(target, { response: window.ytInitialData, page: "browse" });
                        return "called loadData successfully";
                    } catch(e) {
                        return "loadData error: " + e.toString() + "\n" + (e.stack || "");
                    }
                }
                return "no loadData fn found";
            })(),
            directPmUpdate: (function() {
                try {
                    const _ = globalThis._ytKevlar;
                    const app = document.querySelector("ytd-app");
                    const pm = app && app.$ && app.$['page-manager'];
                    if (!pm || !pm.polymerController) return "no pm or polymerController";
                    const endpoint = {
                        clickTrackingParams: "IhMI447tqd3DlwMVui23AB0zEDL3MghleHRlcm5hbMoBBPnxnAk=",
                        commandMetadata: { webCommandMetadata: { url: "/", webPageType: "WEB_PAGE_TYPE_BROWSE", rootVe: 3854, apiUrl: "/youtubei/v1/browse" } },
                        browseEndpoint: { browseId: "FEwhat_to_watch" }
                    };
                    const pageData = {
                        page: "browse",
                        endpoint: endpoint,
                        response: window.ytInitialData,
                        url: "/"
                    };
                    pm.polymerController.updatePageData(pageData);
                    return "updatePageData called successfully";
                } catch(e) {
                    return "directPmUpdate error: " + e.toString() + "\n" + (e.stack || "");
                }
            })(),
            browseCtorPresent: !!customElements.get("ytd-browse"),
            directBrowseCreate: (function() {
                try {
                    const el = document.createElement("ytd-browse");
                    return "created ytd-browse: " + (el ? el.tagName : "null") + ", is=" + (el ? el.is : "none") + ", hasCtrl=" + (el ? !!el.polymerController : "false");
                } catch(e) {
                    return "directBrowseCreate error: " + e.toString() + "\n" + (e.stack || "");
                }
            })(),
            ytdBrowsePresent: !!document.querySelector("ytd-browse"),
            ytdBrowseKids: document.querySelector("ytd-browse") ? document.querySelector("ytd-browse").childNodes.length : null,
            richGridCount: document.querySelectorAll("ytd-rich-grid-renderer").length,
            videoRendererCount: document.querySelectorAll("ytd-rich-item-renderer, ytd-video-renderer").length
        })
    "#);
    println!("\nPost-attached hydration state: {:?}", post_eval);

    for _ in 0..25 {
        std::thread::sleep(std::time::Duration::from_millis(25));
        rt.tick();
        rt.run_jobs();
    }

    let final_eval = rt.eval(r##"
        JSON.stringify({
            ytdBrowsePresent: !!document.querySelector("ytd-browse"),
            ytdBrowseKids: document.querySelector("ytd-browse") ? document.querySelector("ytd-browse").childNodes.length : null,
            browseKids: (function() {
                const b = document.querySelector("ytd-browse");
                return b ? Array.from(b.childNodes).map(c => (c.tagName || c.nodeName) + (c.id ? '#' + c.id : '')) : [];
            })(),
            browseTwoCol: (function() {
                const b = document.querySelector("ytd-browse");
                const bCtrl = b && b.polymerController;
                const tc = b && b.querySelector("ytd-two-column-browse-results-renderer");
                const tcCtrl = tc && tc.polymerController;
                const prim = tc && tc.querySelector("#primary");
                return {
                    bHasData: b ? !!b.data : false,
                    bCtrlHasData: bCtrl ? !!bCtrl.data : false,
                    bDataKeys: b && b.data ? Object.keys(b.data) : null,
                    bHasContents: b && b.data && b.data.contents ? Object.keys(b.data.contents) : null,
                    tcPresent: !!tc,
                    tcHasCtrl: !!tcCtrl,
                    tcIsAttached: tcCtrl ? tcCtrl.isAttached : null,
                    tcHasData: tc ? !!tc.data : false,
                    tcCtrlHasData: tcCtrl ? !!tcCtrl.data : false,
                    tcHasContent: tc ? !!tc.content : false,
                    tcCtrlHasContent: tcCtrl ? !!tcCtrl.content : false,
                    tcPropEffects: b ? (b.__dataProto && b.__dataProto.__propertyEffects ? Object.keys(b.__dataProto.__propertyEffects) : null) : null,
                    primPresent: !!prim,
                    primKids: prim ? Array.from(prim.childNodes).map(c => (c.tagName || c.nodeName) + (c.id ? '#' + c.id : '')) : [],
                    richGridCtor: !!customElements.get("ytd-rich-grid-renderer"),
                    manualSetTcData: (function() {
                        if (tc && !tc.data && b && b.data && b.data.contents && b.data.contents.twoColumnBrowseResultsRenderer) {
                            try {
                                const target = tcCtrl || tc;
                                target.data = b.data.contents.twoColumnBrowseResultsRenderer;
                                if (tcCtrl && tc !== tcCtrl) tc.data = target.data;
                                return "manually set tc.data: tcHasData=" + !!tc.data + ", tcCtrlHasData=" + (tcCtrl ? !!tcCtrl.data : false) + ", tcCtrlHasContent=" + (tcCtrl ? !!tcCtrl.content : false);
                            } catch(e) {
                                return "manualSetTcData error: " + e.toString() + "\n" + (e.stack || "");
                            }
                        }
                        return "no manual set needed or impossible";
                    })()
                };
            })(),
            richGridCount: document.querySelectorAll("ytd-rich-grid-renderer").length,
            videoRendererCount: document.querySelectorAll("ytd-rich-item-renderer, ytd-video-renderer").length,
            pmCurrentPage: (function() {
                const app = document.querySelector("ytd-app");
                const pm = app && app.$ && app.$['page-manager'];
                return pm && pm.polymerController && pm.polymerController.currentPage ? (pm.polymerController.currentPage.tagName || pm.polymerController.currentPage.is) : null;
            })(),
            pmKids: (function() {
                const app = document.querySelector("ytd-app");
                const pm = app && app.$ && app.$['page-manager'];
                return pm ? Array.from(pm.childNodes).map(c => c.tagName || c.nodeName) : [];
            })()
        })
    "##);
    println!("\nFinal hydration state: {:?}", final_eval);

    let msgs = rt.take_console_messages();
    println!("\nConsole messages ({} total):", msgs.len());
    for m in msgs.iter() {
        println!("  [{:?}] {}", m.level, m.text);
    }
}
