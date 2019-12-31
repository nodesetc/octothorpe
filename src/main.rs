
//#![feature(drain_filter)]
extern crate jack;
use jack_sys as j;

pub mod controller;
pub mod message;
pub mod sequencer;
pub mod cycle;
pub mod instrument;
pub mod loopable;
pub mod sequence;
pub mod surface;
pub mod port;
pub mod mixer;
pub mod events;

use std::io;
use std::sync::mpsc::channel;
use std::sync::mpsc::{Sender, Receiver};
use sequencer::Sequencer;
use controller::*;
use mixer::*;
use surface::Surface;
use message::{TimedMessage, Message};
use cycle::{ProcessCycle, Cycle};

pub struct TimebaseHandler {
    beats_per_minute: f64,
    beats_per_bar: isize,
    beat_type: isize,
    is_up_to_date: bool,

    receiver: Receiver<f64>,
}

impl TimebaseHandler {
    pub const TICKS_PER_BEAT: u32 = 1920;
    const BEATS_PER_BAR: u32 = 4;

    pub fn beats_to_ticks(beats: f64) -> u32 {
        (beats * Self::TICKS_PER_BEAT as f64) as u32
    }

    pub fn bars_to_beats(bars: u32) -> u32 {
        bars * Self::BEATS_PER_BAR
    }

    pub fn bars_to_ticks(bars: u32) -> u32 {
        Self::bars_to_beats(bars) * Self::TICKS_PER_BEAT
    }

    pub fn new(receiver: Receiver<f64>) -> Self {
        TimebaseHandler {
            beats_per_minute: 156.0,
            is_up_to_date: false,
            beats_per_bar: 4,
            beat_type: 4,
            receiver,
        }
    }
}

impl jack::TimebaseHandler for TimebaseHandler {
    fn timebase(&mut self, _: &jack::Client, _state: jack::TransportState, _n_frames: jack::Frames, pos: *mut jack::Position, is_new_pos: bool) {
        unsafe {
            // Set position type
            (*pos).valid = j::JackPositionBBT;

            // BPM changed?
            //if ! is_new_pos && (*pos).beats_per_minute != self.beats_per_minute {
                //println!("{:?}", (*pos).beats_per_minute);
            //}

            // Only update timebase when we are asked for it, or when our state changed
            if is_new_pos || ! self.is_up_to_date {
                (*pos).beats_per_bar = self.beats_per_bar as f32;
                (*pos).ticks_per_beat = Self::TICKS_PER_BEAT as f64;
                (*pos).beat_type = self.beat_type as f32;
                (*pos).beats_per_minute = self.beats_per_minute;

                self.is_up_to_date = true;
            }

            let second = (*pos).frame as f64 / (*pos).frame_rate as f64;
            let abs_tick = second / 60.0 * self.beats_per_minute * Self::TICKS_PER_BEAT as f64;
            let abs_beat = abs_tick / (*pos).ticks_per_beat;

            // Plus 1 as humans tend not to count from 0
            (*pos).bar = (abs_beat / (*pos).beats_per_bar as f64) as i32 + 1;
            (*pos).beat = (abs_beat % (*pos).beats_per_bar as f64) as i32 + 1;
            (*pos).bar_start_tick = (abs_beat as i32 * (*pos).ticks_per_beat as i32) as f64;
            (*pos).tick = abs_tick as i32 - (*pos).bar_start_tick as i32;
        }
    }
}


pub struct ProcessHandler {
    // Controllers
    apc40: APC40,
    apc20: APC20,

    mixer: Mixer,
    sequencer: Sequencer,
    surface: Surface,

    //ticks_elapsed: u32,
    //was_repositioned: bool,

    // Port that receives updates from plugin host about parameters changing
    //control_in: jack::Port<jack::MidiIn>,
    //control_out: MidiOut,

    // Sequencer out & cc out etc.
    //sequence_in: jack::Port<jack::MidiIn>,
    //sequence_out: MidiOut,
}

