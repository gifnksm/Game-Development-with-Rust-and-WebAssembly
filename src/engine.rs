use std::{
    cell::{self, RefCell},
    collections::HashMap,
    rc::Rc,
    sync::Mutex,
};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use futures::channel::{
    mpsc::{unbounded, UnboundedReceiver},
    oneshot::channel,
};
use serde::Deserialize;
use wasm_bindgen::{prelude::Closure, JsCast, JsValue};
use web_sys::{
    AudioBuffer, AudioContext, CanvasRenderingContext2d, HtmlElement, HtmlImageElement,
    KeyboardEvent,
};

use crate::{
    browser,
    sound::{self, Looping},
};

pub(crate) async fn load_image(source: &str) -> Result<HtmlImageElement> {
    let image = browser::new_image()?;

    let (complete_tx, complete_rx) = channel::<Result<()>>();
    let success_tx = Rc::new(Mutex::new(Some(complete_tx)));
    let error_tx = Rc::clone(&success_tx);

    let success_callback = browser::closure_once(move || {
        if let Some(success_tx) = success_tx.lock().ok().and_then(|mut opt| opt.take()) {
            if let Err(err) = success_tx.send(Ok(())) {
                error!("error sending success_tx: {err:#?}");
            }
        }
    });

    let error_callback: Closure<dyn FnMut(JsValue)> = browser::closure_once(move |err| {
        if let Some(error_tx) = error_tx.lock().ok().and_then(|mut opt| opt.take()) {
            if let Err(err) = error_tx.send(Err(anyhow!("error loading image: {err:#?}"))) {
                error!("error sending error_tx: {err:#?}");
            }
        }
    });

    image.set_onload(Some(success_callback.as_ref().unchecked_ref()));
    image.set_onerror(Some(error_callback.as_ref().unchecked_ref()));
    image.set_src(source);

    complete_rx.await??;

    Ok(image)
}

#[async_trait(?Send)]
pub(crate) trait Game {
    async fn initialize(&self) -> Result<Box<dyn Game>>;
    fn update(&mut self, keystate: &KeyState);
    fn draw(&self, renderer: &Renderer);
}

const FRAME_SIZE: f32 = 1.0 / 60.0 * 1000.0;

#[derive(Debug)]
pub(crate) struct GameLoop {
    last_frame: f64,
    accumulated_delta: f32,
}

