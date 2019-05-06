
use super::{TICKS_PER_BEAT, BEATS_PER_BAR};
use super::message::Message;

pub struct Playable {
    minimum_ticks: u32,
    pub ticks: u32,
    pub zoom: u32,
    pub offset: u32,
}

fn bars_to_ticks(bars: u8) -> u32 {
    bars as u32 * BEATS_PER_BAR as u32 * TICKS_PER_BEAT as u32
}

impl Playable {
    pub fn new(bars: u8, minimum_bars: u8) -> Self {
        Playable {
            minimum_ticks: bars_to_ticks(minimum_bars),
            ticks: bars_to_ticks(bars),
            zoom: 1, 
            offset: 0,
        }
    }

    pub fn ticks_per_led(&self, leds: u32) -> u32 {
        self.ticks / self.zoom / leds
    }

    pub fn ticks_offset(&self, leds: u32) -> u32 {
        leds * self.offset * self.ticks_per_led()
    }

    pub fn beats(&self) -> u32 {
        self.ticks / TICKS_PER_BEAT as u32
    }

    pub fn bars(&self) -> u32 {
        self.beats() / BEATS_PER_BAR as u32
    }

    pub fn coords_to_leds(&self, coords: Vec<(u32, u32, i32)>, leds: u32) -> Vec<(i32, i32, u8)> {
        return coords.into_iter()
            .flat_map(|(start, end, y)| {
                let start_led = (start as i32 - self.ticks_offset(leds) as i32) / self.ticks_per_led(leds) as i32;
                let total_leds = (end - start) / self.ticks_per_led(leds);

                let mut head = vec![(start_led, y, 1)];
                let tail: Vec<(i32, i32, u8)> = (1..total_leds).map(|led| (start_led + led as i32, y, 5)).collect();
                head.extend(tail);
                head
            })
            .collect()
    }

    fn length_modifier(&self) -> u32 {
        self.ticks / self.minimum_ticks
    }

    pub fn change_zoom(&mut self, button: u32) {
        match button {
            1 | 2 | 4 | 8 => { self.zoom = 8 / button; self.offset = 0 },
            5 => { self.zoom = 2; self.offset = 1 },
            7 => { self.zoom = 4; self.offset = 3 },
            3 | 6 => { self.zoom = 8; self.offset = button - 1 },
            _ => ()
        }
    }

    pub fn change_offset(&mut self, delta: i32) -> bool {
        let offset = self.offset as i32 + delta;

        if offset >= 0 && offset <= self.zoom as i32 - 1 {
            self.offset = offset as u32;
            true
        } else {
            false
        }
    }
    
    pub fn change_length(&mut self, length_modifier: u8) -> bool {
        match length_modifier {
            1 | 2 | 4 | 8  => {
                // Calculate new zoom level to keep pattern grid view the same if possible
                let zoom = self.zoom * length_modifier as u32 / self.length_modifier() as u32;
                self.ticks = length_modifier as u32 * self.minimum_ticks;
                // Only set zoom when it's possible
                if zoom > 0 && zoom <= 8 {
                    self.zoom = zoom;
                }
                true
            },
            _ => false,
        }
    }

}
