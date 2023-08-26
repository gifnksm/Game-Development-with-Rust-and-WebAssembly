use std::{fmt::Debug, rc::Rc};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use rand::{seq::SliceRandom, thread_rng, Rng};
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
pub(crate) enum WalkTheDog {
    Loading,
    Loaded(Walk),
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
    fn velocity(&self) -> i16 {
        -self.boy.walking_speed()
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
}

impl WalkTheDog {
    pub(crate) fn new() -> Self {
        Self::Loading
    }
}

#[async_trait(?Send)]
impl Game for WalkTheDog {
    async fn initialize(&self) -> Result<Box<dyn Game>> {
        match self {
            Self::Loading => {
                let audio = Audio::new()?;
                let background_music = audio.load_sound("background_song.mp3").await?;
                audio.play_looping_sound(&background_music)?;

                let rhb_json = browser::fetch_json("rhb.json").await?;
                let rhb_sheet: Sheet = serde_wasm_bindgen::from_value(rhb_json).map_err(|err| {
                    anyhow!("could not convert `rhb.json` into a `Sheet` structure: {err:#?}")
                })?;
                let image = engine::load_image("rhb.png").await?;
                let sound = audio.load_sound("SFX_Jump_23.mp3").await?;
                let rhb = RedHatBoy::new(rhb_sheet, image, audio, sound);

                let background = engine::load_image("BG.png").await?;
                let stone = engine::load_image("Stone.png").await?;

                let obstacle_json = browser::fetch_json("tiles.json").await?;
                let obstacle_sheet = Rc::new(SpriteSheet::new(
                    serde_wasm_bindgen::from_value(obstacle_json).map_err(|err| {
                        anyhow!("could not convert `tiles.json` into a `Sheet` structure: {err:#?}")
                    })?,
                    engine::load_image("tiles.png").await?,
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
                Ok(Box::new(Self::Loaded(walk)))
            }
            Self::Loaded { .. } => Err(anyhow!("game already initialized")),
        }
    }

    fn update(&mut self, keystate: &KeyState) {
        if let Self::Loaded(walk) = self {
            if keystate.is_pressed("ArrowRight") {
                walk.boy.run_right();
            }
            if keystate.is_pressed("ArrowDown") {
                walk.boy.slide();
            }
            if keystate.is_pressed("Space") {
                walk.boy.jump();
            }

            if keystate.is_pressed("KeyD") {
                walk.debug_mode = !walk.debug_mode;
            }

            walk.boy.update();

            let velocity = walk.velocity();
            for background in &mut walk.backgrounds {
                background.move_horizontally(velocity);
            }
            let [first_background, second_background] = &mut walk.backgrounds;
            if first_background.right() < 0 {
                first_background.set_x(second_background.right());
            }
            if second_background.right() < 0 {
                second_background.set_x(first_background.right());
            }

            walk.obstacles.retain(|obstacle| obstacle.right() > 0);

            for obstacle in &mut walk.obstacles {
                obstacle.move_horizontally(velocity);
                obstacle.check_intersection(&mut walk.boy);
            }

            if walk.timeline < TIMELINE_MINIMUM {
                walk.generate_next_segment();
            } else {
                walk.timeline += velocity;
            }
        }
    }

    fn draw(&self, renderer: &Renderer) {
        renderer.clear(&Rect::from_xy(0, 0, WIDTH, HEIGHT));

        if let WalkTheDog::Loaded(walk) = self {
            renderer.set_debug_mode(walk.debug_mode);

            for background in &walk.backgrounds {
                background.draw(renderer);
            }
            walk.boy.draw(renderer);
            for obstacle in &walk.obstacles {
                obstacle.draw(renderer);
            }
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
