use std::collections::HashMap;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::Deserialize;

use crate::{
    browser,
    engine::{self, Game, KeyState, Rect, Renderer},
};

use self::red_hat_boy::RedHatBoy;

mod red_hat_boy;

#[derive(Debug, Deserialize, Clone)]
struct Sheet {
    frames: HashMap<String, Cell>,
}

#[derive(Debug, Deserialize, Clone, Copy)]
struct SheetRect {
    x: u16,
    y: u16,
    w: u16,
    h: u16,
}

#[derive(Debug, Deserialize, Clone, Copy)]
struct Cell {
    frame: SheetRect,
}

#[derive(Debug)]
pub(crate) enum WalkTheDog {
    Loading,
    Loaded(RedHatBoy),
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
                let json = browser::fetch_json("rhb.json").await?;
                let sheet: Sheet = serde_wasm_bindgen::from_value(json).map_err(|err| {
                    anyhow!("could not convert `rhb.json` into a `Sheet` structure: {err:#?}")
                })?;

                let image = engine::load_image("rhb.png").await?;
                let rhb = RedHatBoy::new(sheet, image);
                Ok(Box::new(Self::Loaded(rhb)))
            }
            Self::Loaded { .. } => Err(anyhow!("game already initialized")),
        }
    }

    fn update(&mut self, keystate: &KeyState) {
        if let Self::Loaded(rhb) = self {
            if keystate.is_pressed("ArrowRight") {
                rhb.run_right();
            }
            if keystate.is_pressed("ArrowDown") {
                rhb.slide();
            }
            if keystate.is_pressed("Space") {
                rhb.jump();
            }

            rhb.update();
        }
    }

    fn draw(&self, renderer: &Renderer) {
        renderer.clear(&Rect {
            x: 0.0,
            y: 0.0,
            width: 600.0,
            height: 600.0,
        });

        if let WalkTheDog::Loaded(rhb) = self {
            rhb.draw(renderer);
        }
    }
}