impl ProcessHandler {
    pub fn new(
        timebase_sender: Sender<f64>,
        client: &jack::Client
    ) -> Self {
        // Create ports
        //let apc_40_in = client.register_port("APC40 in", jack::MidiIn::default()).unwrap();
        //let apc_40_out = client.register_port("APC40 out", jack::MidiOut::default()).unwrap();
        //let apc_20_in = client.register_port("APC20 in", jack::MidiIn::default()).unwrap();
        //let apc_20_out = client.register_port("APC20 out", jack::MidiOut::default()).unwrap();
        //let control_in = client.register_port("control in", jack::MidiIn::default()).unwrap();
        //let control_out = client.register_port("control out", jack::MidiOut::default()).unwrap();
        //let sequence_in = client.register_port("sequence in", jack::MidiIn::default()).unwrap();
        //let sequence_out = client.register_port("sequence out", jack::MidiOut::default()).unwrap();

        // TODO controller should be trait for apc20 & 40

        ProcessHandler { 
            apc20: APC20::new(client),
            apc40: APC40::new(client),

            mixer: Mixer::new(),
            sequencer: Sequencer::new(client), 
            surface: Surface::new(),
            //ticks_elapsed: 0,
            //was_repositioned: false,
            //control_in,
            //control_out: MidiOut{ port: control_out },
            //sequence_in,
            //sequence_out: MidiOut{ port: sequence_out },
        }
    }
}

impl jack::ProcessHandler for ProcessHandler {
    fn process(&mut self, client: &jack::Client, scope: &jack::ProcessScope) -> jack::Control {
        // Get something representing this process cycle
        //let (state, pos) = client.transport_query();
        //let cycle = Cycle::new(pos, self.ticks_elapsed, self.was_repositioned, process_scope.n_frames(), state);
        // Update next ticks to keep track of absoulute ticks elapsed for note off events
        //self.ticks_elapsed += cycle.ticks;
        // cycle.absolute_start indicates this is first cycle program runs for
        //self.was_repositioned = cycle.is_repositioned || cycle.absolute_start == 0;

        let cycle = ProcessCycle { scope, client };

        self.apc20.process_input(&cycle, &mut self.sequencer, &mut self.surface, &mut self.mixer);
        self.apc40.process_input(&cycle, &mut self.sequencer, &mut self.surface, &mut self.mixer);

        self.apc20.output(&cycle, &mut self.sequencer, &mut self.surface);
        self.apc40.output(&cycle, &mut self.sequencer, &mut self.surface);

        //let mut apc_messages = vec![];
        //let mut control_messages = vec![];

        // TODO - Clean up this mess

        // Process incoming midi notes from APC (these correspond to button presses)
        //apc_messages.extend(self.controller.process_apc_note_messages(self.apc_40_in.iter(process_scope), &cycle, client));
        //apc_messages.extend(self.controller.process_apc_note_messages(self.apc_20_in.iter(process_scope), &cycle, client));
        //apc_messages.extend(self.controller.process_plugin_control_change_messages(self.control_in.iter(process_scope)));

        // Process incoming control change messages from APC (knob turns etc.), output adjusted cc
        // messages on seperate CC messages channel so cc messages are not picked up by synths etc.
        //control_messages.extend(self.controller.process_apc_control_change_messages(self.apc_40_in.iter(process_scope)));
        //control_messages.extend(self.controller.process_apc_control_change_messages(self.apc_20_in.iter(process_scope)));

        // Get dynamic grids (indicators and whatnot) & midi messages
        // These are both returned by one function as playing notes will also be used for
        // sequence indicators
        //let (dynamic_grid_messages, mut sequencer_messages) = self.controller.sequencer.output_midi(&cycle);
        //apc_messages.extend(dynamic_grid_messages);

        //sequencer_messages.extend(self.controller.process_instrument_messages(&cycle, self.sequence_in.iter(process_scope)));

        // Draw all the grids that don't change much & output control knob values
        //let (messages, _) = self.sequence_in.iter(process_scope).size_hint();
        //apc_messages.extend(self.controller.sequencer.output_static(messages > 0));

        // Get cycle based control & midi
        //self.apc_40_out.write(process_scope, apc_messages);
        //self.control_out.write(process_scope, control_messages);
        //self.sequence_out.write(process_scope, sequencer_messages);

        jack::Control::Continue
    }
}


fn main() {
    // Setup client
    let (client, _status) =
        jack::Client::new("Octothorpe", jack::ClientOptions::NO_START_SERVER).unwrap();

    let (timebase_sender, timebase_receiver) = channel();

    let processhandler = ProcessHandler::new(timebase_sender, &client);
    let timebasehandler = TimebaseHandler::new(timebase_receiver);

    // Activate client
    let _async_client = client
        .activate_async((), processhandler, timebasehandler)
        .unwrap();

    // Wait for user to input string
    let mut user_input = String::new();
    io::stdin().read_line(&mut user_input).ok();
}

