//! Async-correct re-test of the observer APIs.
//!
//! The first probe checked observer delivery *synchronously*. Per spec,
//! MutationObserver / ResizeObserver / IntersectionObserver deliver
//! asynchronously (microtask or rendering-opportunity timing), so a
//! synchronous check is a false negative. This probe awaits properly.

use mango_html::parse_html;
use mango_js::runtime::JsRuntime;

const BODY: &str = r#"
  var R = [];
  function t(name, ok, detail) { R.push({ n: name, ok: !!ok, d: detail || '' }); }
  function tick() { return new Promise(function(r) { setTimeout(r, 0); }); }

  var moHit = 0, moRecords = null;
  var mo = new MutationObserver(function(recs, obs) { moHit++; moRecords = recs; });
  mo.observe(document.body, { childList: true, attributes: true, subtree: true });
  document.body.appendChild(document.createElement('p'));
  document.body.setAttribute('data-mu', '1');
  await Promise.resolve();
  await tick();
  t('MutationObserver delivers (async)', moHit >= 1, 'hits=' + moHit);
  t('MutationObserver childList record', moRecords && moRecords[0] && moRecords[0].type === 'childList',
    moRecords && moRecords[0] ? moRecords[0].type : 'none');
  t('MutationObserver attribute record', moRecords && moRecords.some(function(r){ return r.type === 'attributes'; }),
    moRecords ? moRecords.map(function(r){return r.type;}).join(',') : 'none');

  var qm = false;
  queueMicrotask(function () { qm = true; });
  await Promise.resolve();
  t('queueMicrotask delivers', qm === true);

  var roHit = 0;
  var ro = new ResizeObserver(function (entries) { roHit++; });
  ro.observe(document.body);
  await tick();
  t('ResizeObserver delivers', roHit >= 1, 'hits=' + roHit);

  var ioHit = 0, ioEntry = null;
  var io = new IntersectionObserver(function (entries) { ioHit++; ioEntry = entries[0]; });
  io.observe(document.body);
  await tick();
  t('IntersectionObserver delivers', ioHit >= 1, 'hits=' + ioHit);
  t('IntersectionObserver entry has isIntersecting', ioEntry && typeof ioEntry.isIntersecting === 'boolean');

  var rafHit = 0, rafTs = null;
  requestAnimationFrame(function (ts) { rafHit++; rafTs = ts; });
  await tick();
  t('requestAnimationFrame delivers', rafHit >= 1, 'hits=' + rafHit);
  t('requestAnimationFrame timestamp is number', typeof rafTs === 'number', 'ts=' + rafTs);

  var settled = 'pending';
  try {
    await Promise.race([
      fetch('data:text/plain,hello').then(function (r) { return r.text(); }),
      new Promise(function (_, rej) { setTimeout(function () { rej(new Error('timeout')); }, 8000); })
    ]).then(function (txt) { settled = 'resolved:' + txt; })
      .catch(function (e) { settled = 'rejected:' + (e && e.message); });
  } catch (e) { settled = 'threw:' + e.message; }
  t('fetch settles (not pending)', settled !== 'pending', settled);

  var pushOk = false, pushErr = '';
  try { history.pushState({ a: 1 }, '', '/probe-path'); pushOk = (location.pathname === '/probe-path'); }
  catch (e) { pushErr = e.message; }
  t('history.pushState updates location', pushOk, pushErr);

  var titleOk = false, titleErr = '';
  try { document.title = 'ProbeTitle'; titleOk = (document.title === 'ProbeTitle'); }
  catch (e) { titleErr = e.message; }
  t('document.title setter', titleOk, 'err=' + titleErr + ' got=' + document.title);

  t('performance.now is number', typeof performance.now === 'number', 'v=' + performance.now());

  var hOk = false;
  try { var h = new Headers(); h.append('x', '1'); hOk = typeof h.getAll === 'function'; } catch (e) {}
  t('Headers.getAll exists', hOk);

  var ibOk = false;
  try {
    var p = document.createElement('div');
    var a = document.createElement('i'), b = document.createElement('b'), c = document.createElement('u');
    p.appendChild(a); p.appendChild(c); p.insertBefore(b, c);
    ibOk = (p.children[1] === b);
  } catch (e) {}
  t('insertBefore honours reference', ibOk);

  var srOk = false;
  try { var host = document.createElement('div'); var sh = host.attachShadow({mode:'open'}); srOk = (sh instanceof ShadowRoot); } catch (e) {}
  t('ShadowRoot instanceof ShadowRoot', srOk);

  var mqlOk = false;
  try { mqlOk = (matchMedia('(min-width: 1px)') instanceof MediaQueryList); } catch (e) {}
  t('matchMedia returns MediaQueryList', mqlOk);
"#;

#[test]
fn probe_async_correctness() {
    let doc = parse_html("<html><head></head><body><div id='probe-el'>x</div></body></html>");
    let mut rt = JsRuntime::new(doc, 1280.0, 900.0);

    let script = format!(
        "globalThis.__R = 'pending'; globalThis.__p = (async function () {{\n{}\n globalThis.__R = JSON.stringify(R);\n}})(); 'kicked';",
        BODY
    );

    let kicked = rt.eval(&script).expect("probe should start");
    println!("kicked: {kicked}");

    // Drain timers/microtasks so the async probe can complete.
    for _ in 0..400 {
        std::thread::sleep(std::time::Duration::from_millis(25));
        rt.tick();
    }

    let v = rt
        .eval("globalThis.__R")
        .unwrap_or_else(|e| format!("ERR {e}"));

    println!("=== ASYNC-CORRECT API PROBE ===");
    println!("resolved: {v}");

    let mut total = 0usize;
    let mut ok = 0usize;
    let mut failures: Vec<String> = Vec::new();
    let mut rest = v.as_str();
    while let Some(pos) = rest.find("\"n\":\"") {
        rest = &rest[pos + 5..];
        let Some(n_end) = rest.find('"') else { break };
        let name = rest[..n_end].to_string();
        rest = &rest[n_end..];
        let Some(okp) = rest.find("\"ok\":") else {
            break;
        };
        rest = &rest[okp + 5..];
        let passed = rest.starts_with("true");
        total += 1;
        if passed {
            ok += 1;
        } else {
            failures.push(name);
        }
        let Some(next) = rest.find("\"n\":\"") else {
            break;
        };
        rest = &rest[next..];
    }
    println!("working: {}/{}", ok, total);
    for f in &failures {
        println!("  [FAIL] {f}");
    }
}
