use std::{iter, rc::Rc};

use rand::{seq::SliceRandom, Rng};
use web_sys::HtmlImageElement;

use crate::{
    engine::{Image, Point, Rect, SpriteSheet},
    game::{Barrier, Obstacle, Platform, HEIGHT},
};

const LOW_PLATFORM: i16 = 420;
const HIGH_PLATFORM: i16 = 375;

const TILE_WIDTH: i16 = 128;
const TILE_HEIGHT: i16 = 128;

const STONE_HEIGHT: i16 = 54;
const STONE_ON_GROUND: i16 = HEIGHT - STONE_HEIGHT;

const FLOATING_HEIGHT: i16 = 93;
const FLOATING_EDGE_WIDTH: i16 = 60;
const FLOATING_EDGE_HEIGHT: i16 = 54;

fn create_floating_platform(
    sprite_sheet: Rc<SpriteSheet>,
    position: Point,
    body_blocks: usize,
) -> Platform {
    let sprite_names = iter::once("13.png")
        .chain(iter::repeat("14.png").take(body_blocks))
        .chain(iter::once("15.png"));

    let platform_width: i16 = iter::repeat(TILE_WIDTH).take(body_blocks + 2).sum();

    let bounding_boxes = [
        Rect::from_xy(0, 0, FLOATING_EDGE_WIDTH, FLOATING_EDGE_HEIGHT),
        Rect::from_xy(
            FLOATING_EDGE_WIDTH,
            0,
            platform_width - (FLOATING_EDGE_WIDTH * 2),
            FLOATING_HEIGHT,
        ),
        Rect::from_xy(
            platform_width - FLOATING_EDGE_WIDTH,
            0,
            FLOATING_EDGE_WIDTH,
            FLOATING_EDGE_HEIGHT,
        ),
    ];

    Platform::new(sprite_sheet, position, sprite_names, bounding_boxes)
}

fn create_repeat_platform(
    sprite_sheet: Rc<SpriteSheet>,
    position: Point,
    mid_blocks: usize,
    tile_names: [&str; 3],
) -> Platform {
    let sprite_names = iter::once(tile_names[0])
        .chain(iter::repeat(tile_names[1]).take(mid_blocks))
        .chain(iter::once(tile_names[2]));
    let platform_width: i16 = iter::repeat(TILE_WIDTH).take(mid_blocks + 2).sum();
    let bounding_boxes = [Rect::from_xy(0, 0, platform_width, TILE_HEIGHT)];
    Platform::new(sprite_sheet, position, sprite_names, bounding_boxes)
}

fn create_filled_top(
    sprite_sheet: Rc<SpriteSheet>,
    position: Point,
    mid_blocks: usize,
) -> Platform {
    create_repeat_platform(
        sprite_sheet,
        position,
        mid_blocks,
        ["1.png", "2.png", "3.png"],
    )
}

fn create_filled_body(
    sprite_sheet: Rc<SpriteSheet>,
    position: Point,
    mid_blocks: usize,
) -> Platform {
    create_repeat_platform(
        sprite_sheet,
        position,
        mid_blocks,
        ["4.png", "5.png", "6.png"],
    )
}

fn create_filled_bottom(
    sprite_sheet: Rc<SpriteSheet>,
    position: Point,
    mid_blocks: usize,
) -> Platform {
    create_repeat_platform(
        sprite_sheet,
        position,
        mid_blocks,
        ["12.png", "9.png", "16.png"],
    )
}

pub(crate) type SegmentGeneratorFn =
    fn(HtmlImageElement, Rc<SpriteSheet>, i16) -> Vec<Box<dyn Obstacle>>;

pub(crate) const SEGMENT_GENERATORS: &[SegmentGeneratorFn] = &[floating_and_stone, mount, ceiling];

fn floating_and_stone(
    stone: HtmlImageElement,
    sprite_sheet: Rc<SpriteSheet>,
    offset_x: i16,
) -> Vec<Box<dyn Obstacle>> {
    let mut rng = rand::thread_rng();

    let stone_offset = *[150, 400].choose(&mut rng).unwrap();
    let platform_offset = *[370, 200].choose(&mut rng).unwrap();
    let platform_y = *[HIGH_PLATFORM, LOW_PLATFORM].choose(&mut rng).unwrap();
    let mid_blocks = rng.gen_range(0..4);

    vec![
        Box::new(Barrier::new(Image::new(
            stone,
            Point {
                x: offset_x + stone_offset,
                y: STONE_ON_GROUND,
            },
        ))),
        Box::new(create_floating_platform(
            sprite_sheet,
            Point {
                x: offset_x + platform_offset,
                y: platform_y,
            },
            mid_blocks,
        )),
    ]
}

fn mount(
    _stone: HtmlImageElement,
    sprite_sheet: Rc<SpriteSheet>,
    offset_x: i16,
) -> Vec<Box<dyn Obstacle>> {
    const INITIAL_MOUNT_OFFSET: i16 = 200;

    let mut rng = rand::thread_rng();
    let h_mid_blocks = rng.gen_range(0..4);
    let v_mid_blocks = rng.gen_range(0..2);

    let mut y = HEIGHT - TILE_HEIGHT;
    let mut obstacles: Vec<Box<dyn Obstacle>> = vec![];
    for _ in 0..v_mid_blocks {
        obstacles.push(Box::new(create_filled_body(
            sprite_sheet.clone(),
            Point {
                x: offset_x + INITIAL_MOUNT_OFFSET,
                y,
            },
            h_mid_blocks,
        )));
        y -= TILE_HEIGHT;
    }
    obstacles.push(Box::new(create_filled_top(
        sprite_sheet.clone(),
        Point {
            x: offset_x + INITIAL_MOUNT_OFFSET,
            y,
        },
        h_mid_blocks,
    )));
    obstacles
}

fn ceiling(
    _stone: HtmlImageElement,
    sprite_sheet: Rc<SpriteSheet>,
    offset_x: i16,
) -> Vec<Box<dyn Obstacle>> {
    const INITIAL_MOUNT_OFFSET: i16 = 200;

    let mut rng = rand::thread_rng();
    let h_mid_blocks = rng.gen_range(0..4);
    let v_mid_blocks = rng.gen_range(0..4);

    let mut y = 0;
    let mut obstacles: Vec<Box<dyn Obstacle>> = vec![];
    for _ in 0..v_mid_blocks {
        obstacles.push(Box::new(create_filled_body(
            sprite_sheet.clone(),
            Point {
                x: offset_x + INITIAL_MOUNT_OFFSET,
                y,
            },
            h_mid_blocks,
        )));
        y += TILE_HEIGHT;
    }
    obstacles.push(Box::new(create_filled_bottom(
        sprite_sheet.clone(),
        Point {
            x: offset_x + INITIAL_MOUNT_OFFSET,
            y,
        },
        h_mid_blocks,
    )));
    obstacles
}
