//! WebRender engine execution and GL context management.

use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use webrender::api::units::*;
use webrender::api::*;
use webrender::{create_webrender_instance, RenderApi, Renderer, Transaction, WebRenderOptions};

use super::builder::build_webrender_display_list;
use crate::display_list::DisplayList;

/// Global mutex to serialize SWGL context calls across threads since SWGL uses a process-static `Context* ctx`.
static SWGL_MUTEX: Mutex<()> = Mutex::new(());

#[derive(Clone)]
struct SyncNotifier {
    publish_id: Arc<AtomicU64>,
    cvar_pair: Arc<(Mutex<()>, Condvar)>,
}

impl SyncNotifier {
    fn new(publish_id: Arc<AtomicU64>, cvar_pair: Arc<(Mutex<()>, Condvar)>) -> Self {
        Self {
            publish_id,
            cvar_pair,
        }
    }
}

impl RenderNotifier for SyncNotifier {
    fn clone(&self) -> Box<dyn RenderNotifier> {
        Box::new(SyncNotifier {
            publish_id: Arc::clone(&self.publish_id),
            cvar_pair: Arc::clone(&self.cvar_pair),
        })
    }

    fn wake_up(&self, _composite_needed: bool) {}

    fn new_frame_ready(
        &self,
        _doc_id: DocumentId,
        publish_id: FramePublishId,
        _params: &FrameReadyParams,
    ) {
        self.publish_id.store(publish_id.0, Ordering::SeqCst);
        let (lock, cvar) = &*self.cvar_pair;
        let _guard = lock.lock().unwrap();
        cvar.notify_all();
    }
}

pub struct WebRenderEngine {
    renderer: Option<Renderer>,
    api: RenderApi,
    document_id: DocumentId,
    pipeline_id: PipelineId,
    epoch: Epoch,
    publish_id: Arc<AtomicU64>,
    cvar_pair: Arc<(Mutex<()>, Condvar)>,
    swgl_ctx: Rc<swgl::Context>,
    width: u32,
    height: u32,
}

impl WebRenderEngine {
    pub fn new(width: u32, height: u32) -> Result<Self, String> {
        let swgl_ctx = Rc::new(swgl::Context::create());
        swgl_ctx.make_current();
        swgl_ctx.init_default_framebuffer(
            0,
            0,
            width as i32,
            height as i32,
            0,
            std::ptr::null_mut(),
        );

        let opts = WebRenderOptions {
            resource_override_path: None,
            clear_color: ColorF::new(0.0, 0.0, 0.0, 0.0),
            clear_caches_with_quads: false,
            ..Default::default()
        };

        let publish_id = Arc::new(AtomicU64::new(0));
        let cvar_pair = Arc::new((Mutex::new(()), Condvar::new()));
        let notifier = Box::new(SyncNotifier::new(
            Arc::clone(&publish_id),
            Arc::clone(&cvar_pair),
        ));

        let (renderer, sender) = create_webrender_instance(
            Rc::clone(&swgl_ctx) as Rc<dyn gleam::gl::Gl>,
            notifier,
            opts,
            None,
        )
        .map_err(|e| format!("Failed to create WebRender instance: {e:?}"))?;

        let api = sender.create_api();
        let document_id = api.add_document(DeviceIntSize::new(width as i32, height as i32));
        let pipeline_id = PipelineId(0, 0);

        Ok(Self {
            renderer: Some(renderer),
            api,
            document_id,
            pipeline_id,
            epoch: Epoch(0),
            publish_id,
            cvar_pair,
            swgl_ctx,
            width,
            height,
        })
    }

    /// Executes a rendering closure using a synchronized [`WebRenderEngine`] instance.
    pub fn with_engine<F, R>(width: u32, height: u32, f: F) -> Option<R>
    where
        F: FnOnce(&mut WebRenderEngine) -> Option<R>,
    {
        let _guard = SWGL_MUTEX.lock().unwrap();
        let mut engine = WebRenderEngine::new(width, height).ok()?;
        f(&mut engine)
    }

    /// Renders a [`DisplayList`] to a raw `0xAARRGGBB` pixel buffer.
    pub fn render(
        &mut self,
        display_list: &DisplayList,
        width: u32,
        height: u32,
    ) -> Option<Vec<u32>> {
        if width == 0 || height == 0 {
            return Some(Vec::new());
        }

        if self.width != width || self.height != height {
            self.api.delete_document(self.document_id);
            self.document_id = self
                .api
                .add_document(DeviceIntSize::new(width as i32, height as i32));
            self.width = width;
            self.height = height;
        }

        self.swgl_ctx.make_current();
        self.swgl_ctx.init_default_framebuffer(
            0,
            0,
            width as i32,
            height as i32,
            0,
            std::ptr::null_mut(),
        );

        let renderer = self.renderer.as_mut()?;

        let prev_publish_id = self.publish_id.load(Ordering::SeqCst);

        let mut txn = Transaction::new();
        let device_rect = DeviceIntRect::from_origin_and_size(
            DeviceIntPoint::zero(),
            DeviceIntSize::new(width as i32, height as i32),
        );
        txn.set_document_view(device_rect);

        let built_frame = build_webrender_display_list(
            display_list,
            self.pipeline_id,
            width,
            height,
            &self.api,
            &mut txn,
        );

        self.epoch.0 += 1;
        txn.set_root_pipeline(self.pipeline_id);
        txn.set_display_list(
            self.epoch,
            (built_frame.pipeline_id, built_frame.display_list),
        );
        txn.generate_frame(0, true, false, RenderReasons::empty());
        self.api.send_transaction(self.document_id, txn);

        // Wait until this specific frame publish notification arrives
        {
            let (lock, cvar) = &*self.cvar_pair;
            let mut guard = lock.lock().unwrap();
            while self.publish_id.load(Ordering::SeqCst) <= prev_publish_id {
                let (new_guard, timeout) =
                    cvar.wait_timeout(guard, Duration::from_secs(5)).unwrap();
                guard = new_guard;
                if timeout.timed_out() {
                    break;
                }
            }
        }

        renderer.update();
        let device_size = DeviceIntSize::new(width as i32, height as i32);
        renderer.render(device_size, 0).ok()?;

        let read_rect = FramebufferIntRect::from_origin_and_size(
            FramebufferIntPoint::zero(),
            FramebufferIntSize::new(width as i32, height as i32),
        );
        let raw_bytes = renderer.read_pixels_rgba8(read_rect);

        // Convert raw RGBA bytes into canonical 0xAARRGGBB format.
        let mut pixels = vec![0u32; (width * height) as usize];
        let row_stride = (width * 4) as usize;
        for y in 0..height {
            let src_y = (height - 1 - y) as usize;
            let src_row = &raw_bytes[src_y * row_stride..(src_y + 1) * row_stride];
            let dst_row = &mut pixels[(y * width) as usize..((y + 1) * width) as usize];
            for x in 0..width as usize {
                let r = src_row[x * 4] as u32;
                let g = src_row[x * 4 + 1] as u32;
                let b = src_row[x * 4 + 2] as u32;
                let a = src_row[x * 4 + 3] as u32;
                dst_row[x] = (a << 24) | (r << 16) | (g << 8) | b;
            }
        }

        // Transient images cleanup can be performed or deferred to document teardown.
        let _ = built_frame.transient_images;

        Some(pixels)
    }
}

impl Drop for WebRenderEngine {
    fn drop(&mut self) {
        self.api.shut_down(true);
        if let Some(renderer) = self.renderer.take() {
            renderer.deinit();
        }
    }
}
