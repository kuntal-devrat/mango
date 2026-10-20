//! Multi-process site isolation, process sandboxing, and out-of-process iframes (OOPIF).
//!
//! Conforms to Chromium's Site Isolation architecture:
//! - Web content from different sites (schemeful eTLD+1) is executed in separate,
//!   sandboxed OS processes or isolated worker threads.
//! - Sandboxed renderers are denied raw network socket and filesystem access.
//! - Out-of-Process IFrames (OOPIFs) run cross-origin child documents in isolated
//!   renderers, communicating via structured IPC.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use mango_ipc::{IpcEndpoint, IpcError, IpcMessage};
use mango_net::cookies::get_registrable_domain;
use mango_net::url::Url;

static NEXT_SITE_INSTANCE_ID: AtomicU64 = AtomicU64::new(1);
static NEXT_PROCESS_ID: AtomicU32 = AtomicU32::new(100);

/// Unique identifier for a `SiteInstance`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SiteInstanceId(pub u64);

/// Unique identifier for an isolated OS/renderer process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProcessId(pub u32);

/// Lifecycle state of an isolated process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessStatus {
    Starting,
    Running,
    Terminated,
    Crashed(i32),
}

/// Sandboxing policy enforced upon a renderer process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxPolicy {
    /// True if direct socket connections are allowed (prohibited in untrusted renderers).
    pub allow_network_sockets: bool,
    /// True if raw local filesystem read access is allowed.
    pub allow_filesystem_read: bool,
    /// True if raw local filesystem write access is allowed.
    pub allow_filesystem_write: bool,
    /// True if hardware devices (camera, microphone) are allowed.
    pub allow_device_access: bool,
    /// The isolated schemeful site strictly locked to this process.
    pub isolated_site: String,
}

impl SandboxPolicy {
    /// Constructs a strict sandbox policy for an untrusted renderer executing web content.
    pub fn for_renderer(site: &str) -> Self {
        Self {
            allow_network_sockets: false,
            allow_filesystem_read: false,
            allow_filesystem_write: false,
            allow_device_access: false,
            isolated_site: site.to_string(),
        }
    }

    /// Privileged browser process policy.
    pub fn for_browser() -> Self {
        Self {
            allow_network_sockets: true,
            allow_filesystem_read: true,
            allow_filesystem_write: true,
            allow_device_access: true,
            isolated_site: "mango://browser".to_string(),
        }
    }

    /// Dedicated network transport process policy.
    pub fn for_network() -> Self {
        Self {
            allow_network_sockets: true,
            allow_filesystem_read: false,
            allow_filesystem_write: false,
            allow_device_access: false,
            isolated_site: "mango://network".to_string(),
        }
    }

    /// Dedicated GPU compositor process policy.
    pub fn for_gpu() -> Self {
        Self {
            allow_network_sockets: false,
            allow_filesystem_read: false,
            allow_filesystem_write: false,
            allow_device_access: true,
            isolated_site: "mango://gpu".to_string(),
        }
    }

    /// Validates whether an action is permitted by this sandbox policy.
    pub fn is_network_allowed(&self) -> bool {
        self.allow_network_sockets
    }

    pub fn is_filesystem_write_allowed(&self) -> bool {
        self.allow_filesystem_write
    }
}

/// A SiteInstance represents a group of documents that can script each other
/// (same origin / same schemeful site eTLD+1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiteInstance {
    pub id: SiteInstanceId,
    /// Schemeful site representation: scheme + "://" + eTLD+1 (or special URL schemes)
    pub site_url: String,
    pub process_id: Option<ProcessId>,
}

impl SiteInstance {
    /// Computes the schemeful site representation for a URL.
    pub fn compute_site(url: &Url) -> String {
        let scheme = url.scheme.to_ascii_lowercase();
        match scheme.as_str() {
            "about" => "about:blank".to_string(),
            "mango" => "mango://chrome".to_string(),
            "data" => "data://".to_string(),
            "file" => "file://".to_string(),
            _ => {
                let host = url.host.as_deref().unwrap_or("localhost");
                let reg_domain = get_registrable_domain(host);
                if let Some(port) = url.port {
                    format!("{scheme}://{reg_domain}:{port}")
                } else {
                    format!("{scheme}://{reg_domain}")
                }
            }
        }
    }

