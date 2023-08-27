use std::{fmt::Debug, rc::Rc};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use futures::channel::mpsc::UnboundedReceiver;
use rand::{seq::SliceRandom, thread_rng};
use web_sys::HtmlImageElement;

use crate::{
    browser,
    engine::{self, Audio, Cell, Game, Image, KeyState, Point, Rect, Renderer, Sheet, SpriteSheet},
    segments::SEGMENT_GENERATORS,
};

use self::red_hat_boy::RedHatBoy;

mod red_hat_boy;

pub(crate) const WIDTH: i16 = 600;
pub(crate) const HEIGHT: i16 = 600;
const TIMELINE_MINIMUM: i16 = 1000;
const OBSTACLE_BUFFER: i16 = 20;

#[derive(Debug)]
pub(crate) struct WalkTheDog {
    machine: Option<WalkTheDogStateMachine>,
}

#[derive(Debug, derive_more::From)]
enum WalkTheDogStateMachine {
    Ready(WalkTheDogState<Ready>),
    Walking(WalkTheDogState<Walking>),
    GameOver(WalkTheDogState<GameOver>),
}
impl WalkTheDogStateMachine {
    fn new(walk: Walk) -> Self {
        WalkTheDogState::new(walk).into()
    }

    fn update(self, keystate: &KeyState) -> Self {
        log!("Keystate is {keystate:#?}");
        match self {
            WalkTheDogStateMachine::Ready(state) => state.update(keystate),
            WalkTheDogStateMachine::Walking(state) => state.update(keystate),
            WalkTheDogStateMachine::GameOver(state) => state.update(),
        }
    }

    fn draw(&self, renderer: &Renderer) {
        match self {
            WalkTheDogStateMachine::Ready(state) => state.draw(renderer),
            WalkTheDogStateMachine::Walking(state) => state.draw(renderer),
            WalkTheDogStateMachine::GameOver(state) => state.draw(renderer),
        }
    }
}

#[derive(Debug)]
struct WalkTheDogState<T> {
    walk: Walk,
    _state: T,
}

impl<T> WalkTheDogState<T> {
    fn draw(&self, renderer: &Renderer) {
        self.walk.draw(renderer);
    }
}

#[derive(Debug)]
struct Ready;

impl WalkTheDogState<Ready> {
    fn new(walk: Walk) -> WalkTheDogState<Ready> {
        Self {
            _state: Ready,
            walk,
        }
    }

    fn update(mut self, keystate: &KeyState) -> WalkTheDogStateMachine {
        self.walk.boy.update();

        if keystate.is_pressed("ArrowRight") {
            self.start_running()
        } else {
            self.into()
        }
    }

    fn start_running(mut self) -> WalkTheDogStateMachine {
        self.run_right();
        WalkTheDogStateMachine::Walking(WalkTheDogState {
            walk: self.walk,
            _state: Walking,
        })
    }

    fn run_right(&mut self) {
        self.walk.boy.run_right();
    }
}

#[derive(Debug)]
struct Walking;

impl WalkTheDogState<Walking> {
    fn update(mut self, keystate: &KeyState) -> WalkTheDogStateMachine {
        if keystate.is_pressed("ArrowDown") {
            self.walk.boy.slide();
        }
        if keystate.is_pressed("Space") {
            self.walk.boy.jump();
        }
        if keystate.is_pressed("KeyD") {
            self.walk.debug_mode = !self.walk.debug_mode;
        }

        self.walk.boy.update();

        let walking_speed = self.walk.velocity();
        for background in &mut self.walk.backgrounds {
            background.move_horizontally(walking_speed);
        }
        let [first_background, second_background] = &mut self.walk.backgrounds;
        if first_background.right() < 0 {
            first_background.set_x(second_background.right());
        }
        if second_background.right() < 0 {
            second_background.set_x(first_background.right());
        }

        self.walk.obstacles.retain(|obstacle| obstacle.right() > 0);

        for obstacle in &mut self.walk.obstacles {
            obstacle.move_horizontally(walking_speed);
            obstacle.check_intersection(&mut self.walk.boy);
        }

        if self.walk.timeline < TIMELINE_MINIMUM {
            self.walk.generate_next_segment();
        } else {
            self.walk.timeline += walking_speed;
        }

        if self.walk.knocked_out() {
            self.end_game()
        } else {
            self.into()
        }
    }

