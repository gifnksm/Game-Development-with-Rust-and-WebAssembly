use anyhow::{anyhow, Result};
use futures::Future;
use js_sys::ArrayBuffer;
use wasm_bindgen::{
    closure::{WasmClosure, WasmClosureFnOnce},
    prelude::*,
};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    CanvasRenderingContext2d, Document, Element, HtmlCanvasElement, HtmlElement, HtmlImageElement,
    Response, Window,
};

macro_rules! log {
    ($($t:tt)*) => {
        web_sys::console::log_1(&format!( $($t)*).into());
    }
}

macro_rules! error {
    ($($t:tt)*) => {
        web_sys::console::error_1(&format!( $($t)*).into());
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

pub(crate) async fn fetch_response(resource: &str) -> Result<Response> {
    fetch_with_str(resource)
        .await?
        .dyn_into()
        .map_err(|element| anyhow!("error converting {element:#?} to `Response`"))
}

pub(crate) async fn fetch_json(json_path: &str) -> Result<JsValue> {
    let resp = fetch_response(json_path).await?;
    JsFuture::from(
        resp.json()
            .map_err(|err| anyhow!("could not get JSON from response: {err:#?}"))?,
    )
    .await
    .map_err(|err| anyhow!("error fetching JSON: {err:#?}"))
}

pub(crate) async fn fetch_array_buffer(resource: &str) -> Result<ArrayBuffer> {
    let array_buffer = fetch_response(resource)
        .await?
        .array_buffer()
        .map_err(|err| anyhow!("could not get array buffer from response: {err:#?}"))?;
    JsFuture::from(array_buffer)
        .await
        .map_err(|err| anyhow!("error converting array buffer into a future: {err:#?}"))?
        .dyn_into()
        .map_err(|err| anyhow!("error converting ras JSValue to ArrayBuffer: {err:#?}"))
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

pub(crate) fn find_html_element_by_id(id: &str) -> Result<HtmlElement> {
    let doc = document()?;
    let element = doc
        .get_element_by_id(id)
        .ok_or_else(|| anyhow!("element with id {id} not found"))?;
    element
        .dyn_into()
        .map_err(|err| anyhow!("error converting to `HtmlElement`: {err:#?}"))
}

fn find_ui() -> Result<Element> {
    let doc = document()?;
    let ui = doc
        .get_element_by_id("ui")
        .ok_or_else(|| anyhow!("UI element not found"))?;
    Ok(ui)
}

pub(crate) fn draw_ui(html: &str) -> Result<()> {
    let ui = find_ui()?;
    ui.insert_adjacent_html("afterbegin", html)
        .map_err(|err| anyhow!("error inserting HTML: {err:#?}"))
}

pub(crate) fn hide_ui() -> Result<()> {
    let ui = find_ui()?;
    if let Some(child) = ui.first_child() {
        ui.remove_child(&child)
            .map_err(|err| anyhow!("error removing child: {err:#?}"))?;
        canvas()?
            .focus()
            .map_err(|err| anyhow!("error focusing canvas: {err:#?}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::wasm_bindgen_test;

    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    async fn test_error_loading_json() {
        let json = fetch_json("not_there.json").await;
        assert!(json.is_err());
    }
}