    /// Allocates a new SiteInstance for a given URL.
    pub fn for_url(url: &Url) -> Self {
        let id = SiteInstanceId(NEXT_SITE_INSTANCE_ID.fetch_add(1, Ordering::Relaxed));
        let site_url = Self::compute_site(url);
        Self {
            id,
            site_url,
            process_id: None,
        }
    }

    /// Checks if a target URL belongs to the same schemeful site.
    pub fn is_same_site(&self, url: &Url) -> bool {
        Self::compute_site(url) == self.site_url
    }
}

/// An active sandboxed renderer process managed by the browser process.
pub struct RendererProcess {
    pub process_id: ProcessId,
    pub site: String,
    pub sandbox: SandboxPolicy,
    pub status: ProcessStatus,
    /// Duplex IPC communication endpoint with the renderer process.
    pub ipc_endpoint: IpcEndpoint,
}

impl RendererProcess {
    pub fn new(process_id: ProcessId, site: String, ipc_endpoint: IpcEndpoint) -> Self {
        let sandbox = SandboxPolicy::for_renderer(&site);
        Self {
            process_id,
            site,
            sandbox,
            status: ProcessStatus::Running,
            ipc_endpoint,
        }
    }

    /// Sends an IPC message to the isolated renderer.
    pub fn send(&self, msg: &IpcMessage) -> Result<u32, IpcError> {
        self.ipc_endpoint.send(msg)
    }

    /// Attempts to poll an incoming message from the renderer.
    pub fn try_recv(&self) -> Result<Option<IpcMessage>, IpcError> {
        self.ipc_endpoint.try_recv()
    }
}

/// Node in a frame hierarchy representing main frame or child iframes.
#[derive(Debug, Clone)]
pub struct FrameTreeNode {
    pub frame_id: u64,
    pub url: Url,
    pub site_instance: SiteInstance,
    pub parent_frame_id: Option<u64>,
    pub is_oopif: bool,
}

/// The process pool and site-to-process coordinator.
pub struct ProcessPool {
    processes: HashMap<ProcessId, RendererProcess>,
    site_to_process: HashMap<String, ProcessId>,
}

impl Default for ProcessPool {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessPool {
    pub fn new() -> Self {
        Self {
            processes: HashMap::new(),
            site_to_process: HashMap::new(),
        }
    }

    /// Retrieves an existing isolated process or spawns a new sandboxed renderer for a site.
    pub fn get_or_create_process(
        &mut self,
        site_instance: &mut SiteInstance,
    ) -> (ProcessId, IpcEndpoint) {
        if let Some(&pid) = self.site_to_process.get(&site_instance.site_url)
            && let Some(proc) = self.processes.get(&pid)
            && proc.status == ProcessStatus::Running
        {
            site_instance.process_id = Some(pid);
            // Return a dummy endpoint since connection is already established
            let (dummy_ep, _) = IpcEndpoint::create_pair();
            return (pid, dummy_ep);
        }

        // Allocate a new sandboxed process
        let pid = ProcessId(NEXT_PROCESS_ID.fetch_add(1, Ordering::Relaxed));
        let (browser_ep, renderer_ep) = IpcEndpoint::create_pair();

        let renderer_proc = RendererProcess::new(pid, site_instance.site_url.clone(), browser_ep);
        self.processes.insert(pid, renderer_proc);
        self.site_to_process
            .insert(site_instance.site_url.clone(), pid);

        site_instance.process_id = Some(pid);
        (pid, renderer_ep)
    }

