
use super::super::handlers::TimebaseHandler;

#[derive(Debug)]
struct ButtonPress {
    start: u64,
    end: Option<u64>,
    //button_type: ButtonType,
    channel: u8,
    note: u8,
}

impl ButtonPress {
    pub fn new(start: u64, channel: u8, note: u8) -> Self {
        Self { start, end: None, channel, note }
    }
}

pub struct Memory {
    presses: Vec<ButtonPress>,
}

#[derive(Debug)]
pub enum ButtonType {
    Grid { x: u8, y: u8 },
    Playable(u8),
    Indicator(u8),
    Instrument(u8),
    Activator(u8),
    Solo(u8),
    Arm(u8),
    Sequence(u8),
    Shift,
    Quantization,
    Play,
    Stop,
    Up,
    Down,
    Right,
    Left,
    Unknown,
}

pub enum FaderType {
    Track(u8),
    Master,
}

pub enum KnobType {
    Effect { time: u32, index: u8},
    Cue,
}

pub enum Event {
    InquiryResponse(u8),
    ButtonPressed(ButtonType),
    ButtonReleased(ButtonType),
    KnobTurned { value: u8, knob_type: KnobType },
    FaderMoved { time: u32, value: u8, fader_type: FaderType },
    Unknown,
}


impl ButtonType {
    fn new(channel: u8, note: u8) -> Self {
        match note {
            0x5B => ButtonType::Play,
            0x5C => ButtonType::Stop,
            0x33 => ButtonType::Instrument(channel),
            0x3F => ButtonType::Quantization,
            0x57 ..= 0x5A => ButtonType::Sequence(note - 0x57),
            // Playable grid
            0x52 ..= 0x56 => ButtonType::Playable(note - 0x52),
            // Grid should add notes & add phrases
            0x35 ..= 0x39 => ButtonType::Grid { x: channel, y: note - 0x35 },
            0x5E => ButtonType::Up,
            0x5F => ButtonType::Down,
            0x60 => ButtonType::Right,
            0x61 => ButtonType::Left,
            0x30 => ButtonType::Arm(channel),
            0x31 => ButtonType::Solo(channel),
            0x32 => ButtonType::Activator(channel),
            _ => ButtonType::Unknown,
        }
    }
}

impl Event {
    pub fn new(time: u32, bytes: &[u8]) -> Self {
        match bytes[0] {
            0xF0 => {
                // 0x06 = inquiry e, 0x02 = inquiry response 0x47 = akai manufacturer, 0x73 = model nr
                if bytes[3] == 0x06 && bytes[4] == 0x02 && bytes[5] == 0x47 && bytes[6] == 0x73 {
                    Self::InquiryResponse(bytes[13])
                } else {
                    Self::Unknown
                }
            },
            0x90 ..= 0x9F => Self::ButtonPressed(ButtonType::new(bytes[0] - 0x90, bytes[1])),
            0x80 ..= 0x8F => Self::ButtonReleased(ButtonType::new(bytes[0] - 0x80, bytes[1])),
            0xB0 ..= 0xB8 => {
                match bytes[1] {
                    0x30 ..= 0x37 | 0x10 ..= 0x17 => {
                        // APC effect knobs are ordered weird, reorder them from to 0..16
                        let modifier = if (0x30 ..= 0x37).contains(&bytes[1]) { 48 } else { 8 };
                        let index = bytes[1] - modifier;

                        Self::KnobTurned { value: bytes[2], knob_type: KnobType::Effect { time, index } }
                    },
                    0x7 => Self::FaderMoved { time, value: bytes[2], fader_type: FaderType::Track(bytes[0] - 0xB0) },
                    0xE => Self::FaderMoved { time, value: bytes[2], fader_type: FaderType::Master },
                    0x2F => Self::KnobTurned { value: bytes[2], knob_type: KnobType::Cue },
                    _ => Self::Unknown,
                }
            },
            _ => Self::Unknown,
        }
    }
}

impl Memory {
    const DOUBLE_PRESS_USECS: u64 = 300000;

    pub fn new() -> Self {
        Self { presses: vec![] }
    }

    // We pressed a button!
    pub fn press(&mut self, start: u64, channel: u8, note: u8) -> bool {
        // Remove all keypresses that are not within double press range, while checking if this
        // key is double pressed wihtin short perioud
        let mut is_double_pressed = false;

        self.presses.retain(|previous| {
            let falls_within_double_press_ticks = 
                previous.end.is_none() || start - previous.end.unwrap() < Memory::DOUBLE_PRESS_USECS;

            let is_same_button = 
                previous.channel == channel && previous.note == note;

            // Ugly side effects, but i thought this to be cleaner as 2 iters looking for the same
            // thing
            is_double_pressed = falls_within_double_press_ticks && is_same_button;

            falls_within_double_press_ticks
        });

        // Save pressed_button to compare next pressed keys with, do this after comparing to not
        // compare with current press
        self.presses.push(ButtonPress::new(start, channel, note));

        is_double_pressed
    }

    pub fn release(&mut self, end: u64, channel: u8, note: u8) {
        let mut pressed_button = self.presses.iter_mut().rev()
            .find(|pressed_button| {
                // press = 0x90, release = 0x80
                pressed_button.channel - 16 == channel && pressed_button.note == note
            })
            // We can safely unwrap as you can't press the same button twice
            .unwrap();

        pressed_button.end = Some(end);
    }
}


