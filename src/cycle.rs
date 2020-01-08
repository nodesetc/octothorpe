
use std::ops::Range;

pub struct ProcessCycle<'a> {
    pub client: &'a jack::Client,
    pub scope: &'a jack::ProcessScope,
    pub tick_range: Range<u32>,
    pub time_range: Range<u64>,
    pub is_rolling: bool,
}

impl<'a> ProcessCycle<'a> {
    pub fn frame_to_tick(pos: jack::Position, frame: u32) -> f64 {
        let second = frame as f64 / pos.frame_rate as f64;
        second / 60.0 * pos.beats_per_minute * pos.ticks_per_beat
    }

    // Save client as we pass this cycle thing everywhere
    pub fn new(client: &'a jack::Client, scope: &'a jack::ProcessScope) -> Self {
        let cycle_times = scope.cycle_times().unwrap();
        let (state, pos) = client.transport_query();

        let tick_start = Self::frame_to_tick(pos, pos.frame) as u32;
        let tick_stop = Self::frame_to_tick(pos, pos.frame + scope.n_frames()) as u32;

        Self {
            client,
            scope,
            time_range: cycle_times.current_usecs .. cycle_times.next_usecs,
            tick_range: tick_start .. tick_stop,
            is_rolling: state == 1,
        }
    }

    pub fn usecs(&self) -> u64 {
        self.time_range.end - self.time_range.start
    }

    pub fn ticks(&self) -> u32 {
        self.tick_range.end - self.tick_range.start
    }

    pub fn time_at_frame(&self, frame: u32) -> u64 {
        // TODO - When can this error?
        let usecs_per_frame = self.usecs() as f32 / self.scope.n_frames() as f32;
        let usecs_since_period_start = frame as f32 * usecs_per_frame;
        self.time_range.start + usecs_since_period_start as u64
    }

    // TODO - This can panic, is that what we want?
    pub fn tick_to_frame(&self, tick: u32) -> u32 {
        let tick_in_cycle = tick - self.tick_range.start;
        let frame_in_cycle = tick_in_cycle as f64 / self.ticks() as f64 * self.scope.n_frames() as f64;
        frame_in_cycle as u32
    }
}

/*
#[derive(Clone, Debug)]
pub struct Cycle {
    pub start: u32,
    pub end: u32,
    pub absolute_start: u32,
    pub absolute_end: u32,
    pub ticks: u32,
    pub frames: u32,

    pub is_rolling: bool,
    // Is this cycle a 0 length reposition cycle?
    pub is_repositioned: bool,
    // Was last cycle a reposition cycle?
    pub was_repositioned: bool,
}

impl Cycle {
    pub fn new(pos: jack::Position, absolute_start: u32, was_repositioned: bool, frames: u32, state: u32) -> Self {
        let start = Cycle::get_tick(pos, pos.frame) as u32;
        let end = Cycle::get_tick(pos, pos.frame + frames) as u32;
        let ticks = end - start;

        let is_rolling = state == 1;
        // Seems repositioning causes a 0 ticks cycle
        let is_repositioned = start == end;

        Cycle { 
            start, 
            end, 
            absolute_start,
            absolute_end: absolute_start + ticks,
            ticks, 
            frames, 
            is_rolling,
            is_repositioned,
            was_repositioned,
        }
    }

    // Used to get repositioned cycle for transport reposition logic
    pub fn repositioned(&self, start: u32) -> Self {
        let mut cycle = self.clone();
        cycle.start = start;
        cycle.end = start + cycle.ticks;
        cycle
    }

    pub fn get_tick(pos: jack::Position, frame: u32) -> f64 {
        let second = frame as f64 / pos.frame_rate as f64;
        second / 60.0 * pos.beats_per_minute * pos.ticks_per_beat
    }

    pub fn ticks_to_frames(&self, ticks: u32) -> u32 {
        (ticks as f64 / self.ticks as f64 * self.frames as f64) as u32
    }

    pub fn delta_frames(&self, tick: u32) -> Option<u32> {
        if tick >= self.start && tick < self.end {
            Some(self.ticks_to_frames(tick - self.start))
        } else {
            None
        }
    }

    pub fn delta_frames_absolute(&self, tick: u32) -> Option<u32> {
        if tick >= self.absolute_start && tick < self.absolute_end {
            Some(self.ticks_to_frames(tick - self.absolute_start))
        } else {
            None
        }
    }

    // Check if a recurring ticks interval falls in this cycle
    pub fn delta_ticks_recurring(&self, tick: u32, interval: u32) -> Option<u32> {
        let pattern_start = self.start % interval;
        let pattern_end = pattern_start + self.ticks;
        let next_tick = tick + interval;

        if tick >= pattern_start && tick < pattern_end
            || next_tick >= pattern_start && next_tick < pattern_end 
        {
            if pattern_start > tick {
                Some(next_tick - pattern_start)
            } else {
                Some(tick - pattern_start)
            }
        } else {
            None
        }
    }
}
*/