    /// Evaluates navigation and assigns Out-of-Process IFrame (OOPIF) isolation if cross-site.
    pub fn create_frame_node(
        &mut self,
        frame_id: u64,
        url: &Url,
        parent: Option<&FrameTreeNode>,
    ) -> FrameTreeNode {
        let mut site_inst = SiteInstance::for_url(url);
        let _ = self.get_or_create_process(&mut site_inst);

        let is_oopif = match parent {
            Some(p) => !site_inst.is_same_site(&p.url),
            None => false,
        };

        FrameTreeNode {
            frame_id,
            url: url.clone(),
            site_instance: site_inst,
            parent_frame_id: parent.map(|p| p.frame_id),
            is_oopif,
        }
    }

    /// Total count of active sandboxed renderer processes.
    pub fn process_count(&self) -> usize {
        self.processes.len()
    }

    /// Retrieves an active renderer process by ID.
    pub fn get_process(&self, pid: ProcessId) -> Option<&RendererProcess> {
        self.processes.get(&pid)
    }

    /// Terminates an isolated renderer process (e.g. tab closed or crash).
    pub fn terminate_process(&mut self, pid: ProcessId) {
        if let Some(proc) = self.processes.get_mut(&pid) {
            proc.status = ProcessStatus::Terminated;
            proc.ipc_endpoint.close();
            self.site_to_process.remove(&proc.site);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_site_instance_computation_and_e_tld() {
        let url_a = Url::parse("https://sub.example.com:443/page").unwrap();
        let site_a = SiteInstance::compute_site(&url_a);
        assert_eq!(site_a, "https://example.com");

        let url_b = Url::parse("https://other.example.com/api").unwrap();
        let site_b = SiteInstance::compute_site(&url_b);
        assert_eq!(site_b, "https://example.com");

        // Same site check
        let inst = SiteInstance::for_url(&url_a);
        assert!(inst.is_same_site(&url_b));

        let cross_site_url = Url::parse("https://malicious.com/").unwrap();
        assert!(!inst.is_same_site(&cross_site_url));
    }

    #[test]
    fn test_site_isolation_process_allocation() {
        let mut pool = ProcessPool::new();

        let url1 = Url::parse("https://site-a.com/index.html").unwrap();
        let mut inst1 = SiteInstance::for_url(&url1);
        let (pid1, _) = pool.get_or_create_process(&mut inst1);

        let url2 = Url::parse("https://site-b.com/profile").unwrap();
        let mut inst2 = SiteInstance::for_url(&url2);
        let (pid2, _) = pool.get_or_create_process(&mut inst2);

        // Strict Site Isolation: site-a and site-b MUST have different process IDs
        assert_ne!(pid1, pid2);
        assert_eq!(pool.process_count(), 2);

        // Subdomain on site-a must share site-a's process
        let url1_sub = Url::parse("https://api.site-a.com/feed").unwrap();
        let mut inst1_sub = SiteInstance::for_url(&url1_sub);
        let (pid1_sub, _) = pool.get_or_create_process(&mut inst1_sub);
        assert_eq!(pid1, pid1_sub);
    }

    #[test]
    fn test_oopif_frame_tree_isolation() {
        let mut pool = ProcessPool::new();

        let top_url = Url::parse("https://bank.com/account").unwrap();
        let top_frame = pool.create_frame_node(1, &top_url, None);
        assert!(!top_frame.is_oopif);

        // Cross-origin advertisement iframe inside bank.com
        let ad_url = Url::parse("https://ads.com/banner").unwrap();
        let ad_frame = pool.create_frame_node(2, &ad_url, Some(&top_frame));

        // Must be identified as an Out-of-Process IFrame
        assert!(ad_frame.is_oopif);
        assert_ne!(
            top_frame.site_instance.process_id,
            ad_frame.site_instance.process_id
        );

        // Sandbox check on ad renderer
        let ad_proc = pool
            .get_process(ad_frame.site_instance.process_id.unwrap())
            .unwrap();
        assert!(!ad_proc.sandbox.is_network_allowed());
        assert!(!ad_proc.sandbox.is_filesystem_write_allowed());
    }
}
