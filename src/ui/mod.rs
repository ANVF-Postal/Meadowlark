use log::info;
use ringbuf::{Consumer, RingBuffer};
// use rusty_daw_io::{
//     ConfigStatus, FatalStreamError, SpawnRtThreadError, StreamHandle, SystemOptions,
// };

pub mod components;

use tuix::style::themes::DEFAULT_THEME;
use tuix::*;

use self::components::LevelsMeter;

use crate::frontend::FrontendState;

const THEME: &str = include_str!("theme.css");

// use crate::rt_thread::{MainFatalErrorHandler, MainRtHandler, RtState};

pub struct App {
    frontend_state: FrontendState,
}

impl App {
    pub fn new(frontend_state: FrontendState) -> Self {
        Self { frontend_state }
    }
}

impl Widget for App {
    type Ret = Entity;
    fn on_build(&mut self, state: &mut State, entity: Entity) -> Self::Ret {
        let row = Row::new().build(state, entity, |builder| {
            builder.set_width(Stretch(1.0)).set_height(Stretch(1.0))
        });

        ValueKnob::new("Amplitude", 0.0, 0.0, 1.0).build(state, row, |builder| {
            builder
                .set_width(Pixels(50.0))
                .set_height(Pixels(50.0))
                .set_space(Stretch(1.0))
        });

        LevelsMeter::new().build(state, row, |builder| {
            builder
                .set_height(Pixels(200.0))
                .set_width(Pixels(50.0))
                .set_space(Stretch(1.0))
                .set_background_color(Color::rgb(50, 50, 50))
        });

        entity
    }
}

pub fn run() {
    // This function is temporary. Eventually we should use rusty-daw-io instead.
    let (sample_rate, max_audio_frames) = crate::rt_backend::default_sample_rate_and_buffer_size();

    let (frontend_state, rt_shared_state) = FrontendState::new(max_audio_frames, sample_rate);

    // This function is temporary. Eventually we should use rusty-daw-io instead.
    let _stream = crate::rt_backend::run_with_default_output(rt_shared_state);

    let window_description = WindowDescription::new().with_title("Meadowlark");
    let app = Application::new(window_description, |state, window| {
        state.add_theme(DEFAULT_THEME);
        state.add_theme(THEME);

        App::new(frontend_state).build(state, window, |builder| builder);
    });

    app.run();
}