use web_sys::HtmlImageElement;

use crate::engine::{Audio, Cell, Point, Rect, Renderer, Sound};

use self::states::{Falling, Idle, Jumping, KnockedOut, Running, Sliding, State};

use super::Sheet;

#[derive(Debug)]
pub(crate) struct RedHatBoy {
    state_machine: StateMachine,
    sprite_sheet: Sheet,
    image: HtmlImageElement,
}

impl RedHatBoy {
    pub(super) fn new(
        sheet: Sheet,
        image: HtmlImageElement,
        audio: Audio,
        jump_sound: Sound,
    ) -> Self {
        Self {
            state_machine: State::new(audio, jump_sound).into(),
            sprite_sheet: sheet,
            image,
        }
    }

    pub(super) fn reset(boy: Self) -> Self {
        let frame = boy.state_machine.as_frame();
        let audio = frame.audio().clone();
        let jump_sound = frame.jump_sound().clone();
        Self::new(boy.sprite_sheet, boy.image, audio, jump_sound)
    }

    pub(super) fn walking_speed(&self) -> i16 {
        self.state_machine.as_frame().walking_speed()
    }

    pub(super) fn velocity_y(&self) -> i16 {
        self.state_machine.as_frame().velocity_y()
    }

    pub(super) fn knocked_out(&self) -> bool {
        self.state_machine.knocked_out()
    }

    pub(super) fn update(&mut self) {
        self.state_machine = self.state_machine.clone().update();
    }

    fn frame_name(&self) -> String {
        let frame = self.state_machine.as_frame();
        format!("{} ({}).png", frame.frame_name(), (frame.frame() / 3) + 1)
    }

    fn current_sprite(&self) -> Option<&Cell> {
        self.sprite_sheet.frames.get(&self.frame_name())
    }

    pub(super) fn bounding_box(&self) -> Rect {
        const X_OFFSET: i16 = 18;
        const Y_OFFSET: i16 = 14;
        const WIDTH_OFFSET: i16 = 28;
        let mut bounding_box = self.destination_box();
        bounding_box.set_x(bounding_box.x() + X_OFFSET);
        bounding_box.width -= WIDTH_OFFSET;
        bounding_box.set_y(bounding_box.y() + Y_OFFSET);
        bounding_box.height -= Y_OFFSET;
        bounding_box
    }

    fn destination_box(&self) -> Rect {
        let frame = self.state_machine.as_frame();
        let sprite = self.current_sprite().expect("cell not found");

        Rect::from_xy(
            frame.position().x + sprite.sprite_source_size.x,
            frame.position().y + sprite.sprite_source_size.y,
            sprite.frame.w,
            sprite.frame.h,
        )
    }

    pub(super) fn draw(&self, renderer: &Renderer) {
        let sprite = self.current_sprite().expect("cell not found");
        renderer.draw_image(
            &self.image,
            &Rect::from_xy(
                sprite.frame.x,
                sprite.frame.y,
                sprite.frame.w,
                sprite.frame.h,
            ),
            &self.destination_box(),
        );
        renderer.draw_bounding_box(&self.bounding_box());
    }

    pub(super) fn run_right(&mut self) {
        self.state_machine = self.state_machine.clone().transition(Event::Run);
    }

    pub(super) fn slide(&mut self) {
        self.state_machine = self.state_machine.clone().transition(Event::Slide);
    }

    pub(super) fn jump(&mut self) {
        self.state_machine = self.state_machine.clone().transition(Event::Jump);
    }

    pub(super) fn land_on(&mut self, position: i16) {
        self.state_machine = self
            .state_machine
            .clone()
            .transition(Event::Land { position });
    }

    pub(super) fn knock_out(&mut self) {
        self.state_machine = self.state_machine.clone().transition(Event::KnockOut);
    }
}

trait Frame {
    fn frame_name(&self) -> &'static str;
    fn frame(&self) -> u8;
    fn position(&self) -> Point;
    fn velocity_y(&self) -> i16;
    fn walking_speed(&self) -> i16;
    fn audio(&self) -> &Audio;
    fn jump_sound(&self) -> &Sound;
}

#[derive(Debug, Clone, Copy)]
enum Event {
    Run,
    Slide,
    Jump,
    Land { position: i16 },
    KnockOut,
    Update,
}

#[derive(Debug, Clone, derive_more::From)]
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

    fn knocked_out(&self) -> bool {
        matches!(self, Self::KnockedOut(_))
    }

    fn transition(self, event: Event) -> Self {
        match (self, event) {
            (Self::Idle(state), Event::Run) => state.run(),

            (Self::Running(state), Event::Slide) => state.slide(),
            (Self::Sliding(state), Event::Slide) => state.slide(),

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
            (this, _) => this,
        }
    }

    fn update(self) -> Self {
        self.transition(Event::Update)
    }
}

mod states {
    use crate::{
        engine::{Audio, Point, Sound},
        game::HEIGHT,
    };

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

    #[derive(Debug, Clone)]
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

        fn walking_speed(&self) -> i16 {
            self.context.velocity.x
        }

        fn audio(&self) -> &Audio {
            &self.context.audio
        }

        fn jump_sound(&self) -> &Sound {
            &self.context.jump_sound
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
        pub(super) fn new(audio: Audio, jump_sound: Sound) -> Self {
            Self {
                context: Context {
                    frame_config: &IDLE,
                    frame: 0,
                    position: Point {
                        x: STARTING_POINT,
                        y: FLOOR,
                    },
                    velocity: Point { x: 0, y: 0 },
                    hold_state: false,
                    audio,
                    jump_sound,
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
                    .reset_frame(&JUMP)
                    .play_jump_sound(),
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

        pub(super) fn land_on(mut self, position: i16) -> StateMachine {
            self.context = self.context.set_on(position).set_vertical_velocity(0);
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
            let hold_state = self.context.hold_state;
            self.context = self.context.update();

            if !hold_state && self.context.is_frames_end() {
                self.stand()
            } else {
                self.into()
            }
        }

        pub(super) fn slide(mut self) -> StateMachine {
            self.context.hold_state = true;
            self.into()
        }

        fn stand(self) -> StateMachine {
            State {
                context: self.context.reset_frame(&RUN),
                _state: Running,
            }
            .into()
        }

        pub(super) fn land_on(mut self, position: i16) -> StateMachine {
            self.context = self.context.set_on(position).set_vertical_velocity(0);
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
                self.land_on(HEIGHT)
            } else {
                self.into()
            }
        }

        pub(super) fn land_on(self, position: i16) -> StateMachine {
            State {
                context: self
                    .context
                    .reset_frame(&RUN)
                    .set_on(position)
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

        pub(super) fn land_on(mut self, position: i16) -> StateMachine {
            self.context = self.context.set_on(position).set_vertical_velocity(0);
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

    #[derive(Debug, Clone)]
    struct Context {
        frame_config: &'static FrameConfig,
        frame: u8,
        position: Point,
        velocity: Point,
        hold_state: bool,
        audio: Audio,
        jump_sound: Sound,
    }

    impl Context {
        fn is_frames_end(&self) -> bool {
            self.frame >= self.frame_config.frames
        }

        fn update(mut self) -> Self {
            self.hold_state = false;
            if self.frame < self.frame_config.frames {
                self.frame += 1;
            } else {
                self.frame = 0;
            }

            if self.velocity.y < TERMINAL_VELOCITY {
                self.velocity.y += GRAVITY;
            }

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

        fn play_jump_sound(self) -> Self {
            if let Err(err) = self.audio.play_sound(&self.jump_sound) {
                log!("Error playing jump sound: {err:#?}");
            }
            self
        }
    }
}
