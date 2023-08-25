use web_sys::HtmlImageElement;

use crate::engine::{Cell, Point, Rect, Renderer};

use self::states::{Falling, Idle, Jumping, KnockedOut, Running, Sliding, State};

use super::Sheet;

#[derive(Debug)]
pub(crate) struct RedHatBoy {
    state_machine: StateMachine,
    sprite_sheet: Sheet,
    image: HtmlImageElement,
}

impl RedHatBoy {
    pub(super) fn new(sheet: Sheet, image: HtmlImageElement) -> Self {
        Self {
            state_machine: State::new().into(),
            sprite_sheet: sheet,
            image,
        }
    }

    pub(super) fn pos_y(&self) -> i16 {
        self.state_machine.as_frame().position().y
    }

    pub(super) fn velocity_y(&self) -> i16 {
        self.state_machine.as_frame().velocity_y()
    }

    pub(super) fn update(&mut self) {
        self.state_machine = self.state_machine.update();
    }

    fn frame_name(&self) -> String {
        let frame = self.state_machine.as_frame();
        format!("{} ({}).png", frame.frame_name(), (frame.frame() / 3) + 1)
    }

    fn current_sprite(&self) -> Option<&Cell> {
        self.sprite_sheet.frames.get(&self.frame_name())
    }

    pub(super) fn bounding_box(&self) -> Rect {
        const X_OFFSET: f32 = 18.0;
        const Y_OFFSET: f32 = 14.0;
        const WIDTH_OFFSET: f32 = 28.0;
        let mut bounding_box = self.destination_box();
        bounding_box.x += X_OFFSET;
        bounding_box.width -= WIDTH_OFFSET;
        bounding_box.y += Y_OFFSET;
        bounding_box.height -= Y_OFFSET;
        bounding_box
    }

    fn destination_box(&self) -> Rect {
        let frame = self.state_machine.as_frame();
        let sprite = self.current_sprite().expect("cell not found");

        Rect {
            x: (frame.position().x + sprite.sprite_source_size.x as i16).into(),
            y: (frame.position().y + sprite.sprite_source_size.y as i16).into(),
            width: sprite.frame.w.into(),
            height: sprite.frame.h.into(),
        }
    }

    pub(super) fn draw(&self, renderer: &Renderer) {
        let sprite = self.current_sprite().expect("cell not found");
        renderer.draw_image(
            &self.image,
            &Rect {
                x: sprite.frame.x.into(),
                y: sprite.frame.y.into(),
                width: sprite.frame.w.into(),
                height: sprite.frame.h.into(),
            },
            &self.destination_box(),
        );
        renderer.draw_rect(&self.bounding_box());
    }

    pub(super) fn run_right(&mut self) {
        self.state_machine = self.state_machine.transition(Event::Run);
    }

    pub(super) fn slide(&mut self) {
        self.state_machine = self.state_machine.transition(Event::Slide);
    }

    pub(super) fn jump(&mut self) {
        self.state_machine = self.state_machine.transition(Event::Jump);
    }

    pub(super) fn land_on(&mut self, position: f32) {
        self.state_machine = self.state_machine.transition(Event::Land { position });
    }

    pub(super) fn knock_out(&mut self) {
        self.state_machine = self.state_machine.transition(Event::KnockOut);
    }
}

trait Frame {
    fn frame_name(&self) -> &'static str;
    fn frame(&self) -> u8;
    fn position(&self) -> Point;
    fn velocity_y(&self) -> i16;
}

#[derive(Debug, Clone, Copy)]
enum Event {
    Run,
    Slide,
    Jump,
    Land { position: f32 },
    KnockOut,
    Update,
}

#[derive(Debug, Clone, Copy, derive_more::From)]
enum StateMachine {
    Idle(State<Idle>),
    Running(State<Running>),
    Sliding(State<Sliding>),
    Jumping(State<Jumping>),
    Falling(State<Falling>),
    KnockedOut(State<KnockedOut>),
}

impl StateMachine {
    fn as_frame(&self) -> &dyn Frame {
        match self {
            Self::Idle(state) => state,
            Self::Running(state) => state,
            Self::Sliding(state) => state,
            Self::Jumping(state) => state,
            Self::Falling(state) => state,
            Self::KnockedOut(state) => state,
        }
    }

