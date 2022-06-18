//! # Program (State) Layer
//!
//! This layer owns the state of the program.
//!
//! It is solely in charge of mutating this state. The backend layer and the UI
//! layer cannot mutate this state directly (with the exception of some
//! UI-specific state that does not need to be undo-able such as panel or window
//! size). The backend layer indirectly mutates this state by sending events to
//! the program layer, and the ui layer indirectly mutates this state by calling
//! methods on the ProgramState struct which the UI layer owns.
//!
//! The program layer also owns the handle to the audio thread and is in charge
//! of connecting to the system's audio and MIDI devices. It is also in charge
//! of some offline DSP such as resampling audio clips.

pub mod program_state;

use std::error::Error;

use cpal::traits::{DeviceTrait, HostTrait};
use crossbeam::channel::Receiver;
use dropseed::{
    ActivateEngineSettings, DSEngineAudioThread, DSEngineEvent, DSEngineHandle, DSEngineRequest,
    EngineDeactivatedInfo, HostInfo, PluginEvent, PluginScannerEvent,
};
use meadowlark_core_types::{MusicalTime, SampleRate};
pub use program_state::ProgramState;
use rtrb::RingBuffer;

use self::program_state::{ChannelRackOrientation, PanelState, TimelineGridState};
use vizia::prelude::*;

#[derive(Debug)]
enum UIToAudioThreadMsg {
    NewEngineAudioThread(DSEngineAudioThread),
    DropEngineAudioThread,
}

/// This is in charge of keeping track of state for the whole program.
///
/// The UI must continually call `ProgramLayer::poll()` (on every frame or an
/// equivalent timer).
#[derive(Lens)]
pub struct ProgramLayer {
    /// The state of the whole program.
    ///
    /// Unless explicitely stated, the UI may NOT directly mutate the state of any
    /// of these variables. It is intended for the UI to call the methods on this
    /// struct in order to mutate state.
    pub state: ProgramState,

    #[lens(ignore)]
    engine_handle: DSEngineHandle,
    #[lens(ignore)]
    engine_rx: Receiver<DSEngineEvent>,
    #[lens(ignore)]
    to_audio_thread_tx: rtrb::Producer<UIToAudioThreadMsg>,
    #[lens(ignore)]
    _cpal_stream: Option<cpal::Stream>,
}

impl ProgramLayer {
    // Create some dummy state for now
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let (to_audio_thread_tx, mut from_gui_rx) = RingBuffer::<UIToAudioThreadMsg>::new(16);

        // Note, using cpal is temporary. We will switch to rainout once it's ready.
        // ---  Initialize cpal stream  -----------------------------------------------

        let cpal_host = cpal::default_host();

        let device = cpal_host
            .default_output_device()
            .ok_or("CPAL error: no output device available".to_string())?;

        let config = device.default_output_config()?;

        let num_out_channels = usize::from(config.channels());
        let sample_rate: SampleRate = config.sample_rate().0.into();

        let mut engine_audio_thread: Option<DSEngineAudioThread> = None;

        let cpal_stream = device.build_output_stream(
            &config.into(),
            move |audio_buffer: &mut [f32], _: &cpal::OutputCallbackInfo| {
                while let Ok(msg) = from_gui_rx.pop() {
                    match msg {
                        UIToAudioThreadMsg::NewEngineAudioThread(new_audio_thread) => {
                            engine_audio_thread = Some(new_audio_thread);
                        }
                        UIToAudioThreadMsg::DropEngineAudioThread => {
                            engine_audio_thread = None;
                        }
                    }
                }

                if let Some(engine_audio_thread) = &mut engine_audio_thread {
                    engine_audio_thread
                        .process_cpal_interleaved_output_only(num_out_channels, audio_buffer);
                }
            },
            |e| {
                panic!("{}", e);
            },
        )?;

        // ---  Initialize Dropseed Engine  -------------------------------------------

        let (mut engine_handle, engine_rx) = DSEngineHandle::new(
            HostInfo::new(
                String::from("Meadowlark"),
                env!("CARGO_PKG_VERSION").into(),
                Some("Meadowlark".into()),
                Some("https://meadowlark.app".into()),
            ),
            vec![],
        );

        log::debug!("{:?}", &engine_handle.internal_plugins_res);

        engine_handle.send(DSEngineRequest::ActivateEngine(Box::new(ActivateEngineSettings {
            sample_rate,
            min_frames: 1,
            max_frames: crate::MAX_BLOCK_SIZE as u32,
            num_audio_in_channels: 0,
            num_audio_out_channels: 2,
            ..Default::default()
        })));

        engine_handle.send(DSEngineRequest::RescanPluginDirectories);

        // ----------------------------------------------------------------------------

