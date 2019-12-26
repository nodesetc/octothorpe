
use super::message::{TimedMessage, Message};
use super::cycle::Cycle;
use super::sequencer::Sequencer;
use super::handlers::{TimebaseHandler, MidiOut};

#[derive(Debug)]
struct PressedButton {
    start: u32,
    end: Option<u32>,
    channel: u8,
    note: u8,
}

impl PressedButton {
    pub fn new(start: u32, channel: u8, note: u8) -> Self {
        Self { start, end: None, channel, note }
    }
}

struct Buttons {
    pressed: Vec<PressedButton>,
}

impl Buttons {
    pub fn new() -> Self {
        Self { pressed: vec![] }
    }

    // We pressed a button!
    pub fn press(&mut self, start: u32, channel: u8, note: u8) -> bool {
        // Remove all keypresses that are not within double press range, while checking if this
        // key is double pressed wihtin short perioud
        let mut is_double_pressed = false;

        self.pressed.retain(|previous| {
            let falls_within_double_press_ticks = 
                previous.end.is_none() || start - previous.end.unwrap() < Controller::DOUBLE_PRESS_TICKS;

            let is_same_button = 
                previous.channel == channel && previous.note == note;

            // Ugly side effects, but i thought this to be cleaner as 2 iters looking for the same
            // thing
            is_double_pressed = falls_within_double_press_ticks && is_same_button;

            falls_within_double_press_ticks
        });

        // Save pressed_button to compare next pressed keys with, do this after comparing to not
        // compare with current press
        self.pressed.push(PressedButton::new(start, channel, note));

        is_double_pressed
    }

    pub fn release(&mut self, end: u32, channel: u8, note: u8) {
        let mut pressed_button = self.pressed.iter_mut().rev()
            .find(|pressed_button| {
                // press = 0x90, release = 0x80
                pressed_button.channel - 16 == channel && pressed_button.note == note
            })
            // We can safely unwrap as you can't press the same button twice
            .unwrap();

        pressed_button.end = Some(end);
    }
}

enum ControllerEvent {
    InquiryResponse { device_id: u8 },
    ButtonPressed { button_type: ButtonType },
    ButtonReleased { button_type: ButtonType },
    KnobTurned { value: u8, knob_type: KnobType },
    FaderMoved { time: u32, value: u8, fader_type: FaderType },
    Unknown,
}

enum ButtonType {
    Grid { x: u8, y: u8 },
    Playable { index: u8 },
    Indicator { index: u8 },
    Instrument { index: u8 },
    Activator { index: u8 },
    Solo { index: u8 },
    Arm { index: u8 },
    Sequence { index: u8 },
    Shift,
    Quantization,
    Play,
    Stop,
    Arrow { direction: Direction },
    Unknown,
}

impl ButtonType {
    fn new(channel: u8, note: u8) -> Self {
        match note {
            0x5B => ButtonType::Play,
            0x5C => ButtonType::Stop,
            0x33 => ButtonType::Instrument{ index: channel },
            0x3F => ButtonType::Quantization,
            0x57 ..= 0x5A => ButtonType::Sequence { index: note - 0x57 },
            // Playable grid
            0x52 ..= 0x56 => ButtonType::Playable { index: note - 0x52 },
            // Grid should add notes & add phrases
            0x35 ..= 0x39 => ButtonType::Grid { x: channel, y: note - 0x35 },
            0x5E => ButtonType::Arrow { direction: Direction::Up },
            0x5F => ButtonType::Arrow { direction: Direction::Down },
            0x60 => ButtonType::Arrow { direction: Direction::Right },
            0x61 => ButtonType::Arrow { direction: Direction::Left },
            0x30 => ButtonType::Arm { index: channel },
            0x31 => ButtonType::Solo { index: channel },
            0x32 => ButtonType::Activator { index: channel },
            _ => ButtonType::Unknown,
        }
    }
}

enum FaderType {
    Track { index: u8 },
    Master,
}

enum KnobType {
    Effect { time: u32, index: u8},
    Cue,
}

enum Direction {
    Up,
    Down,
    Right,
    Left,
}

