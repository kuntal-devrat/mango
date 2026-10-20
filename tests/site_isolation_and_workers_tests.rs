//! Integration tests for Phase 5: Multi-Process Architecture, Site Isolation,
//! Zero-Copy IPC, Shared Workers, BroadcastChannel, and Web Locks API.

use std::sync::mpsc;
use std::time::Duration;

use mango_engine::site_isolation::{ProcessPool, SandboxPolicy, SiteInstance};
use mango_ipc::{FLAG_SYNC, IpcEndpoint, IpcFrame, IpcMessage, ShmRingBuffer};
use mango_js::JsRuntime;
use mango_js::broadcast_channel::BroadcastChannelHub;
use mango_js::web_locks::{LockMode, LockResult, WebLocksEngine};
use mango_net::Url;

// ══════════════════════════════════════════════════════════════════════════════
// 1. Zero-Copy IPC Layer & Binary Framing
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_ipc_binary_framing_roundtrip() {
    let msg = IpcMessage::Navigate {
        target_url: "https://example.com/isolated".to_string(),
        history_id: 42,
    };
    let payload = serde_json::to_vec(&msg).unwrap();

    let frame = IpcFrame::new(101, FLAG_SYNC, 42, payload.clone());
    let encoded = frame.serialize();

    assert_eq!(&encoded[0..4], b"MGOI");
    assert_eq!(encoded.len(), 16 + payload.len());

    let (decoded_frame, consumed) = IpcFrame::parse(&encoded).unwrap().unwrap();
    assert_eq!(consumed, encoded.len());
    assert_eq!(decoded_frame.header.msg_type, 101);
    assert_eq!(decoded_frame.header.flags, FLAG_SYNC);
    assert_eq!(decoded_frame.header.correlation_id, 42);
    assert_eq!(decoded_frame.header.payload_length, payload.len() as u32);
    assert_eq!(decoded_frame.payload, payload);

    let decoded_msg: IpcMessage = serde_json::from_slice(&decoded_frame.payload).unwrap();
    match decoded_msg {
        IpcMessage::Navigate {
            target_url,
            history_id,
        } => {
            assert_eq!(target_url, "https://example.com/isolated");
            assert_eq!(history_id, 42);
        }
        _ => panic!("Unexpected IPC message variant"),
    }
}

#[test]
fn test_ipc_shm_ring_buffer_zero_copy() {
    let ring = ShmRingBuffer::new(4096);
    assert_eq!(ring.capacity(), 4096);
    assert_eq!(ring.available_read(), 0);

    // Write a simulated DisplayList / Framebuffer chunk (2048 bytes)
    let chunk1: Vec<u8> = (0..2048).map(|i| (i % 251) as u8).collect();
    let written1 = ring.write(&chunk1).unwrap();
    assert_eq!(written1, 2048);
    assert_eq!(ring.available_read(), 2048);

    // Read back first 1024 bytes
    let read_buf = ring.read(1024);
    assert_eq!(read_buf.len(), 1024);
    assert_eq!(&read_buf[..], &chunk1[..1024]);
    assert_eq!(ring.available_read(), 1024);

    // Write another 2048 bytes (tests circular ring wraparound)
    let chunk2: Vec<u8> = (0..2048).map(|i| (i % 199) as u8).collect();
    let written2 = ring.write(&chunk2).unwrap();
    assert_eq!(written2, 2048);

    // Drain remaining 3072 bytes
    let read_remaining = ring.read(3072);
    assert_eq!(read_remaining.len(), 3072);

    assert_eq!(&read_remaining[..1024], &chunk1[1024..]);
    assert_eq!(&read_remaining[1024..], &chunk2[..]);
    assert_eq!(ring.available_read(), 0);
}

