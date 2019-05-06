
use super::message::Message;

pub struct MainGrid {
    width: u8,
    current: [u8; 40],
    next: [u8; 40],
}

impl Grid for MainGrid {}

impl MainGrid {
    pub fn new() -> Self {
        MainGrid { width: 8, current: [0; 40], next: [0; 40] }
    }
}

pub struct RowGrid {
    width: u8,
    current: [u8; 8],
    next: [u8; 8],
}

impl Grid for RowGrid {}

impl RowGrid {
    pub fn new() -> Self {
        RowGrid { width: 8, current: [0; 8], next: [0; 8] }
    }
}

pub struct SequenceGrid {
    width: u8,
    current: [u8; 4],
    next: [u8; 4],
}

impl Grid for SequenceGrid {}

impl SequenceGrid {
    pub fn new() -> Self {
        SequenceGrid { width: 1, current: [0; 4], next: [0; 4] }
    }
}

pub struct SingleGrid {
    width: u8,
    current: [u8; 1],
    next: [u8; 1],
}

impl Grid for SingleGrid {}

impl SingleGrid {
    pub fn new() -> Self {
        SingleGrid { width: 1, current: [0; 1], next: [0; 1] }
    }
}

pub struct PlayableGrid {
    width: u8,
    current: [u8; 5],
    next: [u8; 5],
}

impl Grid for PlayableGrid {}

impl PlayableGrid {
    pub fn new() -> Self {
        PlayableGrid { width: 1, current: [0; 5], next: [0; 5] }
    }
}

// TODO - undraw & redraw?
pub trait Grid {
    fn switch_led(&mut self, x: u8, y: u8, state: u8) {
        // Do not allow switching outside of grid
        if x < self.width as i32 || x >= 0 || y < self.height as i32 || y >= 0 {
            self.next[x * self.width + y] = state;
        }
    }

    fn led_states(&mut self) -> Vec<(u8, u8, u8)> {
        // Generate ledstates to change current state to next state
        let led_states = (0..self.next.len() as u8)
            .filter(|index| self.next[index as usize] != self.current[index as usize])
            .map(|index| {
                let x = index % self.width;
                let y = index / self.width;
    
                (x, y, self.next[index])
            })
            .collect();

        // Make current state match next state as we're outputting that right now
        self.current = self.next.clone();
        
        // All the led_states
        led_states
    }
}
