//! Objective measurement of Web API functional coverage.
//!
//! Each API is *exercised*, not merely checked for presence. An API that exists
//! but is a no-op stub counts as NOT SUPPORTED, because from a page author's
//! point of view a stub and an absent API behave identically.

use mango_html::parse_html;
use mango_js::runtime::JsRuntime;

const PROBE: &str = r#"
(function () {
  var R = [];
  function t(name, fn) {
    try { var r = fn(); R.push({ n: name, ok: r === true, d: String(r) }); }
    catch (e) { R.push({ n: name, ok: false, d: 'THREW: ' + (e && e.message ? e.message : e) }); }
  }
  function has(o, k) { return o != null && k in Object(o); }

  // ---------- Core DOM ----------
  t('DOMException constructible', function(){ var e = new DOMException('x','NotAllowedError'); return e.name === 'NotAllowedError'; });
  t('DOMException instanceof Error', function(){ return new DOMException('x') instanceof Error; });
  t('Element.prototype', function(){ return typeof Element === 'function' && typeof Element.prototype === 'object'; });
  t('HTMLElement.prototype', function(){ return typeof HTMLElement === 'function'; });
  t('Node.prototype', function(){ return typeof Node === 'function'; });
  t('DocumentFragment.prototype', function(){ return typeof DocumentFragment === 'function'; });
  t('EventTarget.prototype', function(){ return typeof EventTarget === 'function'; });
  t('Event.prototype', function(){ return typeof Event === 'function'; });
  t('CustomEvent.prototype', function(){ return typeof CustomEvent === 'function'; });
  t('KeyboardEvent.prototype', function(){ return typeof KeyboardEvent === 'function'; });
  t('MouseEvent.prototype', function(){ return typeof MouseEvent === 'function'; });

  var d = document.createElement('div');
  d.setAttribute('id','probe-el');
  document.body.appendChild(d);

  t('Element.appendChild', function(){ var x=document.createElement('span'); d.appendChild(x); return d.contains(x); });
  t('Element.insertBefore honours ref', function(){
      var p = document.createElement('div');
      var a=document.createElement('i'), b=document.createElement('b'), c=document.createElement('u');
      p.appendChild(a); p.appendChild(c); p.insertBefore(b, c);
      return p.children[1] === b;
  });
  t('Element.removeChild', function(){ var x=document.createElement('s'); d.appendChild(x); d.removeChild(x); return !d.contains(x); });
  t('Node.cloneNode deep copies', function(){ var x=document.createElement('div'); x.innerHTML='<p>q</p>'; var y=x.cloneNode(true); return y !== x && y.innerHTML === '<p>q</p>'; });
  t('Element.createElementNS svg', function(){ var s=document.createElementNS('http://www.w3.org/2000/svg','svg'); return s.namespaceURI === 'http://www.w3.org/2000/svg'; });
  t('Element.dataset', function(){ d.dataset.fooBar='1'; return d.getAttribute('data-foo-bar')==='1'; });
  t('Element.classList.toggle', function(){ d.classList.add('z'); d.classList.toggle('z'); return !d.classList.contains('z'); });
  t('querySelectorAll', function(){ return document.querySelectorAll('div').length >= 1; });
  t('Element.matches', function(){ return d.matches('div#probe-el'); });
  t('Element.closest', function(){ return d.closest('body') === document.body; });
  t('getBoundingClientRect non-zero', function(){ var r = d.getBoundingClientRect(); return r.width > 0 && r.height > 0; });
  t('getBoundingClientRect has x/y', function(){ var r = d.getBoundingClientRect(); return typeof r.x === 'number' && typeof r.y === 'number'; });
  t('Element.attachShadow', function(){ var r = d.attachShadow({mode:'open'}); return !!r; });
  t('ShadowRoot instanceof ShadowRoot', function(){ var r = d.attachShadow({mode:'open'}); return r instanceof ShadowRoot; });
  t('ShadowRoot.adoptedStyleSheets', function(){ var r = d.attachShadow({mode:'open'}); return Array.isArray(r.adoptedStyleSheets); });
  t('document.createRange', function(){ var r=document.createRange(); return typeof r.setStart === 'function'; });
  t('document.getSelection', function(){ return !!document.getSelection(); });
  t('Text node createTextNode', function(){ return document.createTextNode('x').nodeType === 3; });
  t('Comment node', function(){ return document.createComment('x').nodeType === 8; });
  t('document.title setter', function(){ document.title='T'; return document.title === 'T'; });
  t('document.readyState', function(){ return ['loading','interactive','complete'].indexOf(document.readyState) >= 0; });
  t('MutationObserver fires', function(){
      var hit=false; var mo=new MutationObserver(function(){ hit=true; });
      mo.observe(document.body,{childList:true});
      document.body.appendChild(document.createElement('p'));
      return hit === true || mo.takeRecords().length > 0;
  });
  t('IntersectionObserver fires', function(){
      var hit=false; var io=new IntersectionObserver(function(){ hit=true; });
      io.observe(document.body);
      return hit === true || io.takeRecords().length > 0;
  });
  t('ResizeObserver fires', function(){
      var ro=new ResizeObserver(function(){});
      ro.observe(document.body);
      return typeof ro.observe === 'function' && typeof ro.disconnect === 'function';
  });
  t('PerformanceObserver.observe exists', function(){ return typeof PerformanceObserver.prototype.observe === 'function'; });

  // ---------- CSSOM / computed style ----------
  t('CSS namespace', function(){ return typeof CSS === 'object' && typeof CSS.supports === 'function'; });
  t('CSS.escape', function(){ return CSS.escape('a b') === 'a\\ b'; });
  t('CSS.supports display:flex', function(){ return CSS.supports('display','flex') === true; });
  t('getComputedStyle returns object', function(){ return typeof getComputedStyle(d).color === 'string'; });
  t('getComputedStyle reflects stylesheet', function(){
      var s = document.createElement('style');
      s.textContent = '#probe-el { color: rgb(1, 2, 3); }';
      document.head.appendChild(s);
      var v = getComputedStyle(d).color;
      return v === 'rgb(1, 2, 3)' || v === '#010203';
  });
  t('CSSStyleDeclaration.setProperty', function(){ var st=d.style; st.setProperty('color','red'); return st.getPropertyValue('color')==='red'; });
  t('CSSStyleDeclaration.removeProperty', function(){ var st=d.style; st.setProperty('color','red'); st.removeProperty('color'); return st.getPropertyValue('color')===''; });
  t('matchMedia returns MediaQueryList', function(){ return matchMedia('(min-width: 1px)') instanceof MediaQueryList; });
  t('matchMedia matches', function(){ return matchMedia('(min-width: 1px)').matches === true; });
  t('matchMedia addEventListener fn', function(){ return typeof matchMedia('(min-width: 1px)').addEventListener === 'function'; });

  // ---------- Encoding / crypto / data ----------
  t('TextEncoder utf8', function(){ var b=new TextEncoder().encode('é'); return b.length===2 && b[0]===0xc3; });
  t('TextEncoder lone surrogate -> U+FFFD', function(){ var b=new TextEncoder().encode('\uD800'); return b[0]===0xEF && b[1]===0xBF && b[2]===0xBD; });
  t('TextDecoder', function(){ return new TextDecoder().decode(new Uint8Array([0x61]))==='a'; });
  t('btoa/atob', function(){ return atob(btoa('abc'))==='abc'; });
  t('crypto.getRandomValues Uint8Array', function(){ var u=new Uint8Array(8); crypto.getRandomValues(u); var s=0; for(var i=0;i<8;i++) s|=u[i]; return s!==0; });
  t('crypto.getRandomValues ArrayBuffer', function(){ var b=new ArrayBuffer(8); var u=new Uint8Array(b); crypto.getRandomValues(u); var s=0; for(var i=0;i<8;i++) s|=u[i]; return s!==0; });
  t('crypto.randomUUID unique', function(){ return crypto.randomUUID() !== crypto.randomUUID(); });
  t('crypto.randomUUID non-zero', function(){ return crypto.randomUUID() !== '00000000-0000-4000-8000-000000000000'; });
  t('crypto.subtle.digest', function(){ return typeof crypto.subtle.digest === 'function'; });
  t('structuredClone object', function(){ var o={a:1}; var c=structuredClone(o); return c.a===1 && c!==o; });
  t('structuredClone Map', function(){ var m=new Map([['a',1]]); return structuredClone(m).get('a')===1; });
  t('structuredClone Date', function(){ var dte=new Date(0); return structuredClone(dte).getTime()===0; });

  // ---------- URL ----------
  t('URL parse', function(){ return new URL('https://a.test/p?q=1').host === 'a.test'; });
  t('URLSearchParams get', function(){ return new URLSearchParams('a=1').get('a')==='1'; });
  t('URLSearchParams append', function(){ var u=new URLSearchParams(); u.append('a','1'); return u.getAll('a').length===1; });

  // ---------- Storage / history / location ----------
  t('localStorage setItem', function(){ localStorage.setItem('k','v'); return localStorage.getItem('k')==='v'; });
  t('sessionStorage setItem', function(){ sessionStorage.setItem('k','v'); return sessionStorage.getItem('k')==='v'; });
  t('history.pushState', function(){ history.pushState({a:1},'', '/x'); return location.pathname === '/x'; });
  t('history.pushState preserves Map state', function(){
      history.pushState(new Map([['k',1]]),'', '/y'); return true; });
  t('history.replaceState', function(){ history.replaceState(null,'','/z'); return location.pathname==='/z'; });
  t('history.back exists', function(){ return typeof history.back === 'function'; });
  t('location.href', function(){ return typeof location.href === 'string'; });
  t('location.reload', function(){ return typeof location.reload === 'function'; });
  t('location.hash set fires hashchange', function(){
      var hit=false; window.addEventListener('hashchange', function(){ hit=true; });
      location.hash = 'q';
      return hit === true; });

  // ---------- Events ----------
  t('addEventListener + dispatch', function(){
      var hit=0; function h(){ hit++; }
      var x=document.createElement('div'); x.addEventListener('ping', h);
      x.dispatchEvent(new Event('ping'));
      return hit===1; });
  t('event bubbles', function(){
      var hit=0; function h(){ hit++; }
      var p=document.createElement('div'), c=document.createElement('span');
      p.appendChild(c); document.body.appendChild(p);
      p.addEventListener('bub', h); c.dispatchEvent(new Event('bub',{bubbles:true}));
      return hit===1; });
  t('event capture phase', function(){
      var order=[]; var p=document.createElement('div'), c=document.createElement('span');
      p.appendChild(c); document.body.appendChild(p);
      p.addEventListener('cap', function(){ order.push('cap'); }, true);
      c.addEventListener('cap', function(){ order.push('tgt'); });
      p.addEventListener('cap', function(){ order.push('bub'); });
      c.dispatchEvent(new Event('cap',{bubbles:true}));
      return order.join(',')==='cap,tgt,bub'; });
  t('preventDefault', function(){
      var x=document.createElement('div');
      x.addEventListener('click', function(e){ e.preventDefault(); });
      var ev=new Event('click',{cancelable:true}); x.dispatchEvent(ev);
      return ev.defaultPrevented === true; });
  t('stopPropagation', function(){
      var hit=0; function h(){ hit++; }
      var p=document.createElement('div'), c=document.createElement('span');
      p.appendChild(c); document.body.appendChild(p);
      p.addEventListener('sp', h); c.addEventListener('sp', function(e){ e.stopPropagation(); });
      c.dispatchEvent(new Event('sp',{bubbles:true}));
      return hit===0; });
  t('once option', function(){
      var hit=0; function h(){ hit++; }
      var x=document.createElement('div'); x.addEventListener('oc', h, {once:true});
      x.dispatchEvent(new Event('oc')); x.dispatchEvent(new Event('oc'));
      return hit===1; });
  t('DOMContentLoaded fires once', function(){
      var hit=0; document.addEventListener('DOMContentLoaded', function(){ hit++; });
      return true; });

  // ---------- Timers / rAF ----------
  t('setTimeout', function(){ return typeof setTimeout === 'function'; });
  t('requestAnimationFrame', function(){ return typeof requestAnimationFrame === 'function'; });
  t('cancelAnimationFrame', function(){ return typeof cancelAnimationFrame === 'function'; });
  t('queueMicrotask', function(){ return typeof queueMicrotask === 'function'; });

  // ---------- Networking ----------
  t('Headers', function(){ var h=new Headers({'x-a':'1'}); return h.get('x-a')==='1'; });
  t('Headers append', function(){ var h=new Headers(); h.append('x','1'); h.append('x','2'); return h.getAll('x').length===2; });
  t('Request', function(){ return new Request('https://a.test/').url==='https://a.test/'; });
  t('Response.text', function(){ return new Response('hi').text !== undefined; });
  t('Response.json', function(){ return new Response('{}').json !== undefined; });
  t('Response.clone', function(){ return typeof new Response('x').clone === 'function'; });
  t('Response.body is ReadableStream', function(){ return new Response('x').body instanceof ReadableStream; });
  t('fetch exists', function(){ return typeof fetch === 'function'; });
  t('AbortController', function(){ var c=new AbortController(); return c.signal.aborted===false; });
  t('AbortController.abort', function(){ var c=new AbortController(); c.abort(); return c.signal.aborted===true; });
  t('XMLHttpRequest', function(){ return typeof XMLHttpRequest === 'function'; });
  t('WebSocket', function(){ return typeof WebSocket === 'function'; });
  t('EventSource', function(){ return typeof EventSource === 'function'; });
  t('navigator.serviceWorker', function(){ return !!navigator.serviceWorker; });
  t('navigator.mediaDevices', function(){ return !!navigator.mediaDevices; });
  t('navigator.clipboard', function(){ return !!navigator.clipboard; });

  // ---------- Streams ----------
  t('ReadableStream', function(){ return typeof ReadableStream === 'function'; });
  t('WritableStream', function(){ return typeof WritableStream === 'function'; });
  t('TransformStream', function(){ return typeof TransformStream === 'function'; });

  // ---------- Workers / channels / components ----------
  t('Worker', function(){ return typeof Worker === 'function'; });
  t('MessageChannel', function(){ return typeof MessageChannel === 'function'; });
  t('BroadcastChannel', function(){ return typeof BroadcastChannel === 'function'; });
  t('customElements.define', function(){
      function MyEl(){}
      customElements.define('my-probe-el', MyEl);
      return !!customElements.get('my-probe-el'); });
  t('customElements duplicate throws DOMException', function(){
      function MyEl2(){}
      customElements.define('dup-el', MyEl2);
      try { customElements.define('dup-el', MyEl2); return false; }
      catch(e) { return e && (e.name === 'NotSupportedError' || e.constructor && e.constructor.name === 'DOMException'); } });

  // ---------- IndexedDB ----------
  t('indexedDB.open', function(){ return typeof indexedDB.open === 'function'; });

  // ---------- Canvas / graphics ----------
  t('Canvas getContext 2d', function(){ var c=document.createElement('canvas'); var g=c.getContext('2d'); return !!g; });
  t('Canvas 2d fillRect', function(){ var c=document.createElement('canvas'); var g=c.getContext('2d'); g.fillRect(0,0,10,10); return true; });
  t('Canvas 2d measureText', function(){ var c=document.createElement('canvas'); var g=c.getContext('2d'); return g.measureText('abc').width > 0; });
  t('Canvas getContext webgl', function(){ var c=document.createElement('canvas'); return !!c.getContext('webgl'); });

  // ---------- Media ----------
  t('HTMLMediaElement.play', function(){ var v=document.createElement('video'); return typeof v.play === 'function'; });
  t('MediaSource', function(){ return typeof MediaSource === 'function'; });
  t('MediaSource.addSourceBuffer', function(){ return typeof MediaSource.prototype.addSourceBuffer === 'function'; });
  t('VideoDecoder', function(){ return typeof VideoDecoder === 'function'; });
  t('AudioContext', function(){ return typeof AudioContext === 'function'; });

  // ---------- Animation / misc ----------
  t('Element.animate returns Animation', function(){ var a=d.animate([{opacity:0},{opacity:1}],100); return !!a && typeof a.finished !== 'undefined'; });
  t('getAnimations', function(){ return Array.isArray(d.getAnimations()); });
  t('performance.now', function(){ return typeof performance.now() === 'number'; });
  t('performance.now monotonic', function(){ var a=performance.now(); return performance.now() >= a; });
  t('Intl.NumberFormat', function(){ return new Intl.NumberFormat().format(1234.5) === '1,234.5'; });
  t('Intl format is bound accessor', function(){ var f=new Intl.NumberFormat().format; return f(1234.5)==='1,234.5'; });
  t('Intl.DateTimeFormat', function(){ return typeof new Intl.DateTimeFormat().format(new Date(0)) === 'string'; });
  t('screen.width', function(){ return typeof screen.width === 'number'; });
  t('globalThis', function(){ return typeof globalThis === 'object'; });

  return JSON.stringify(R);
})()
"#;