    fn transition(self, event: Event) -> Self {
        match (self, event) {
            (Self::Idle(state), Event::Run) => state.run(),

            (Self::Running(state), Event::Slide) => state.slide(),

            (Self::Running(state), Event::Jump) => state.jump(),

            (Self::Running(state), Event::Land { position }) => state.land_on(position),
            (Self::Sliding(state), Event::Land { position }) => state.land_on(position),
            (Self::Jumping(state), Event::Land { position }) => state.land_on(position),
            (Self::Falling(state), Event::Land { position }) => state.land_on(position),

            (Self::Running(state), Event::KnockOut) => state.knock_out(),
            (Self::Sliding(state), Event::KnockOut) => state.knock_out(),
            (Self::Jumping(state), Event::KnockOut) => state.knock_out(),

            (Self::Idle(state), Event::Update) => state.update(),
            (Self::Running(state), Event::Update) => state.update(),
            (Self::Sliding(state), Event::Update) => state.update(),
            (Self::Jumping(state), Event::Update) => state.update(),
            (Self::Falling(state), Event::Update) => state.update(),
            _ => self,
        }
    }

    fn update(self) -> Self {
        self.transition(Event::Update)
    }
}

mod states {
    use crate::{engine::Point, game::HEIGHT};

    use super::{Frame, StateMachine};

    const FLOOR: i16 = 479;
    const PLAYER_HEIGHT: i16 = HEIGHT - FLOOR;
    const STARTING_POINT: i16 = -20;
    const TERMINAL_VELOCITY: i16 = 20;
    const GRAVITY: i16 = 1;
    const RUNNING_SPEED: i16 = 4;
    const JUMP_SPEED: i16 = -25;

    trait FrameName {
        const FRAME_NAME: &'static str;
    }

    #[derive(Debug, Clone, Copy)]
    pub(super) struct State<S> {
        context: Context,
        _state: S,
    }

    impl<S> Frame for State<S> {
        fn frame_name(&self) -> &'static str {
            self.context.frame_config.frame_name
        }

        fn frame(&self) -> u8 {
            self.context.frame
        }

        fn position(&self) -> Point {
            self.context.position
        }

