use web_sys::HtmlImageElement;

use crate::engine::{Point, Rect, Renderer};

use self::states::{Idle, Jumping, Running, Sliding, State};

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

    pub(super) fn update(&mut self) {
        self.state_machine = self.state_machine.update();
    }

    pub(super) fn draw(&self, renderer: &Renderer) {
        let frame = self.state_machine.as_frame();

        let frame_name = format!("{} ({}).png", frame.frame_name(), (frame.frame() / 3) + 1);
        let sprite = self
            .sprite_sheet
            .frames
            .get(&frame_name)
            .expect("cell not found");

        renderer.draw_image(
            &self.image,
            &Rect {
                x: sprite.frame.x.into(),
                y: sprite.frame.y.into(),
                width: sprite.frame.w.into(),
                height: sprite.frame.h.into(),
            },
            &Rect {
                x: frame.position().x.into(),
                y: frame.position().y.into(),
                width: sprite.frame.w.into(),
                height: sprite.frame.h.into(),
            },
        );
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
}

trait Frame {
    fn frame_name(&self) -> &'static str;
    fn frame(&self) -> u8;
    fn position(&self) -> Point;
}

#[derive(Debug, Clone, Copy)]
enum Event {
    Run,
    Slide,
    Update,
    Jump,
}

#[derive(Debug, Clone, Copy, derive_more::From)]
enum StateMachine {
    Idle(State<Idle>),
    Running(State<Running>),
    Sliding(State<Sliding>),
    Jumping(State<Jumping>),
}

impl StateMachine {
    fn as_frame(&self) -> &dyn Frame {
        match self {
            Self::Idle(state) => state,
            Self::Running(state) => state,
            Self::Sliding(state) => state,
            Self::Jumping(state) => state,
        }
    }

    fn transition(self, event: Event) -> Self {
        match (self, event) {
            (Self::Idle(state), Event::Run) => state.run(),
            (Self::Running(state), Event::Slide) => state.slide(),
            (Self::Running(state), Event::Jump) => state.jump(),
            (Self::Idle(state), Event::Update) => state.update(),
            (Self::Running(state), Event::Update) => state.update(),
            (Self::Sliding(state), Event::Update) => state.update(),
            (Self::Jumping(state), Event::Update) => state.update(),
            _ => self,
        }
    }

    fn update(self) -> Self {
        self.transition(Event::Update)
    }
}

mod states {
    use crate::engine::Point;

    use super::{Frame, StateMachine};

    const FLOOR: i16 = 475;
    const GRAVITY: i16 = 1;
    const RUNNING_SPEED: i16 = 3;
    const JUMP_SPEED: i16 = -25;

    trait FrameName {
        const FRAME_NAME: &'static str;
    }

    #[derive(Debug, Clone, Copy)]
    pub(super) struct State<S> {
        context: Context,
        _state: S,
    }

    impl<S> Frame for State<S>
    where
        S: FrameName,
    {
        fn frame_name(&self) -> &'static str {
            S::FRAME_NAME
        }

        fn frame(&self) -> u8 {
            self.context.frame
        }

        fn position(&self) -> Point {
            self.context.position
        }
    }

    #[derive(Debug, Clone, Copy)]
    pub(super) struct Idle;

    impl FrameName for Idle {
        const FRAME_NAME: &'static str = "Idle";
    }

    impl State<Idle> {
        const FRAMES: u8 = 29;

        pub(super) fn new() -> Self {
            Self {
                context: Context {
                    frame: 0,
                    position: Point { x: 0, y: FLOOR },
                    velocity: Point { x: 0, y: 0 },
                },
                _state: Idle,
            }
        }

        pub(super) fn update(mut self) -> StateMachine {
            self.context = self.context.update(Self::FRAMES);
            self.into()
        }

        pub(super) fn run(self) -> StateMachine {
            State {
                context: self.context.reset_frame().run_right(),
                _state: Running,
            }
            .into()
        }
    }

    #[derive(Debug, Clone, Copy)]
    pub(super) struct Running;

    impl FrameName for Running {
        const FRAME_NAME: &'static str = "Run";
    }

    impl State<Running> {
        const FRAMES: u8 = 23;

        pub(super) fn update(mut self) -> StateMachine {
            self.context = self.context.update(Self::FRAMES);
            self.into()
        }

        pub(super) fn jump(self) -> StateMachine {
            State {
                context: self.context.set_vertical_velocity(JUMP_SPEED).reset_frame(),
                _state: Jumping,
            }
            .into()
        }

        pub(super) fn slide(self) -> StateMachine {
            State {
                context: self.context.reset_frame(),
                _state: Sliding,
            }
            .into()
        }
    }

    #[derive(Debug, Clone, Copy)]
    pub(super) struct Sliding;

    impl FrameName for Sliding {
        const FRAME_NAME: &'static str = "Slide";
    }

    impl State<Sliding> {
        const FRAMES: u8 = 14;

        pub(super) fn update(mut self) -> StateMachine {
            self.context = self.context.update(Self::FRAMES);

            if self.context.frame >= Self::FRAMES {
                self.stand()
            } else {
                self.into()
            }
        }

        fn stand(self) -> StateMachine {
            State {
                context: self.context.reset_frame(),
                _state: Running,
            }
            .into()
        }
    }

    #[derive(Debug, Clone, Copy)]
    pub(super) struct Jumping;

    impl FrameName for Jumping {
        const FRAME_NAME: &'static str = "Jump";
    }

    impl State<Jumping> {
        const FRAMES: u8 = 35;

        pub(super) fn update(mut self) -> StateMachine {
            self.context = self.context.update(Self::FRAMES);
            if self.context.position.y >= FLOOR {
                self.land()
            } else {
                self.into()
            }
        }

        fn land(self) -> StateMachine {
            State {
                context: self.context.set_vertical_velocity(0).reset_frame(),
                _state: Running,
            }
            .into()
        }
    }

    #[derive(Debug, Clone, Copy)]
    pub(super) struct Context {
        frame: u8,
        position: Point,
        velocity: Point,
    }

    impl Context {
        pub(super) fn update(mut self, frame_count: u8) -> Self {
            self.velocity.y += GRAVITY;

            if self.frame < frame_count {
                self.frame += 1;
            } else {
                self.frame = 0;
            }

            self.position.x += self.velocity.x;
            self.position.y += self.velocity.y;
            if self.position.y > FLOOR {
                self.position.y = FLOOR;
            }
            self
        }

        fn reset_frame(mut self) -> Self {
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
    }
}
