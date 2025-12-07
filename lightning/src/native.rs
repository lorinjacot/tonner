use dioxus::{
    logger::tracing::{span, Level},
    prelude::*,
};
use dioxus_native::{use_wgpu, CustomPaintSource, DeviceHandle};
use storm::{Engine, EngineBuilder};

#[component]
pub fn EngineProvider(children: Element) -> Element {
    let engine = use_signal_sync(|| None);
    use_context_provider(|| EngineContext(engine));

    let engine_initializer = EngineInitializer(engine);
    let canvas_src = use_wgpu(move || engine_initializer);

    rsx! {
        canvas {
            width: 0,
            height: 0,
            "src": canvas_src,
        }
        {children}
    }
}

#[derive(Clone)]
pub struct EngineContext(pub SyncSignal<Option<Engine>>);

struct EngineInitializer(SyncSignal<Option<Engine>>);

impl CustomPaintSource for EngineInitializer {
    fn resume(&mut self, device_handle: &DeviceHandle) {
        let device_handle = device_handle.clone();
        let mut signal = self.0;
        std::thread::spawn(move || {
            let _span = span!(Level::ERROR, "Engine initialization").entered();
            if let Some(engine) = signal.as_ref() {
                if engine.device() == &device_handle.device {
                    debug!("Engine already initalized with the correct device. Nothing to do.");
                    return;
                } else {
                    debug!("Engine already initalized but with a different device. Creating a new engine.");
                }
            }
            let engine = pollster::block_on(
                EngineBuilder::default()
                    .device(device_handle.device, device_handle.queue)
                    .build(),
            );
            *signal.write() = Some(engine);
            debug!("Engine initalized!");
        });
    }

    fn suspend(&mut self) {}

    fn render(
        &mut self,
        _ctx: dioxus_native::CustomPaintCtx<'_>,
        _width: u32,
        _height: u32,
        _scale: f64,
    ) -> Option<dioxus_native::TextureHandle> {
        None
    }
}
