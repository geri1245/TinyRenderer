use std::time::{Duration, Instant};

pub struct FrameTimer {
    pub last_frame_time: Instant,
}

impl FrameTimer {
    pub fn new() -> FrameTimer {
        FrameTimer {
            last_frame_time: Instant::now(),
        }
    }

    pub fn get_delta_and_reset_timer(&mut self) -> Duration {
        let now = Instant::now();
        let frame_time = now.duration_since(self.last_frame_time);
        self.last_frame_time = now;

        frame_time
    }
}