    fn end_game(self) -> WalkTheDogStateMachine {
        browser::draw_ui("<button id='new_game'>New Game</button>").unwrap();
        let element = browser::find_html_element_by_id("new_game").unwrap();
        let receiver = engine::add_click_handler(element);

        WalkTheDogState {
            walk: self.walk,
            _state: GameOver {
                new_game_event: receiver,
            },
        }
        .into()
    }
}

#[derive(Debug)]
struct GameOver {
    new_game_event: UnboundedReceiver<()>,
}

impl GameOver {
    fn new_game_pressed(&mut self) -> bool {
        matches!(self.new_game_event.try_next(), Ok(Some(())))
    }
}

impl WalkTheDogState<GameOver> {
    fn update(mut self) -> WalkTheDogStateMachine {
        if self._state.new_game_pressed() {
            self.new_game()
        } else {
            self.into()
        }
    }

    fn new_game(self) -> WalkTheDogStateMachine {
        if let Err(err) = browser::hide_ui() {
            error!("error hiding UI: {err:#?}");
        }
        WalkTheDogState {
            _state: Ready,
            walk: Walk::reset(self.walk),
        }
        .into()
    }
}

#[derive(Debug)]
pub(crate) struct Walk {
    debug_mode: bool,
    boy: RedHatBoy,
    backgrounds: [Image; 2],
    obstacle_sheet: Rc<SpriteSheet>,
    obstacles: Vec<Box<dyn Obstacle>>,
    stone: HtmlImageElement,
    timeline: i16,
}

impl Walk {
    async fn new() -> Result<Self> {
        let audio = Audio::new()?;
        let background_music = audio.load_sound("sounds/background_song.mp3").await?;
        audio.play_looping_sound(&background_music)?;

        let rhb_json = browser::fetch_json("sprites_sheets/rhb.json").await?;
        let rhb_sheet: Sheet = serde_wasm_bindgen::from_value(rhb_json).map_err(|err| {
            anyhow!("could not convert `rhb.json` into a `Sheet` structure: {err:#?}")
        })?;
        let image = engine::load_image("sprites_sheets/rhb.png").await?;
        let sound = audio.load_sound("sounds/SFX_Jump_23.mp3").await?;
        let rhb = RedHatBoy::new(rhb_sheet, image, audio, sound);

        let background = engine::load_image("images/BG.png").await?;
        let stone = engine::load_image("images/Stone.png").await?;

        let obstacle_json = browser::fetch_json("sprites_sheets/tiles.json").await?;
        let obstacle_sheet = Rc::new(SpriteSheet::new(
            serde_wasm_bindgen::from_value(obstacle_json).map_err(|err| {
                anyhow!("could not convert `tiles.json` into a `Sheet` structure: {err:#?}")
            })?,
            engine::load_image("sprites_sheets/tiles.png").await?,
        ));

        let background_width = background.width() as i16;
        let backgrounds = [
            Image::new(background.clone(), Point { x: 0, y: 0 }),
            Image::new(
                background,
                Point {
                    x: background_width,
                    y: 0,
                },
            ),
        ];

        let mut walk = Walk {
            debug_mode: cfg!(debug_assertions),
            boy: rhb,
            backgrounds,
            obstacles: vec![],
            obstacle_sheet,
            stone,
            timeline: 0,
        };
        walk.generate_next_segment();
        Ok(walk)
    }

