use anyhow::{anyhow, Result};
use futures::Future;
use wasm_bindgen::{
    closure::{WasmClosure, WasmClosureFnOnce},
    prelude::*,
};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    CanvasRenderingContext2d, Document, HtmlCanvasElement, HtmlImageElement, Response, Window,
};

macro_rules! log {
    ($($t:tt)*) => {
        web_sys::console::log_1(&format!( $($t)*).into());
    }
}

pub(crate) fn window() -> Result<Window> {
    web_sys::window().ok_or_else(|| anyhow!("no global `window` exists"))
}

pub(crate) fn document() -> Result<Document> {
    window()?
        .document()
        .ok_or_else(|| anyhow!("should have a `document` on `window`"))
}

pub(crate) fn canvas() -> Result<HtmlCanvasElement> {
    document()?
        .get_element_by_id("canvas")
        .ok_or_else(|| anyhow!("no canvas found"))?
        .dyn_into::<HtmlCanvasElement>()
        .map_err(|element| anyhow!("error converting {element:#?} to `HtmlCanvasElement`"))
}

pub(crate) fn context() -> Result<CanvasRenderingContext2d> {
    canvas()?
        .get_context("2d")
        .map_err(|js_value| anyhow!("error getting 2d context {js_value:#?}"))?
        .ok_or_else(|| anyhow!("no 2d context found"))?
        .dyn_into::<CanvasRenderingContext2d>()
        .map_err(|element| anyhow!("error converting {element:#?} to `CanvasRenderingContext2d`"))
}

pub(crate) fn spawn_local<F>(future: F)
where
    F: Future<Output = ()> + 'static,
{
    wasm_bindgen_futures::spawn_local(future);
}

pub(crate) async fn fetch_with_str(resource: &str) -> Result<JsValue> {
    JsFuture::from(window()?.fetch_with_str(resource))
        .await
        .map_err(|err| anyhow!("error fetching {err:#?}"))
}

pub(crate) async fn fetch_json(json_path: &str) -> Result<JsValue> {
    let resp_value = fetch_with_str(json_path).await?;
    let resp: Response = resp_value
        .dyn_into()
        .map_err(|element| anyhow!("error converting {element:#?} to `Response`"))?;
    JsFuture::from(
        resp.json()
            .map_err(|err| anyhow!("could not get JSON from response: {err:#?}"))?,
    )
    .await
    .map_err(|err| anyhow!("error fetching JSON: {err:#?}"))
}

pub(crate) fn new_image() -> Result<HtmlImageElement> {
    HtmlImageElement::new().map_err(|err| anyhow!("could not create `HtmlImageElement`: {err:#?}"))
}

pub(crate) fn closure_once<F, A, R>(fn_once: F) -> Closure<F::FnMut>
where
    F: 'static + WasmClosureFnOnce<A, R>,
{
    Closure::once(fn_once)
}

pub(crate) fn closure_wrap<T>(data: Box<T>) -> Closure<T>
where
    T: WasmClosure + ?Sized,
{
    Closure::wrap(data)
}

pub(crate) type LoopClosure = Closure<dyn FnMut(f64)>;
pub(crate) fn request_animation_frame(callback: &LoopClosure) -> Result<i32> {
    window()?
        .request_animation_frame(callback.as_ref().unchecked_ref())
        .map_err(|err| anyhow!("cannot request animation frame: {err:#?}"))
}

pub(crate) fn create_raf_closure(f: impl FnMut(f64) + 'static) -> LoopClosure {
    closure_wrap(Box::new(f))
}

pub(crate) fn now() -> Result<f64> {
    Ok(window()?
        .performance()
        .ok_or_else(|| anyhow!("performance object not found"))?
        .now())
}
