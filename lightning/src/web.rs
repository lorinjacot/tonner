use dioxus::logger::tracing;
use dioxus::prelude::*;
use storm::{Engine, EngineBuilder};

#[component]
pub fn EngineProvider(children: Element) -> Element {
    let mut engine = use_signal(|| None);
    use_context_provider(|| EngineContext(engine));

    use_future(move || async move {
        let _span = tracing::span!(tracing::Level::ERROR, "Engine initialization").entered();
        let storm_engine = EngineBuilder::default().build().await;
        *engine.write() = Some(storm_engine);
        debug!("Engine initalized!");
    });

    children
}

#[derive(Clone)]
pub struct EngineContext(pub Signal<Option<Engine>>);