impl GameLoop {
    pub async fn start(game: impl Game + 'static) -> Result<()> {
        let mut keyevent_receiver = prepare_input()?;
        let mut game = game.initialize().await?;
        let mut game_loop = GameLoop {
            last_frame: browser::now()?,
            accumulated_delta: 0.0,
        };

        let renderer = Renderer::new(browser::context()?);

        let f = Rc::new(RefCell::new(None));
        let g = Rc::clone(&f);

        let mut keystate = KeyState::new();
        *g.borrow_mut() = Some(browser::create_raf_closure(move |perf| {
            process_input(&mut keystate, &mut keyevent_receiver);

            let frame_time = perf - game_loop.last_frame;
            game_loop.accumulated_delta += frame_time as f32;

            while game_loop.accumulated_delta > FRAME_SIZE {
                game.update(&keystate);
                game_loop.accumulated_delta -= FRAME_SIZE;
            }
            game_loop.last_frame = perf;
            game.draw(&renderer);

            if renderer.debug_mode.get() {
                unsafe {
                    draw_frame_rate(&renderer, frame_time);
                }
            }

            if let Err(err) = browser::request_animation_frame(f.borrow().as_ref().unwrap()) {
                error!("error requesting animation frame: {err:#?}");
            }
        }));

        browser::request_animation_frame(
            g.borrow()
                .as_ref()
                .ok_or_else(|| anyhow!("GameLoop: loop is `None`"))?,
        )?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Rect {
    pub(crate) position: Point,
    pub(crate) width: i16,
    pub(crate) height: i16,
}

impl Rect {
    pub(crate) const fn new(position: Point, width: i16, height: i16) -> Self {
        Self {
            position,
            width,
            height,
        }
    }

    pub(crate) const fn from_xy(x: i16, y: i16, width: i16, height: i16) -> Self {
        Rect::new(Point { x, y }, width, height)
    }

    pub(crate) const fn intersects(&self, rect: &Rect) -> bool {
        (self.left() < rect.right() && self.right() > rect.left())
            && (self.top() < rect.bottom() && self.bottom() > rect.top())
    }

    pub(crate) const fn x(&self) -> i16 {
        self.position.x
    }

    pub(crate) fn set_x(&mut self, x: i16) {
        self.position.x = x;
    }

    pub(crate) const fn y(&self) -> i16 {
        self.position.y
    }

    pub(crate) fn set_y(&mut self, y: i16) {
        self.position.y = y;
    }

    pub(crate) const fn left(&self) -> i16 {
        self.x()
    }

    pub(crate) const fn right(&self) -> i16 {
        self.x() + self.width
    }

    pub(crate) const fn top(&self) -> i16 {
        self.y()
    }

    pub(crate) const fn bottom(&self) -> i16 {
        self.y() + self.height
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Point {
    pub x: i16,
    pub y: i16,
}

#[derive(Debug)]
pub(crate) struct Renderer {
    context: CanvasRenderingContext2d,
    debug_mode: cell::Cell<bool>,
}

impl Renderer {
    fn new(context: CanvasRenderingContext2d) -> Self {
        Self {
            context,
            debug_mode: cell::Cell::new(false),
        }
    }

    pub(crate) fn set_debug_mode(&self, debug_mode: bool) {
        self.debug_mode.set(debug_mode);
    }

    pub(crate) fn clear(&self, rect: &Rect) {
        self.context.clear_rect(
            rect.x().into(),
            rect.y().into(),
            rect.width.into(),
            rect.height.into(),
        )
    }

    pub(crate) fn draw_image(&self, image: &HtmlImageElement, frame: &Rect, destination: &Rect) {
        self.context
            .draw_image_with_html_image_element_and_sw_and_sh_and_dx_and_dy_and_dw_and_dh(
                image,
                frame.x().into(),
                frame.y().into(),
                frame.width.into(),
                frame.height.into(),
                destination.x().into(),
                destination.y().into(),
                destination.width.into(),
                destination.height.into(),
            )
            .expect("error drawing image");
    }

    pub(crate) fn draw_entire_image(&self, image: &HtmlImageElement, position: Point) {
        self.context
            .draw_image_with_html_image_element(image, position.x.into(), position.y.into())
            .expect("error drawing image");
    }

    pub(crate) fn draw_rect(&self, rect: &Rect) {
        self.context.stroke_rect(
            rect.x().into(),
            rect.y().into(),
            rect.width.into(),
            rect.height.into(),
        );
    }

    pub(crate) fn draw_text(&self, test: &str, location: &Point) -> Result<()> {
        self.context.set_font("16pt serif");
        self.context
            .fill_text(test, location.x.into(), location.y.into())
            .map_err(|err| anyhow!("error drawing text: {err:#?}"))?;
        Ok(())
    }

    pub(crate) fn draw_bounding_box(&self, rect: &Rect) {
        if self.debug_mode.get() {
            self.draw_rect(rect);
        }
    }
}

#[derive(Debug, Clone)]
enum KeyPress {
    KeyUp(KeyboardEvent),
    KeyDown(KeyboardEvent),
}

fn prepare_input() -> Result<UnboundedReceiver<KeyPress>> {
    let (keydown_sender, keyevent_receiver) = unbounded();
    let keydown_sender = Rc::new(RefCell::new(keydown_sender));
    let keyup_sender = Rc::clone(&keydown_sender);

    let onkeydown = browser::closure_wrap(Box::new(move |keycode| {
        if let Err(err) = keydown_sender
            .borrow_mut()
            .start_send(KeyPress::KeyDown(keycode))
        {
            error!("error sending keydown event: {err:#?}");
        }
    }) as Box<dyn FnMut(KeyboardEvent)>);

    let onkeyup = browser::closure_wrap(Box::new(move |keycode| {
        if let Err(err) = keyup_sender
            .borrow_mut()
            .start_send(KeyPress::KeyUp(keycode))
        {
            error!("error sending keyup event: {err:#?}");
        }
    }) as Box<dyn FnMut(KeyboardEvent)>);

    browser::canvas()?.set_onkeydown(Some(onkeydown.as_ref().unchecked_ref()));
    browser::canvas()?.set_onkeyup(Some(onkeyup.as_ref().unchecked_ref()));
    onkeydown.forget();
    onkeyup.forget();
    Ok(keyevent_receiver)
}

fn process_input(state: &mut KeyState, keyevent_receiver: &mut UnboundedReceiver<KeyPress>) {
    loop {
        match keyevent_receiver.try_next() {
            Ok(None) => break,
            Err(_err) => break,
            Ok(Some(evt)) => match evt {
                KeyPress::KeyUp(evt) => state.set_released(&evt.code()),
                KeyPress::KeyDown(evt) => state.set_pressed(&evt.code(), evt),
            },
        }
    }
}

#[derive(Debug)]
pub(crate) struct KeyState {
    pressed_keys: HashMap<String, KeyboardEvent>,
}

impl KeyState {
    fn new() -> Self {
        KeyState {
            pressed_keys: HashMap::new(),
        }
    }

    pub(crate) fn is_pressed(&self, code: &str) -> bool {
        self.pressed_keys.contains_key(code)
    }

    fn set_pressed(&mut self, code: &str, event: KeyboardEvent) {
        log!("pressed: {:?}", code);
        self.pressed_keys.insert(code.into(), event);
    }

    fn set_released(&mut self, code: &str) {
        log!("released: {:?}", code);
        self.pressed_keys.remove(code);
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Image {
    element: HtmlImageElement,
    bounding_box: Rect,
}

impl Image {
    pub(crate) fn new(element: HtmlImageElement, position: Point) -> Self {
        let bounding_box = Rect::new(
            position,
            element.width().try_into().unwrap(),
            element.height().try_into().unwrap(),
        );
        Self {
            element,
            bounding_box,
        }
    }

    pub(crate) fn right(&self) -> i16 {
        self.bounding_box.right()
    }

    pub(crate) fn bounding_box(&self) -> &Rect {
        &self.bounding_box
    }

    pub(crate) fn set_x(&mut self, x: i16) {
        self.bounding_box.set_x(x);
    }

    pub(crate) fn move_horizontally(&mut self, distance: i16) {
        self.bounding_box.set_x(self.bounding_box.x() + distance);
    }

    pub(crate) fn draw(&self, renderer: &Renderer) {
        renderer.draw_entire_image(&self.element, self.bounding_box.position);
    }
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct Sheet {
    pub(crate) frames: HashMap<String, Cell>,
}

#[derive(Debug, Deserialize, Clone, Copy)]
pub(crate) struct SheetRect {
    pub(crate) x: i16,
    pub(crate) y: i16,
    pub(crate) w: i16,
    pub(crate) h: i16,
}

#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Cell {
    pub(crate) frame: SheetRect,
    pub(crate) sprite_source_size: SheetRect,
}

#[derive(Debug, Clone)]
pub(crate) struct SpriteSheet {
    sheet: Sheet,
    image: HtmlImageElement,
}

impl SpriteSheet {
    pub(crate) fn new(sheet: Sheet, image: HtmlImageElement) -> Self {
        Self { sheet, image }
    }

    pub(crate) fn cell(&self, name: &str) -> Option<&Cell> {
        self.sheet.frames.get(name)
    }

    pub(crate) fn draw(&self, renderer: &Renderer, source: &Rect, destination: &Rect) {
        renderer.draw_image(&self.image, source, destination);
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Audio {
    context: AudioContext,
}

#[derive(Debug, Clone)]
pub(crate) struct Sound {
    buffer: AudioBuffer,
}

impl Audio {
    pub(crate) fn new() -> Result<Self> {
        Ok(Audio {
            context: sound::create_audio_context()?,
        })
    }

    pub(crate) async fn load_sound(&self, filename: &str) -> Result<Sound> {
        let array_buffer = browser::fetch_array_buffer(filename).await?;
        let audio_buffer = sound::decode_audio_data(&self.context, &array_buffer).await?;
        Ok(Sound {
            buffer: audio_buffer,
        })
    }

    pub(crate) fn play_sound(&self, sound: &Sound) -> Result<()> {
        sound::play_sound(&self.context, &sound.buffer, Looping::No)
    }

    pub(crate) fn play_looping_sound(&self, sound: &Sound) -> Result<()> {
        sound::play_sound(&self.context, &sound.buffer, Looping::Yes)
    }
}

pub(crate) fn add_click_handler(elem: HtmlElement) -> UnboundedReceiver<()> {
    let (mut click_sender, click_receiver) = unbounded();
    let on_click = browser::closure_wrap(Box::new(move || {
        if let Err(err) = click_sender.start_send(()) {
            error!("error sending click event: {err:#?}");
        }
    }) as Box<dyn FnMut()>);
    elem.set_onclick(Some(on_click.as_ref().unchecked_ref()));
    on_click.forget();
    click_receiver
}

unsafe fn draw_frame_rate(renderer: &Renderer, frame_time: f64) {
    static mut FRAMES_COUNTED: i32 = 0;
    static mut TOTAL_FRAME_TIME: f64 = 0.0;
    static mut FRAME_RATE: i32 = 0;

    FRAMES_COUNTED += 1;
    TOTAL_FRAME_TIME += frame_time;

    if TOTAL_FRAME_TIME > 1000.0 {
        FRAME_RATE = FRAMES_COUNTED;
        TOTAL_FRAME_TIME = 0.0;
        FRAMES_COUNTED = 0;
    }

    if let Err(err) = renderer.draw_text(
        &format!("Frame Rate {FRAME_RATE}"),
        &Point { x: 400, y: 100 },
    ) {
        error!("error drawing frame rate: {err:#?}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_rects_that_intersect_on_the_left() {
        let rect1 = Rect {
            position: Point { x: 10, y: 10 },
            height: 100,
            width: 100,
        };
        let rect2 = Rect {
            position: Point { x: 0, y: 10 },
            height: 100,
            width: 100,
        };
        assert!(rect2.intersects(&rect1))
    }

    #[test]
    fn two_rects_that_intersect_on_the_right() {
        let rect1 = Rect {
            position: Point { x: 10, y: 10 },
            height: 100,
            width: 100,
        };
        let rect2 = Rect {
            position: Point { x: 90, y: 10 },
            height: 100,
            width: 100,
        };
        assert!(rect2.intersects(&rect1))
    }

    #[test]
    fn two_rects_that_intersect_on_the_top() {
        let rect1 = Rect {
            position: Point { x: 10, y: 10 },
            height: 100,
            width: 100,
        };
        let rect2 = Rect {
            position: Point { x: 10, y: 0 },
            height: 100,
            width: 100,
        };
        assert!(rect2.intersects(&rect1))
    }

    #[test]
    fn two_rects_that_intersect_on_the_bottom() {
        let rect1 = Rect {
            position: Point { x: 10, y: 10 },
            height: 100,
            width: 100,
        };
        let rect2 = Rect {
            position: Point { x: 10, y: 90 },
            height: 100,
            width: 100,
        };
        assert!(rect2.intersects(&rect1))
    }

    #[test]
    fn two_rects_that_does_not_intersect() {
        let rect1 = Rect {
            position: Point { x: 10, y: 10 },
            height: 100,
            width: 100,
        };
        let rect2 = Rect {
            position: Point { x: 110, y: 110 },
            height: 100,
            width: 100,
        };
        assert!(!rect2.intersects(&rect1))
    }
}