#[test]
fn test_ipc_duplex_channel_endpoints() {
    let (browser_ep, renderer_ep) = IpcEndpoint::create_pair();

    // Browser sends navigation to renderer
    let nav_msg = IpcMessage::Navigate {
        target_url: "https://finance.site.com".to_string(),
        history_id: 1,
    };
    let corr_id = browser_ep.send(&nav_msg).unwrap();

    // Renderer polls incoming message
    let received = renderer_ep
        .try_recv()
        .unwrap()
        .expect("message should be pending");
    match received {
        IpcMessage::Navigate {
            target_url,
            history_id,
        } => {
            assert_eq!(target_url, "https://finance.site.com");
            assert_eq!(history_id, 1);
        }
        _ => panic!("Expected Navigate message"),
    }

    // Renderer sends correlation response
    let resp = IpcMessage::DidCommitProvisionalLoad {
        url: "https://finance.site.com".to_string(),
        title: "Finance".to_string(),
        http_status: 200,
    };
    renderer_ep.send_response(corr_id, &resp).unwrap();

    let resp_received = browser_ep.recv_timeout(Duration::from_millis(100)).unwrap();
    match resp_received {
        IpcMessage::DidCommitProvisionalLoad {
            url,
            title,
            http_status,
        } => {
            assert_eq!(url, "https://finance.site.com");
            assert_eq!(title, "Finance");
            assert_eq!(http_status, 200);
        }
        _ => panic!("Expected DidCommitProvisionalLoad message"),
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// 2. Site Isolation, Sandbox Policy & OOPIF Frame Trees
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_site_isolation_schemeful_site_computation() {
    let u1 = Url::parse("https://news.google.com/path").unwrap();
    let u2 = Url::parse("https://mail.google.com/inbox").unwrap();
    let u3 = Url::parse("http://google.com/").unwrap();
    let u4 = Url::parse("https://attacker.org/").unwrap();

    let s1 = SiteInstance::compute_site(&u1);
    let s2 = SiteInstance::compute_site(&u2);
    let s3 = SiteInstance::compute_site(&u3);
    let s4 = SiteInstance::compute_site(&u4);

    // Subdomains share same schemeful eTLD+1
    assert_eq!(s1, "https://google.com");
    assert_eq!(s2, "https://google.com");
    assert_eq!(s1, s2);

    // Different schemes are NOT same site (schemeful)
    assert_eq!(s3, "http://google.com");
    assert_ne!(s1, s3);

    // Cross-domain is distinct site
    assert_eq!(s4, "https://attacker.org");
    assert_ne!(s1, s4);

    // Special schemes
    let about_blank = Url::parse("about:blank").unwrap();
    assert_eq!(SiteInstance::compute_site(&about_blank), "about:blank");
}

#[test]
fn test_site_isolation_sandbox_policy_enforcement() {
    let renderer_policy = SandboxPolicy::for_renderer("https://example.com");
    assert!(!renderer_policy.is_network_allowed());
    assert!(!renderer_policy.is_filesystem_write_allowed());
    assert_eq!(renderer_policy.isolated_site, "https://example.com");

    let browser_policy = SandboxPolicy::for_browser();
    assert!(browser_policy.is_network_allowed());
    assert!(browser_policy.is_filesystem_write_allowed());

    let net_policy = SandboxPolicy::for_network();
    assert!(net_policy.is_network_allowed());
    assert!(!net_policy.is_filesystem_write_allowed());

    let gpu_policy = SandboxPolicy::for_gpu();
    assert!(!gpu_policy.is_network_allowed());
    assert!(!gpu_policy.is_filesystem_write_allowed());
}

#[test]
fn test_site_isolation_process_pool_allocation() {
    let mut pool = ProcessPool::new();

    let u1 = Url::parse("https://bank.com/account").unwrap();
    let mut site1 = SiteInstance::for_url(&u1);

    let (pid1, _ep1) = pool.get_or_create_process(&mut site1);
    assert_eq!(site1.process_id, Some(pid1));

    // Navigating within same site reuses same renderer process
    let u1_sub = Url::parse("https://auth.bank.com/login").unwrap();
    let mut site1_sub = SiteInstance::for_url(&u1_sub);
    let (pid1_sub, _ep1_sub) = pool.get_or_create_process(&mut site1_sub);
    assert_eq!(pid1, pid1_sub);

    // Navigating cross-site allocates separate isolated renderer process
    let u2 = Url::parse("https://evil.org/malware").unwrap();
    let mut site2 = SiteInstance::for_url(&u2);
    let (pid2, _ep2) = pool.get_or_create_process(&mut site2);
    assert_ne!(pid1, pid2);
    assert_eq!(pool.process_count(), 2);
}

#[test]
fn test_site_isolation_oopif_frame_tree() {
    let mut pool = ProcessPool::new();

    // 1. Root Frame: https://shopping.com
    let root_url = Url::parse("https://shopping.com").unwrap();
    let root_node = pool.create_frame_node(1, &root_url, None);
    assert!(!root_node.is_oopif);
    assert_eq!(root_node.parent_frame_id, None);
    let root_pid = root_node.site_instance.process_id.unwrap();

    // 2. Same-Site Child Frame: https://checkout.shopping.com
    let same_site_url = Url::parse("https://checkout.shopping.com").unwrap();
    let same_child = pool.create_frame_node(2, &same_site_url, Some(&root_node));
    assert!(!same_child.is_oopif);
    assert_eq!(same_child.site_instance.process_id.unwrap(), root_pid);

    // 3. Cross-Site Child Frame (OOPIF): https://payment-gateway.org
    let cross_site_url = Url::parse("https://payment-gateway.org").unwrap();
    let oopif_child = pool.create_frame_node(3, &cross_site_url, Some(&root_node));
    assert!(oopif_child.is_oopif);
    let oopif_pid = oopif_child.site_instance.process_id.unwrap();
    assert_ne!(oopif_pid, root_pid);
}

// ══════════════════════════════════════════════════════════════════════════════
// 3. BroadcastChannel Hub & Origin Isolation
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_broadcast_channel_pubsub_and_sender_omission() {
    let hub = BroadcastChannelHub::new();
    let origin = "https://app.example.com";
    let channel_name = "sync-channel";

    let (tx1, rx1) = mpsc::channel();
    let (tx2, rx2) = mpsc::channel();
    let (tx3, rx3) = mpsc::channel();

    let id1 = hub.next_id();
    let id2 = hub.next_id();
    let id3 = hub.next_id();

    hub.subscribe(origin, channel_name, id1, None, Some(tx1));
    hub.subscribe(origin, channel_name, id2, None, Some(tx2));
    hub.subscribe(origin, channel_name, id3, None, Some(tx3));

    assert_eq!(hub.subscriber_count(origin, channel_name), 3);

    // Endpoint 1 posts message
    let delivered = hub.broadcast(origin, channel_name, id1, r#"{"action":"refresh"}"#);
    assert_eq!(delivered, 2);

    // Sender 1 must NOT receive its own message
    assert!(rx1.try_recv().is_err());

    // Endpoints 2 and 3 MUST receive the broadcast
    let msg2 = rx2.try_recv().unwrap();
    assert_eq!(msg2, r#"{"action":"refresh"}"#);
    let msg3 = rx3.try_recv().unwrap();
    assert_eq!(msg3, r#"{"action":"refresh"}"#);

    // Close endpoint 3
    hub.unsubscribe(origin, channel_name, id3);
    assert_eq!(hub.subscriber_count(origin, channel_name), 2);

    // Endpoint 2 broadcasts
    let delivered2 = hub.broadcast(origin, channel_name, id2, "second message");
    assert_eq!(delivered2, 1);
    assert!(rx3.try_recv().is_err());
    assert_eq!(rx1.try_recv().unwrap(), "second message");
}

#[test]
fn test_broadcast_channel_cross_origin_security_isolation() {
    let hub = BroadcastChannelHub::new();

    let (tx_a, _rx_a) = mpsc::channel();
    let (tx_b, rx_b) = mpsc::channel();

    let id_a = hub.next_id();
    let id_b = hub.next_id();

    // Same channel name, different origins
    hub.subscribe("https://trusted.com", "alerts", id_a, None, Some(tx_a));
    hub.subscribe("https://untrusted.com", "alerts", id_b, None, Some(tx_b));

    hub.broadcast("https://trusted.com", "alerts", id_a, "confidential-token");

    // Untrusted origin MUST NOT receive messages from trusted origin
    assert!(rx_b.try_recv().is_err());
}

// ══════════════════════════════════════════════════════════════════════════════
// 4. Web Locks API Engine
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_web_locks_exclusive_fifo_and_shared_concurrency() {
    let engine = WebLocksEngine::new();
    let origin = "https://database.example.com";

    // 1. Reader 1 requests Shared lock -> Granted immediately
    let res1 = engine.request(origin, "users", LockMode::Shared, false, false, "reader1");
    let grant1 = match res1 {
        LockResult::Granted(g) => g,
        _ => panic!("Expected reader1 to be granted"),
    };

    // 2. Reader 2 requests Shared lock -> Granted concurrently
    let res2 = engine.request(origin, "users", LockMode::Shared, false, false, "reader2");
    let grant2 = match res2 {
        LockResult::Granted(g) => g,
        _ => panic!("Expected reader2 to be granted"),
    };

    // 3. Writer requests Exclusive lock -> Queued in FIFO order
    let res3 = engine.request(origin, "users", LockMode::Exclusive, false, false, "writer");
    assert!(matches!(res3, LockResult::Queued(_)));

    // 4. Reader 3 requests Shared lock -> Queued behind writer to prevent writer starvation
    let res4 = engine.request(origin, "users", LockMode::Shared, false, false, "reader3");
    assert!(matches!(res4, LockResult::Queued(_)));

    let snap = engine.query(origin);
    assert_eq!(snap.held.len(), 2);
    assert_eq!(snap.pending.len(), 2);

    // Release reader 1 -> writer still blocked by reader 2
    let unblocked1 = engine.release(origin, "users", grant1.grant_id);
    assert!(unblocked1.is_empty());

    // Release reader 2 -> writer becomes granted!
    let unblocked2 = engine.release(origin, "users", grant2.grant_id);
    assert_eq!(unblocked2.len(), 1);
    assert_eq!(unblocked2[0].client_id, "writer");
    assert_eq!(unblocked2[0].mode, LockMode::Exclusive);

    // Release writer -> reader 3 becomes granted!
    let unblocked3 = engine.release(origin, "users", unblocked2[0].grant_id);
    assert_eq!(unblocked3.len(), 1);
    assert_eq!(unblocked3[0].client_id, "reader3");
    assert_eq!(unblocked3[0].mode, LockMode::Shared);
}

#[test]
fn test_web_locks_steal_and_if_available() {
    let engine = WebLocksEngine::new();
    let origin = "https://app.com";

    // Client A acquires lock
    let res1 = engine.request(
        origin,
        "resource",
        LockMode::Exclusive,
        false,
        false,
        "clientA",
    );
    assert!(matches!(res1, LockResult::Granted(_)));

    // Client B with ifAvailable: true immediately fails
    let res2 = engine.request(
        origin,
        "resource",
        LockMode::Exclusive,
        true,
        false,
        "clientB",
    );
    assert_eq!(res2, LockResult::Unavailable);

    // Client C steals the lock immediately
    let res3 = engine.request(
        origin,
        "resource",
        LockMode::Exclusive,
        false,
        true,
        "clientC",
    );
    assert!(matches!(res3, LockResult::Granted(_)));

    let snap = engine.query(origin);
    assert_eq!(snap.held.len(), 1);
    assert_eq!(snap.held[0].client_id, "clientC");
}

// ══════════════════════════════════════════════════════════════════════════════
// 5. JavaScript Global Runtime End-to-End
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_js_runtime_broadcast_channel_and_web_locks() {
    let doc = mango_html::parse_html(
        "<!doctype html><html><head><title>Initial</title></head><body><div id='out'></div></body></html>",
    );
    let mut runtime = JsRuntime::new(doc, 800.0, 600.0);

    // Verify BroadcastChannel class exists, can be constructed, and posts messages
    let script = r#"
        if (typeof BroadcastChannel !== 'function') throw new Error("BroadcastChannel not defined");
        const bc = new BroadcastChannel('test-ch');
        if (bc.name !== 'test-ch') throw new Error("bc.name mismatch");
        if (typeof bc.postMessage !== 'function') throw new Error("postMessage not a function");
        bc.postMessage({ hello: 'world' });
        bc.close();
    "#;
    runtime
        .execute_script(script)
        .expect("BroadcastChannel script should execute successfully");

    // Verify navigator.locks and LockManager
    let locks_script = r#"
        if (!navigator.locks) throw new Error("navigator.locks missing");
        if (typeof navigator.locks.request !== 'function') throw new Error("locks.request missing");
        if (typeof navigator.locks.query !== 'function') throw new Error("locks.query missing");
    "#;
    runtime
        .execute_script(locks_script)
        .expect("Web Locks script should execute successfully");

    // Verify SharedWorker constructor is defined
    let sw_script = r#"
        if (typeof SharedWorker !== 'function') throw new Error("SharedWorker missing");
    "#;
    runtime
        .execute_script(sw_script)
        .expect("SharedWorker script should execute successfully");
}
