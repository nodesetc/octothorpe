
pub struct CueKnob {
    delta: i8,
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum ButtonType {
    Grid(u8, u8),
    Side(u8),
    Indicator(u8),
    Track(u8),
    Activator(u8),
    Solo(u8),
    Arm(u8),
    Shift,
    Quantization,
    Play,
    Stop,
    Up,
    Down,
    Right,
    Left,
    Master(u8),
    Unknown,
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum FaderType {
    Track(u8),
    Velocity,
    CrossFade,
    Master,
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum KnobType {
    Effect(u8),
    Move(u8),
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum InputEventType {
    InquiryResponse(u8, u8),
    ButtonPressed(ButtonType),
    ButtonReleased(ButtonType),
    KnobTurned { value: u8, knob_type: KnobType },
    DeltaKnobTurned { delta: i8, knob_type: KnobType },
    FaderMoved { value: u8, fader_type: FaderType },
    Unknown,
}

#[derive(Debug)]
pub struct InputEvent {
    pub time: u32,
    pub event_type: InputEventType,
}

pub enum ControllerInput {
    APC40,
    APC20,
}

impl ControllerInput {
    pub fn message_to_input_event(&self, message: jack::RawMidi, button_offset_x: u8, button_offset_y: u8) -> InputEvent {
        InputEvent {
            time: message.time,
            event_type: self.bytes_to_input_event_type(message.bytes, button_offset_x, button_offset_y),
        }
    }

    fn button_type(&self, channel: u8, note: u8) -> ButtonType {
         match note {
            0x5B => ButtonType::Play,
            0x5C => ButtonType::Stop,
            0x33 => ButtonType::Track(channel),
            0x3F => ButtonType::Quantization,
            // These used to be sequence buttons, but will now be more control groups for plugin parameters
            //0x57 ..= 0x5A => ButtonType::Sequence(note - 0x57),
            // Side grid is turned upside down as we draw the phrases upside down as we draw notes
            // updside down due to lower midi nodes having lower numbers, therefore the 4 -
            0x52 ..= 0x56 => ButtonType::Side(4 - (note - 0x52)),
            0x51 => ButtonType::Shift,
            0x50 => {
                match self {
                    ControllerInput::APC20 => ButtonType::Master(0),
                    ControllerInput::APC40 => ButtonType::Master(1),
                }
            },
            // Grid should add notes & add phrases
            0x35 ..= 0x39 => ButtonType::Grid(channel, 4 - (note - 0x35)),
            0x5E => ButtonType::Up,
            0x5F => ButtonType::Down,
            0x60 => ButtonType::Right,
            0x61 => ButtonType::Left,
            0x62 => ButtonType::Shift,
            0x30 => ButtonType::Arm(channel),
            0x31 => ButtonType::Solo(channel),
            0x32 => ButtonType::Activator(channel),
            _ => ButtonType::Unknown,
        }
    }

    fn bytes_to_input_event_type(&self, bytes: &[u8], button_offset_x: u8, button_offset_y: u8) -> InputEventType {
        match bytes[0] {
            0xF0 => {
                // 0x06 = inquiry e, 0x02 = inquiry response 0x47 = akai manufacturer, 0x73 = APC40, 0x7b = APC20
                if bytes[3] == 0x06 && bytes[4] == 0x02 && bytes[5] == 0x47 && (bytes[6] == 0x73 || bytes[6] == 0x7b) {
                    InputEventType::InquiryResponse(bytes[13], bytes[6])
                } else {
                    InputEventType::Unknown
                }
            },
            0x90 ..= 0x9F => InputEventType::ButtonPressed(self.button_type(bytes[0] - 0x90 + button_offset_x, bytes[1] + button_offset_y)),
            0x80 ..= 0x8F => InputEventType::ButtonReleased(self.button_type(bytes[0] - 0x80 + button_offset_x, bytes[1] + button_offset_y)),
            0xB0 ..= 0xB8 => self.cc_to_input_event_type(bytes, button_offset_x, button_offset_y),
            _ => InputEventType::Unknown,
        }
    }

    fn cc_to_input_event_type(&self, bytes: &[u8], button_offset_x: u8, _offset_y: u8) -> InputEventType {
        match bytes[1] {
            0x30 ..= 0x37 | 0x10 ..= 0x17 => {
                // APC effect knobs are ordered weird, reorder them from to 0..16
                let modifier = if (0x30 ..= 0x37).contains(&bytes[1]) { 48 } else { 8 };
                let index = bytes[1] - modifier;

                InputEventType::KnobTurned { value: bytes[2], knob_type: KnobType::Effect(index) }
            },
            0x7 => InputEventType::FaderMoved { value: bytes[2], fader_type: FaderType::Track(bytes[0] - 0xB0 + button_offset_x) },
            0xE => {
                match self {
                    ControllerInput::APC20 => InputEventType::FaderMoved { value: bytes[2], fader_type: FaderType::Velocity },
                    ControllerInput::APC40 => InputEventType::FaderMoved { value: bytes[2], fader_type: FaderType::Master },
                }
            }
            0xF => InputEventType::FaderMoved { value: bytes[2], fader_type: FaderType::CrossFade },
            0x2F => {
                // Transform 0->up / 128->down to -delta / +delta
                let delta = (bytes[2] as i8).rotate_left(1) / 2;

                match self {
                    ControllerInput::APC20 => InputEventType::DeltaKnobTurned { delta, knob_type: KnobType::Move(0) },
                    ControllerInput::APC40 => InputEventType::DeltaKnobTurned { delta, knob_type: KnobType::Move(1) },
                }
            },
            _ => InputEventType::Unknown,
        }
    }
}

impl InputEvent {
    pub fn is_crossfader(event_type: &InputEventType) -> bool { 
        matches!(event_type, InputEventType::FaderMoved { fader_type: FaderType::CrossFade, .. }) 
    }

    pub fn is_activator_button(event_type: &InputEventType) -> bool { 
        matches!(event_type, InputEventType::ButtonPressed(ButtonType::Activator(_)))
    }

    pub fn is_track_button(event_type: &InputEventType) -> bool {
        matches!(event_type, InputEventType::ButtonPressed(ButtonType::Track(_)))
    }

    pub fn is_solo_button(event_type: &InputEventType) -> bool { 
        matches!(event_type, InputEventType::ButtonPressed(ButtonType::Solo(_)))
    }

    pub fn is_grid_button(event_type: &InputEventType) -> bool { 
        matches!(event_type, InputEventType::ButtonPressed(ButtonType::Grid(_, _)))
    }

    pub fn is_right_button(event_type: &InputEventType) -> bool { 
        matches!(event_type, InputEventType::ButtonPressed(ButtonType::Right))
    }

    pub fn is_left_button(event_type: &InputEventType) -> bool { 
        matches!(event_type, InputEventType::ButtonPressed(ButtonType::Left))
    }
}