impl ControllerEvent {
    fn new(time: u32, bytes: &[u8]) -> Self {
        match bytes[0] {
            0xF0 => {
                // Is this inquiry response
                if bytes[3] == 0x06 && bytes[4] == 0x02  
                    && bytes[5] == 0x47 && bytes[6] == 0x73 
                {
                    Self::InquiryResponse { device_id: bytes[13] }
                } else {
                    Self::Unknown
                }
            },
            0x90 ..= 0x9F => Self::ButtonPressed { button_type: ButtonType::new(bytes[0] - 0x90, bytes[1]) },
            0x80 ..= 0x8F => Self::ButtonReleased { button_type: ButtonType::new(bytes[0] - 0x80, bytes[1]) },
            0xB0 ..= 0xB8 => {
                match bytes[1] {
                    0x30 ..= 0x37 | 0x10 ..= 0x17 => {
                        // APC effect knobs are ordered weird, reorder them from to 0..16
                        let modifier = if (0x30 ..= 0x37).contains(&bytes[1]) { 48 } else { 8 };
                        let index = bytes[1] - modifier;

                        Self::KnobTurned { value: bytes[2], knob_type: KnobType::Effect { time, index } }
                    },
                    0x7 => Self::FaderMoved { 
                        time, 
                        value: bytes[2],
                        fader_type: FaderType::Track { index: bytes[0] - 0xB0 } 
                    },
                    0xE => Self::FaderMoved { time, value: bytes[2], fader_type: FaderType::Master },
                    0x2F => Self::KnobTurned { value: bytes[2], knob_type: KnobType::Cue },
                    _ => Self::Unknown,
                }
            },
            _ => Self::Unknown,
        }
    }
}

pub struct Controller {
    buttons: Buttons,

    // Ports that connect to APC
    input: jack::Port<jack::MidiIn>,
    output: MidiOut,

    is_identified: bool,
}

impl Controller {
    const DOUBLE_PRESS_TICKS: u32 = TimebaseHandler::TICKS_PER_BEAT / 2;

    pub fn new(client: &jack::Client) -> Self {
        let input = client.register_port("APC40 in", jack::MidiIn::default()).unwrap();
        let output = client.register_port("APC40 out", jack::MidiOut::default()).unwrap();
        
        Controller {
            buttons: Buttons::new(),

            input,
            output: MidiOut::new(output),

            is_identified: false,
        }
    }