        fn velocity_y(&self) -> i16 {
            self.context.velocity.y
        }
    }

    #[derive(Debug, Clone, Copy)]
    struct FrameConfig {
        frame_name: &'static str,
        frames: u8,
    }
    impl FrameConfig {
        const fn new(frame_name: &'static str, frames: u8) -> Self {
            Self { frame_name, frames }
        }
    }

    const IDLE: FrameConfig = FrameConfig::new("Idle", 29);
    const RUN: FrameConfig = FrameConfig::new("Run", 23);
    const SLIDE: FrameConfig = FrameConfig::new("Slide", 14);
    const JUMP: FrameConfig = FrameConfig::new("Jump", 35);
    const DEAD: FrameConfig = FrameConfig::new("Dead", 29);

    #[derive(Debug, Clone, Copy)]
    pub(super) struct Idle;

    impl State<Idle> {
        pub(super) fn new() -> Self {
            Self {
                context: Context {
                    frame_config: &IDLE,
                    frame: 0,
                    position: Point {
                        x: STARTING_POINT,
                        y: FLOOR,
                    },
                    velocity: Point { x: 0, y: 0 },
                },
                _state: Idle,
            }
        }

        pub(super) fn update(mut self) -> StateMachine {
            self.context = self.context.update();
            self.into()
        }

        pub(super) fn run(self) -> StateMachine {
            State {
                context: self.context.reset_frame(&RUN).run_right(),
                _state: Running,
            }
            .into()
        }
    }

    #[derive(Debug, Clone, Copy)]
    pub(super) struct Running;

    impl State<Running> {
        pub(super) fn update(mut self) -> StateMachine {
            self.context = self.context.update();
            self.into()
        }

        pub(super) fn jump(self) -> StateMachine {
            State {
                context: self
                    .context
                    .set_vertical_velocity(JUMP_SPEED)
                    .reset_frame(&JUMP),
                _state: Jumping,
            }
            .into()
        }

        pub(super) fn slide(self) -> StateMachine {
            State {
                context: self.context.reset_frame(&SLIDE),
                _state: Sliding,
            }
            .into()
        }

        pub(super) fn land_on(mut self, position: f32) -> StateMachine {
            self.context = self
                .context
                .set_on(position as i16)
                .set_vertical_velocity(0);
            self.into()
        }

        pub(super) fn knock_out(self) -> StateMachine {
            State {
                context: self.context.reset_frame(&DEAD).stop(),
                _state: Falling,
            }
            .into()
        }
    }

    #[derive(Debug, Clone, Copy)]
    pub(super) struct Sliding;

    impl State<Sliding> {
        pub(super) fn update(mut self) -> StateMachine {
            self.context = self.context.update();

            if self.context.is_frames_end() {
                self.stand()
            } else {
                self.into()
            }
        }

        fn stand(self) -> StateMachine {
            State {
                context: self.context.reset_frame(&RUN),
                _state: Running,
            }
            .into()
        }

        pub(super) fn land_on(mut self, position: f32) -> StateMachine {
            self.context = self
                .context
                .set_on(position as i16)
                .set_vertical_velocity(0);
            self.into()
        }

        pub(super) fn knock_out(self) -> StateMachine {
            State {
                context: self.context.reset_frame(&DEAD).stop(),
                _state: Falling,
            }
            .into()
        }
    }

    #[derive(Debug, Clone, Copy)]
    pub(super) struct Jumping;

    impl State<Jumping> {
        pub(super) fn update(mut self) -> StateMachine {
            self.context = self.context.update();
            if self.context.position.y >= FLOOR {
                self.land_on(HEIGHT.into())
            } else {
                self.into()
            }
        }

        pub(super) fn land_on(self, position: f32) -> StateMachine {
            State {
                context: self
                    .context
                    .reset_frame(&RUN)
                    .set_on(position as i16)
                    .set_vertical_velocity(0),
                _state: Running,
            }
            .into()
        }

        pub(super) fn knock_out(self) -> StateMachine {
            State {
                context: self.context.reset_frame(&DEAD).stop(),
                _state: Falling,
            }
            .into()
        }
    }

    #[derive(Debug, Clone, Copy)]
    pub(super) struct Falling;

    impl State<Falling> {
        pub(super) fn update(mut self) -> StateMachine {
            self.context = self.context.update();
            if self.context.is_frames_end() {
                self.knock_out()
            } else {
                self.into()
            }
        }

        pub(super) fn land_on(mut self, position: f32) -> StateMachine {
            self.context = self
                .context
                .set_on(position as i16)
                .set_vertical_velocity(0);
            self.into()
        }

        fn knock_out(self) -> StateMachine {
            State {
                context: self.context,
                _state: KnockedOut,
            }
            .into()
        }
    }

    #[derive(Debug, Clone, Copy)]
    pub(super) struct KnockedOut;

    #[derive(Debug, Clone, Copy)]
    struct Context {
        frame_config: &'static FrameConfig,
        frame: u8,
        position: Point,
        velocity: Point,
    }

    impl Context {
        fn is_frames_end(&self) -> bool {
            self.frame >= self.frame_config.frames
        }

        fn update(mut self) -> Self {
            if self.frame < self.frame_config.frames {
                self.frame += 1;
            } else {
                self.frame = 0;
            }

            if self.velocity.y < TERMINAL_VELOCITY {
                self.velocity.y += GRAVITY;
            }

            self.position.x += self.velocity.x;
            self.position.y += self.velocity.y;
            if self.position.y > FLOOR {
                self.position.y = FLOOR;
            }
            self
        }

        fn reset_frame(mut self, frame_config: &'static FrameConfig) -> Self {
            self.frame_config = frame_config;
            self.frame = 0;
            self
        }

        fn run_right(mut self) -> Self {
            self.velocity.x += RUNNING_SPEED;
            self
        }

        fn set_vertical_velocity(mut self, y: i16) -> Self {
            self.velocity.y = y;
            self
        }

        fn set_on(mut self, position: i16) -> Self {
            let position = position - PLAYER_HEIGHT;
            self.position.y = position;
            self
        }

        fn stop(mut self) -> Self {
            self.velocity.x = 0;
            if self.velocity.y < 0 {
                self.velocity.y = 0;
            }
            self
        }
    }
}