    fn reset(mut walk: Self) -> Self {
        walk.obstacles = vec![];
        walk.timeline = 0;
        walk.generate_next_segment();
        walk.boy = RedHatBoy::reset(walk.boy);
        walk
    }

    fn velocity(&self) -> i16 {
        -self.boy.walking_speed()
    }

    fn knocked_out(&self) -> bool {
        self.boy.knocked_out()
    }

    fn generate_next_segment(&mut self) {
        let mut rng = thread_rng();

        let generator = SEGMENT_GENERATORS.choose(&mut rng).unwrap();

        let mut next_obstacles = generator(
            self.stone.clone(),
            Rc::clone(&self.obstacle_sheet),
            self.timeline + OBSTACLE_BUFFER,
        );

        self.timeline = rightmost(&next_obstacles);
        self.obstacles.append(&mut next_obstacles);
    }

    fn draw(&self, renderer: &Renderer) {
        renderer.set_debug_mode(self.debug_mode);

        for background in &self.backgrounds {
            background.draw(renderer);
        }
        self.boy.draw(renderer);
        for obstacle in &self.obstacles {
            obstacle.draw(renderer);
        }
    }
}

impl WalkTheDog {
    pub(crate) fn new() -> Self {
        WalkTheDog { machine: None }
    }
}

#[async_trait(?Send)]
impl Game for WalkTheDog {
    async fn initialize(&self) -> Result<Box<dyn Game>> {
        match self.machine {
            None => {
                let walk = Walk::new().await?;
                let machine = WalkTheDogStateMachine::new(walk);
                Ok(Box::new(Self {
                    machine: Some(machine),
                }))
            }
            Some(_) => Err(anyhow!("game already initialized")),
        }
    }

    fn update(&mut self, keystate: &KeyState) {
        if let Some(machine) = self.machine.take() {
            self.machine.replace(machine.update(keystate));
        }
        assert!(self.machine.is_some());
    }

    fn draw(&self, renderer: &Renderer) {
        renderer.clear(&Rect::from_xy(0, 0, WIDTH, HEIGHT));

        if let Some(machine) = &self.machine {
            machine.draw(renderer);
        }
    }
}

pub(crate) trait Obstacle: Debug {
    fn right(&self) -> i16;
    fn check_intersection(&self, boy: &mut RedHatBoy);
    fn draw(&self, renderer: &Renderer);
    fn move_horizontally(&mut self, x: i16);
}

#[derive(Debug, Clone)]
pub(crate) struct Platform {
    sheet: Rc<SpriteSheet>,
    bounding_boxes: Vec<Rect>,
    sprites: Vec<Cell>,
    position: Point,
}

impl Platform {
    pub(crate) fn new<'a>(
        sheet: Rc<SpriteSheet>,
        position: Point,
        sprite_names: impl IntoIterator<Item = &'a str> + 'a,
        bounding_boxes: impl IntoIterator<Item = Rect>,
    ) -> Self {
        let sprites = sprite_names
            .into_iter()
            .map(|sprite_name| sheet.cell(sprite_name).cloned())
            .collect::<Option<Vec<_>>>()
            .unwrap();
        let bounding_boxes = bounding_boxes
            .into_iter()
            .map(|mut bounding_box| {
                bounding_box.set_x(bounding_box.x() + position.x);
                bounding_box.set_y(bounding_box.y() + position.y);
                bounding_box
            })
            .collect();
        Self {
            sheet,
            position,
            sprites,
            bounding_boxes,
        }
    }
}

impl Obstacle for Platform {
    fn right(&self) -> i16 {
        self.bounding_boxes
            .last()
            .unwrap_or(&Rect::default())
            .right()
    }

    fn check_intersection(&self, boy: &mut RedHatBoy) {
        let boy_bounding_box = boy.bounding_box();

        if let Some(box_to_land_on) = self
            .bounding_boxes
            .iter()
            .find(|bounding_box| boy_bounding_box.intersects(bounding_box))
        {
            if boy.velocity_y() > 0 && boy_bounding_box.top() < box_to_land_on.top() {
                boy.land_on(box_to_land_on.top());
            } else {
                boy.knock_out();
            }
        }
    }