    /*
     * Process input & output from controller jackports
     */
    pub fn process(&mut self, client: &jack::Client, process_scope: &jack::ProcessScope, absolute_start: u32, sequencer: &mut Sequencer) {
        for message in self.input.iter(process_scope) {
            let controller_event = ControllerEvent::new(message.time, message.bytes);

            //println!("0x{:X}, 0x{:X}, 0x{:X}", message.bytes[0], message.bytes[1], message.bytes[2]);
            // Only process channel note messages
            match message.bytes[0] {
                0xF0 => {
                    // Is this inquiry response
                    if message.bytes[3] == 0x06 && message.bytes[4] == 0x02  
                        && message.bytes[5] == 0x47 && message.bytes[6] == 0x73 
                    {
                        // Introduce ourselves to controller
                        // 0x41 after 0x04 is ableton mode (only led rings are not controlled by host, but can be set.)
                        // 0x42 is ableton alternate mode (all leds controlled from host)
                        let message = Message::Introduction([0xF0, 0x47, message.bytes[13], 0x73, 0x60, 0x00, 0x04, 0x41, 0x00, 0x00, 0x00, 0xF7]);
                        // Make sure we stop inquiring
                        self.is_identified = true;

                        self.output.output_message(TimedMessage::new(0, message));
                    }
                },
                0xB0 => {
                    if message.bytes[1] == 0x2F {
                        sequencer.cue_knob_turned(message.bytes[2]);
                    }
                },
                0x90 ..= 0x9F => {
                    // Rememberrr
                    let press_tick = absolute_start + message.time;
                    let is_double_pressed = self.buttons.press(press_tick, message.bytes[0], message.bytes[1]);

                    match message.bytes[1] {
                        0x5B => { client.transport_start() },
                        0x5C => {
                            let (state, _) = client.transport_query();
                            match state {
                                1 => client.transport_stop(),
                                _ => client.transport_reposition(jack::Position::default()),
                            };
                        },
                        _ => {
                            // Always single press ?
                            //sequencer.key_pressed(message);
                            /*
                             * Next up is double press & single presss logic
                             * TODO - Add grid multi key range support here
                             */

                            // Double pressed_button when its there
                            if is_double_pressed && (0x52 ..= 0x56).contains(&message.bytes[1]) && sequencer.is_showing_pattern() {
                                let pattern_index = (message.bytes[1] - 0x52) as usize;
                                sequencer.instrument().patterns[pattern_index].switch_recording_state()
                            }
                        }
                    }

                },
                0x80 ..= 0x8F => {
                    let release_tick = absolute_start + message.time;
                    self.buttons.release(release_tick, message.bytes[0], message.bytes[1]);
                },
                0xB0 ..= 0xB8 => {
                    match message.bytes[1] {
                        // APC knobs are ordered weird, reorder them from to 0..16
                        0x10 ..= 0x17 => sequencer.knob_turned(message.time, message.bytes[1] - 8, message.bytes[2]),
                        0x30 ..= 0x37 => sequencer.knob_turned(message.time, message.bytes[1] - 48, message.bytes[2]),
                        0x7 => sequencer.fader_adjusted(message.time, message.bytes[0] - 0xB0, message.bytes[2]),
                        0xE => sequencer.master_adjusted(message.time, message.bytes[2]),
                        _ => (),
                    }
                },
                _ => (),
            }
        }

        // Identify when no controller found yet
        if ! self.is_identified {
            self.output.output_message(TimedMessage::new(0, Message::Inquiry([0xF0, 0x7E, 0x00, 0x06, 0x01, 0xF7])));
        }

        self.output.write_midi(process_scope);
    }

