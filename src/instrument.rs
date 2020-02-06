
use std::ops::Range;
use super::port::*;
use super::loopable::*;
use super::cycle::*;
use super::events::*;
use super::message::*;

pub struct Instrument {
    // TODO - these are public as we're testing with premade patterns
    pub patterns: [Pattern; 5],
    pub phrases: [Phrase; 5],

    playing_notes: Vec<PlayingNoteEvent>,

    //pub knob_group: u8,
    //knob_values: [u8; 128],

    output: MidiOut,
}

impl Instrument {
    pub fn new(client: &jack::Client, id: u8) -> Self {
        let patterns = [Pattern::new(), Pattern::new(), Pattern::new(), Pattern::new(), Pattern::new()];
        let phrases = [Phrase::new(), Phrase::new(), Phrase::new(), Phrase::new(), Phrase::new()];

        //let input = client.register_port("APC20 in", jack::MidiIn::default()).unwrap();
        let output = client.register_port(format!("Instrument {}", id).as_str(), jack::MidiOut::default()).unwrap();

        Instrument {
            phrases,
            patterns,

            playing_notes: vec![],

            // There's 4 knob groups, this way we can have knobs * 4 !
            //knob_group: 0,
            //knob_values: [0; 128],

            output: MidiOut::new(output),
        }
    }

    pub fn get_pattern(&mut self, index: u8) -> &mut Pattern {
        &mut self.patterns[index as usize]
    }

    pub fn phrase(&self, index: u8) -> &Phrase { &self.phrases[index as usize] }
    pub fn phrase_mut(&mut self, index: u8) -> &mut Phrase { &mut self.phrases[index as usize] }

    pub fn clone_pattern(&mut self, from: u8, to: u8) {
        self.patterns[to as usize] = self.patterns[from as usize].clone();
    }

    pub fn clone_phrase(&mut self, from: u8, to: u8) {
        self.phrases[to as usize] = self.phrases[from as usize].clone();
    }

    pub fn starting_notes(&self, range: Range<u32>, sequence_start: u32, phrase_index: u8) -> Vec<PlayingNoteEvent> {
        let phrase = self.phrase(phrase_index);

        let phrase_start_tick = (range.start - sequence_start) % phrase.length();
        let iteration = (range.start - sequence_start) / phrase.length();
        let mut phrase_stop_tick = (range.end - sequence_start) % phrase.length();
        if phrase_stop_tick == 0 { 
            phrase_stop_tick = phrase.length();
        }

        phrase.pattern_events.iter()
            // Create seperate end & start ranges for looping patterns,
            .flat_map(|pattern_event| {
                if pattern_event.is_looping() {
                    let offset = phrase.length() - pattern_event.stop().unwrap();
                    vec![
                        (0 .. pattern_event.stop().unwrap(), offset, pattern_event), 
                        (pattern_event.start() .. phrase.length(), 0, pattern_event)
                    ]
                } else {
                    vec![(pattern_event.start() .. pattern_event.stop().unwrap(), 0, pattern_event)]
                }
            })
            // Check if range overlaps with phrase range
            .filter(|(pattern_range, _, _)| {
                pattern_range.start < phrase_stop_tick && pattern_range.end > phrase_stop_tick
            })
            // Check if notes falls in current cycle, offset by note_offset to get correct part of
            // looping patterns
            .flat_map(|(pattern_range, note_offset, pattern_event)| {
                self.patterns[pattern_event.pattern as usize].note_events.iter()
                    .filter(|note_event| note_event.stop().is_some())
                    .filter_map(move |note_event| {
                        // TODO - Looping patterns with length set explicitly
                        let note_start_tick = phrase_start_tick + note_offset;
                        let note_stop_tick = phrase_stop_tick + note_offset;

                        if (note_start_tick .. note_stop_tick).contains(&(note_event.start() + pattern_range.start)) {
                            let base_tick = sequence_start + iteration * phrase.length();
                            let mut stop = note_event.stop().unwrap();
                            if note_event.is_looping() { stop += pattern_event.length(phrase.length()) }

                            let event = PlayingNoteEvent {
                                // subtract start_tick here to make up for the shift in start due
                                // to looping pattern
                                start: base_tick + note_event.start() + pattern_range.start - note_offset,
                                stop: base_tick + stop + pattern_range.start - note_offset,
                                note: note_event.note,
                                start_velocity: note_event.start_velocity,
                                stop_velocity: note_event.stop_velocity.unwrap(),
                            };

                            Some(event)
                        } else {
                            None
                        }
                    })
            })
            .collect()
    }

    pub fn output_midi(&mut self, cycle: &ProcessCycle, starting_notes: Vec<PlayingNoteEvent>) {
        // Always play note off messages
        let mut messages = vec![];

        self.playing_notes.retain(|note| {
            // Play & remove notes that fall in cycle
            if cycle.tick_range.contains(&note.stop) {
                let frame = cycle.tick_to_frame(note.stop);
                messages.push(TimedMessage::new(frame, Message::Note([0x80, note.note, note.stop_velocity])));
                false
            } else {
                true
            }
        });

        // Create actual midi from note representations
        let note_on = starting_notes.iter()
            .map(|note| {
                let frame = cycle.tick_to_frame(note.start);
                TimedMessage::new(frame, Message::Note([0x90, note.note, note.start_velocity]))
            });

        messages.extend(note_on);

        // Remember playing notes to later trigger note off message & output note on messages
        self.playing_notes.extend(starting_notes);

        // Output note off mesassages && write midi
        self.output.output_messages(&mut messages);
        self.output.write_midi(cycle.scope);
    }

    /*
    pub fn switch_knob_group(&mut self, group: u8) {
        self.knob_group = group;
    }

    pub fn set_knob_value(&mut self, index: u8, value: u8) -> u8 {
        let knob = self.knob_group * 16 + index;
        self.knob_values[knob as usize] = value;
        knob
    }

    pub fn get_knob_values(&self) -> &[u8] {
        let start = self.knob_group as usize * 16;
        let end = start as usize + 16;
        &self.knob_values[start .. end]
    }

    pub fn knob_value_changed(&mut self, knob: u8, value: u8) -> Option<u8> {
        if self.knob_values[knob as usize] != value {
            self.knob_values[knob as usize] = value;
            Some(value)
        } else {
            None
        }
    }
    */
}