        Ok(ProgramLayer {
            state: ProgramState {
                engine_running: false,
                notification_log: Vec::new(),
                tracks: Vec::new(),
                timeline_grid: TimelineGridState {
                    horizontal_zoom_level: 0.0,
                    vertical_zoom_level: 0.0,
                    left_start: MusicalTime::from_beats(0),
                    top_start: 0.0,
                    lane_height: 1.0,
                    lanes: Vec::new(),
                    project_length: MusicalTime::from_beats(4),
                    used_lanes: 0,
                },
                panels: PanelState {
                    channel_rack_orientation: ChannelRackOrientation::Horizontal,
                    hide_patterns: false,
                    hide_piano_roll: false,
                    browser_width: 100.0,
                    show_browser: false,
                },
            },
            engine_handle,
            engine_rx,
            to_audio_thread_tx,
            _cpal_stream: Some(cpal_stream),
        })
    }

    pub fn poll_engine(&mut self) {
        for msg in self.engine_rx.try_iter() {
            //dbg!(&msg);

            match msg {
                // Sent whenever the engine is deactivated.
                //
                // The DSEngineAudioThread sent in a previous EngineActivated event is now
                // invalidated. Please drop it and wait for a new EngineActivated event to
                // replace it.
                //
                // To keep using the audio graph, you must reactivate the engine with
                // `DSEngineRequest::ActivateEngine`, and then restore the audio graph
                // from an existing save state if you wish using
                // `DSEngineRequest::RestoreFromSaveState`.
                DSEngineEvent::EngineDeactivated(res) => {
                    self.to_audio_thread_tx
                        .push(UIToAudioThreadMsg::DropEngineAudioThread)
                        .unwrap();

                    match res {
                        // The engine was deactivated gracefully after recieving a
                        // `DSEngineRequest::DeactivateEngine` request.
                        EngineDeactivatedInfo::DeactivatedGracefully { recovered_save_state } => {
                            log::info!("Engine deactivated gracefully");
                        }
                        // The engine has crashed.
                        EngineDeactivatedInfo::EngineCrashed {
                            error_msg,
                            recovered_save_state,
                        } => {
                            log::error!("Engine crashed: {}", error_msg);
                        }
                    }
                }

                // This message is sent whenever the engine successfully activates.
                DSEngineEvent::EngineActivated(info) => {
                    self.to_audio_thread_tx
                        .push(UIToAudioThreadMsg::NewEngineAudioThread(info.audio_thread))
                        .unwrap();
                }

                // When this message is received, it means that the audio graph is starting
                // the process of restoring from a save state.
                //
                // Reset your UI as if you are loading up a project for the first time, and
                // wait for the `AudioGraphModified` event to repopulate the UI.
                //
                // If the audio graph is in an invalid state as a result of restoring from
                // the save state, then the `EngineDeactivated` event will be sent instead.
                DSEngineEvent::AudioGraphCleared => {}

                // This message is sent whenever the audio graph has been modified.
                //
                // Be sure to update your UI from this new state.
                DSEngineEvent::AudioGraphModified(mut res) => {}

                DSEngineEvent::Plugin(event) => match event {
                    // Sent whenever a plugin becomes activated after being deactivated or
                    // when the plugin restarts.
                    //
                    // Make sure your UI updates the port configuration on this plugin.
                    PluginEvent::Activated { plugin_id, new_handle, new_param_values } => {}

                    // Sent whenever a plugin becomes deactivated. When a plugin is deactivated
                    // you cannot access any of its methods until it is reactivated.
                    PluginEvent::Deactivated {
                        plugin_id,
                        // If this is `Ok(())`, then it means the plugin was gracefully
                        // deactivated from user request.
                        //
                        // If this is `Err(e)`, then it means the plugin became deactivated
                        // because it failed to restart.
                        status,
                    } => {}

                    PluginEvent::ParamsModified { plugin_id, modified_params } => {}

                    unkown_event => {
                        log::warn!("{:?}", unkown_event);
                    }
                },

                DSEngineEvent::PluginScanner(event) => match event {
                    // A new CLAP plugin scan path was added.
                    PluginScannerEvent::ClapScanPathAdded(path) => {}
                    // A CLAP plugin scan path was removed.
                    PluginScannerEvent::ClapScanPathRemoved(path) => {}
                    // A request to rescan all plugin directories has finished. Update
                    // the list of available plugins in your UI.
                    PluginScannerEvent::RescanFinished(mut info) => {}
                    unkown_event => {
                        log::warn!("{:?}", unkown_event);
                    }
                },
                unkown_event => {
                    log::warn!("{:?}", unkown_event);
                }
            }
        }
    }
}

pub enum ProgramEvent {
    SaveProject,
    LoadProject,
}

impl Model for ProgramLayer {
    // Update the program layer here
    fn event(&mut self, cx: &mut Context, event: &mut Event) {
        event.map(|program_event, meta| match program_event {
            ProgramEvent::SaveProject => {
                let save_state = serde_json::to_string(&self.state).unwrap();
                std::fs::write("project.json", save_state).unwrap();
            }

            ProgramEvent::LoadProject => {
                let save_state = std::fs::read_to_string("project.json").unwrap();
                let project_state = serde_json::from_str(&save_state).unwrap();
                self.state = project_state;
            }
        });

        self.state.event(cx, event);
    }
}