    /*
    // Process messages from APC controller keys being pushed
    pub fn process_sysex_input<'a, I>(&mut self, input: I, cycle: &Cycle, client: &jack::Client) -> Vec<TimedMessage>
        where
            I: Iterator<Item = jack::RawMidi<'a>>,
    {
        input
            .filter_map(|message| {
                //println!("0x{:X}, 0x{:X}, 0x{:X}", message.bytes[0], message.bytes[1], message.bytes[2]);
                // 0x06 = inquiry e, 0x02 = inquiry response
                // 0x47 = akai manufacturer, 0x73 = model nr
                if message.bytes[0] == 0xF0 &&
                    message.bytes[3] == 0x06 && message.bytes[4] == 0x02  
                    && message.bytes[5] == 0x47 && message.bytes[6] == 0x73 
                {
                    // Introduce ourselves to controller
                    // 0x41 after 0x04 is ableton mode (only led rings are not controlled by host, but can be set.)
                    // 0x42 is ableton alternate mode (all leds controlled from host)
                    let message = Message::Introduction([0xF0, 0x47, message.bytes[13], 0x73, 0x60, 0x00, 0x04, 0x41, 0x00, 0x00, 0x00, 0xF7]);
                    let introduction = TimedMessage::new(0, message);

                    // Rerender & draw what we want to see
                    self.sequencer.reset();
                    let mut messages = vec![introduction];
                    // TODO - Before we timed the messages after introduction to 128 frames, why?
                    messages.extend(self.sequencer.output_static(true));

                    Some(messages)
                } else {
                    None
                }
            })
            .flatten()
            .collect()
    }

    // Process messages from APC controller keys being pushed
    pub fn process_apc_note_messages<'a, I>(&mut self, input: I, cycle: &Cycle, client: &jack::Client)
        where
            I: Iterator<Item = jack::RawMidi<'a>>,
    {
        input
            .for_each(|message| {
                //println!("0x{:X}, 0x{:X}, 0x{:X}", message.bytes[0], message.bytes[1], message.bytes[2]);
                // Only process channel note messages
                match message.bytes[0] {
                    0xB0 => {
                        if message.bytes[1] == 0x2F {
                            self.sequencer.cue_knob_turned(message.bytes[2]);
                        }
                    },
                    0x90 ..= 0x9F => {
                        let pressed_key = PressedKey { 
                            time: cycle.absolute_start + message.time, 
                            channel: message.bytes[0],
                            key: message.bytes[1],
                        };

                        // Remove keypresses that are not within double press range
                        self.pressed_keys.retain(|previous| {
                            pressed_key.time - previous.time < Controller::DOUBLE_PRESS_TICKS
                        });

                        // Check for old keypresses matching currently pressed key
                        let double_presses: Vec<bool> = self.pressed_keys.iter()
                            .filter_map(|previous| {
                                if previous.channel == pressed_key.channel && previous.key == pressed_key.key {
                                    Some(true)
                                } else {
                                    None
                                }
                            })
                            .collect();

                        // Always single press 
                        match message.bytes[1] {
                            0x5B => { client.transport_start() },
                            0x5C => {
                                let (state, _) = client.transport_query();
                                match state {
                                    1 => client.transport_stop(),
                                    _ => client.transport_reposition(jack::Position::default()),
                                };
                            },
                            _ => self.sequencer.key_pressed(message),
                        }

                        // Double pressed_key when its there
                        if double_presses.len() > 0 {
                            self.sequencer.key_double_pressed(message);
                        }

                        // Save pressed_key
                        self.pressed_keys.push(pressed_key);

                    },
                    0x80 ..= 0x8F => self.sequencer.key_released(message),
                    _ => (),
                }
            })
    }

    // Process messages from APC controller keys being pushed
    pub fn process_apc_control_change_messages<'a, I>(&mut self, input: I) -> Vec<TimedMessage>
        where
            I: Iterator<Item = jack::RawMidi<'a>>,
    {
        input
            .filter_map(|message| {
                //println!("0x{:X}, 0x{:X}, 0x{:X}", message.bytes[0], message.bytes[1], message.bytes[2]);
                // Only process channel note messages
                match message.bytes[0] {
                    0xB0 ..= 0xB8 => {
                        match message.bytes[1] {
                            // APC knobs are ordered weird, reorder them from to 0..16
                            0x10..=0x17 => Some(self.sequencer.knob_turned(message.time, message.bytes[1] - 8, message.bytes[2])),
                            0x30..=0x37 => Some(self.sequencer.knob_turned(message.time, message.bytes[1] - 48, message.bytes[2])),
                            0x7 => Some(self.sequencer.fader_adjusted(message.time, message.bytes[0] - 0xB0, message.bytes[2])),
                            0xE => Some(self.sequencer.master_adjusted(message.time, message.bytes[2])),
                            _ => None,
                        }
                    },
                    _ => None,
                }
            })
            .flatten()
            .collect()
    }
    */

        /*
    // Process incoming control change messages from plugins of which parameters were changed
    pub fn process_plugin_control_change_messages<'a, I>(&mut self, input: I) -> Vec<TimedMessage>
        where
            I: Iterator<Item = jack::RawMidi<'a>>,
    {
        input
            .filter_map(|message| {
                // Only process channel note messages
                match message.bytes[0] {
                    0xB0..=0xBF => self.sequencer.plugin_parameter_changed(message),
                    _ => None,
                }
            })
            .collect()
    }

    pub fn process_instrument_messages<'a, I>(&mut self, cycle: &Cycle, input: I) -> Vec<TimedMessage>
        where
            I: Iterator<Item = jack::RawMidi<'a>>,
    {
        input
            .filter_map(|message| {
                let option = match message.bytes[0] {
                    0x90 | 0x80 => Some((self.sequencer.keyboard_target, 0)),
                    0x99 | 0x89 => Some((self.sequencer.drumpad_target, 9)),
                    _ => None,
                };

                // Only process channel note messages
                if let Some((index, offset)) = option {
                    Some(self.sequencer.recording_key_played(index + self.sequencer.instrument_group * 8, message.bytes[0] - offset, cycle, message))
                } else {
                    None
                }
            })
            .collect()
    }
    */
}

