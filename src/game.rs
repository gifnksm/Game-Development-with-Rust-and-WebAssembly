use anyhow::{anyhow, Result};
use async_trait::async_trait;
use web_sys::HtmlImageElement;

use crate::{
    browser,
    engine::{self, Game, Image, KeyState, Point, Rect, Renderer, Sheet},
};

use self::red_hat_boy::RedHatBoy;

mod red_hat_boy;

const HEIGHT: i16 = 600;

#[derive(Debug)]
pub(crate) enum WalkTheDog {
    Loading,
    Loaded(Walk),
}

#[derive(Debug)]
pub(crate) struct Walk {
    boy: RedHatBoy,
    background: Image,
    stone: Image,
    platform: Platform,
}

impl WalkTheDog {
    pub(crate) fn new() -> Self {
        Self::Loading
    }
}

const LOW_PLATFORM: i16 = 420;
const HIGH_PLATFORM: i16 = 375;
const FIRST_PLATFORM: i16 = 370;

#[async_trait(?Send)]
impl Game for WalkTheDog {
    async fn initialize(&self) -> Result<Box<dyn Game>> {
        match self {
            Self::Loading => {
                let rhb_json = browser::fetch_json("rhb.json").await?;
                let rhb_sheet: Sheet = serde_wasm_bindgen::from_value(rhb_json).map_err(|err| {
                    anyhow!("could not convert `rhb.json` into a `Sheet` structure: {err:#?}")
                })?;

                let background = engine::load_image("BG.png").await?;
                let stone = engine::load_image("Stone.png").await?;

                let platform_json = browser::fetch_json("tiles.json").await?;
                let platform_sheet: Sheet =
                    serde_wasm_bindgen::from_value(platform_json).map_err(|err| {
                        anyhow!("could not convert `tiles.json` into a `Sheet` structure: {err:#?}")
                    })?;

                let platform = Platform::new(
                    platform_sheet,
                    engine::load_image("tiles.png").await?,
                    Point {
                        x: FIRST_PLATFORM,
                        y: LOW_PLATFORM,
                    },
                );

                let image = engine::load_image("rhb.png").await?;
                let rhb = RedHatBoy::new(rhb_sheet, image);
                Ok(Box::new(Self::Loaded(Walk {
                    boy: rhb,
                    background: Image::new(background, Point { x: 0, y: 0 }),
                    stone: Image::new(stone, Point { x: 150, y: 546 }),
                    platform,
                })))
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

            walk.boy.update();

            let boy_bounding_box = walk.boy.bounding_box();

            if walk
                .platform
                .bounding_boxes()
                .iter()
                .any(|bounding_box| boy_bounding_box.intersects(bounding_box))
            {
                let platform_y = walk
                    .platform
                    .bounding_boxes()
                    .iter()
                    .map(|bounding_box| bounding_box.y)
                    .fold(f32::NAN, f32::min);
                if walk.boy.velocity_y() >= 0
                    && boy_bounding_box.y + boy_bounding_box.height * 0.9 <= platform_y
                {
                    walk.boy.land_on(platform_y);
                } else {
                    walk.boy.knock_out();
                }
            }

            if boy_bounding_box.intersects(walk.stone.bounding_box()) {
                walk.boy.knock_out();
            }
        }
    }

    fn draw(&self, renderer: &Renderer) {
        renderer.clear(&Rect {
            x: 0.0,
            y: 0.0,
            width: 600.0,
            height: 600.0,
        });

        if let WalkTheDog::Loaded(walk) = self {
            walk.background.draw(renderer);
            walk.boy.draw(renderer);
            walk.stone.draw(renderer);
            walk.platform.draw(renderer);
        }
    }
}

#[derive(Debug, Clone)]
struct Platform {
    sheet: Sheet,
    image: HtmlImageElement,
    position: Point,
}

impl Platform {
    fn new(sheet: Sheet, image: HtmlImageElement, position: Point) -> Self {
        Self {
            sheet,
            image,
            position,
        }
    }

    fn destination_box(&self) -> Rect {
        let platform = self
            .sheet
            .frames
            .get("13.png")
            .expect("13.png does not exist");
        Rect {
            x: self.position.x.into(),
            y: self.position.y.into(),
            width: (platform.frame.w * 3).into(),
            height: platform.frame.h.into(),
        }
    }

    fn bounding_boxes(&self) -> Vec<Rect> {
        const X_OFFSET: f32 = 60.0;
        const END_HEIGHT: f32 = 54.0;
        let destination_box = self.destination_box();

        let bounding_box_one = Rect {
            x: destination_box.x,
            y: destination_box.y,
            width: X_OFFSET,
            height: END_HEIGHT,
        };

        let bounding_box_two = Rect {
            x: destination_box.x + X_OFFSET,
            y: destination_box.y,
            width: destination_box.width - X_OFFSET * 2.0,
            height: destination_box.height,
        };

        let bounding_box_three = Rect {
            x: destination_box.x + destination_box.width - X_OFFSET,
            y: destination_box.y,
            width: X_OFFSET,
            height: END_HEIGHT,
        };

        vec![bounding_box_one, bounding_box_two, bounding_box_three]
    }

    fn draw(&self, renderer: &Renderer) {
        let platform = self
            .sheet
            .frames
            .get("13.png")
            .expect("13.png does not exist");

        renderer.draw_image(
            &self.image,
            &Rect {
                x: platform.frame.x.into(),
                y: platform.frame.y.into(),
                width: (platform.frame.w * 3).into(),
                height: platform.frame.h.into(),
            },
            &self.destination_box(),
        );
        for bounding_box in self.bounding_boxes() {
            renderer.draw_rect(&bounding_box);
        }
    }
}