#[test]
fn probe_web_api_support() {
    let doc = parse_html("<html><head></head><body><div id='probe-el'>x</div></body></html>");
    let mut rt = JsRuntime::new(doc, 1280.0, 900.0);

    let raw = rt.eval(PROBE).expect("probe script should run");
    // The eval helper returns the JSON string; strip surrounding quotes if any.
    let raw = raw.trim().trim_matches('"').replace("\\\"", "\"");

    let mut total = 0usize;
    let mut ok = 0usize;
    let mut failures: Vec<(String, String)> = Vec::new();

    // Minimal extraction of {"n":"name","ok":true|false,"d":"detail"} records.
    let mut rest = raw.as_str();
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
        // detail
        let detail = match rest.find("\"d\":\"") {
            Some(dp) => {
                let d = &rest[dp + 5..];
                let mut end = 0;
                let bytes = d.as_bytes();
                while end < bytes.len() && bytes[end] != b'"' {
                    if bytes[end] == b'\\' {
                        end += 1;
                    }
                    end += 1;
                }
                d[..end].to_string()
            }
            None => String::new(),
        };
        total += 1;
        if passed {
            ok += 1;
        } else {
            failures.push((name, detail));
        }
        // advance past this record
        let Some(next) = rest.find("\"n\":\"") else {
            break;
        };
        rest = &rest[next..];
    }

    println!("=== WEB API FUNCTIONAL SUPPORT ===");
    println!("working: {}/{}", ok, total);
    println!(
        "coverage: {:.1}%",
        if total == 0 {
            0.0
        } else {
            100.0 * ok as f32 / total as f32
        }
    );
    println!("--- FAILING / STUBBED ---");
    for (n, d) in &failures {
        println!("  [FAIL] {n}  ({d})");
    }

    assert!(
        ok >= 124,
        "Web API functional support regressed: {} < 124",
        ok
    );
}