    fn draw(&self, renderer: &Renderer) {
        let mut x = 0;
        for sprite in &self.sprites {
            self.sheet.draw(
                renderer,
                &Rect::from_xy(
                    sprite.frame.x,
                    sprite.frame.y,
                    sprite.frame.w,
                    sprite.frame.h,
                ),
                &Rect::from_xy(
                    self.position.x + x,
                    self.position.y,
                    sprite.frame.w,
                    sprite.frame.h,
                ),
            );
            x += sprite.frame.w;
        }
        for bounding_box in &self.bounding_boxes {
            renderer.draw_bounding_box(bounding_box);
        }
    }

    fn move_horizontally(&mut self, x: i16) {
        self.position.x += x;
        for bounding_box in &mut self.bounding_boxes {
            bounding_box.set_x(bounding_box.x() + x);
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Barrier {
    image: Image,
}

impl Barrier {
    pub(crate) fn new(image: Image) -> Self {
        Self { image }
    }
}

impl Obstacle for Barrier {
    fn right(&self) -> i16 {
        self.image.right()
    }

    fn check_intersection(&self, boy: &mut RedHatBoy) {
        if boy.bounding_box().intersects(self.image.bounding_box()) {
            boy.knock_out();
        }
    }

    fn draw(&self, renderer: &Renderer) {
        self.image.draw(renderer);
        renderer.draw_bounding_box(self.image.bounding_box());
    }

    fn move_horizontally(&mut self, x: i16) {
        self.image.move_horizontally(x);
    }
}

fn rightmost(obstacle_list: &[Box<dyn Obstacle>]) -> i16 {
    obstacle_list
        .iter()
        .map(|obstacle| obstacle.right())
        .max()
        .unwrap_or(0)
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use futures::channel::mpsc::unbounded;
//     use std::collections::HashMap;
//     use web_sys::AudioBufferOptions;

//     use wasm_bindgen_test::wasm_bindgen_test;

//     wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

//     #[wasm_bindgen_test]
//     async fn test_transition_from_game_over_to_new_game() {
//         let (_, receiver) = unbounded();

//         let image = HtmlImageElement::new().unwrap();
//         let audio = Audio::new().unwrap();
//         let options = AudioBufferOptions::new(1, 8000.0);
//         let sound = audio.load_sound_from_options(&options).unwrap();
//         let rhb = RedHatBoy::new(
//             Sheet {
//                 frames: HashMap::new(),
//             },
//             image.clone(),
//             audio,
//             sound,
//         );
//         let sprite_sheet = SpriteSheet::new(
//             Sheet {
//                 frames: HashMap::new(),
//             },
//             image.clone(),
//         );
//         let walk = Walk {
//             boy: rhb,
//             backgrounds: [
//                 Image::new(image.clone(), Point { x: 0, y: 0 }),
//                 Image::new(image.clone(), Point { x: 0, y: 0 }),
//             ],
//             obstacles: vec![],
//             obstacle_sheet: Rc::new(sprite_sheet),
//             stone: image.clone(),
//             timeline: 0,
//             debug_mode: false,
//         };

//         let document = browser::document().unwrap();
//         document
//             .body()
//             .unwrap()
//             .insert_adjacent_html("afterbegin", "<div id='ui'></div>")
//             .unwrap();
//         browser::draw_ui("<p>This is the UI</p>").unwrap();
//         let state = WalkTheDogState {
//             _state: GameOver {
//                 new_game_event: receiver,
//             },
//             walk,
//         };

//         state.new_game();

//         let ui = browser::find_html_element_by_id("ui").unwrap();
//         assert_eq!(ui.child_element_count(), 0);
//     }
// }
