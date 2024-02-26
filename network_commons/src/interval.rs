use core::time::Duration;

#[derive(Debug, Clone, Copy)]
pub struct Interval {
    pub duration: Duration,
    pub multiplier: i8,
}

impl Interval {
    pub fn new(duration: Duration, multiplier: i8) -> Self {
        Self {
            duration,
            multiplier,
        }
    }

    pub fn as_nanos(&self) -> i64 {
        self.duration.as_nanos() as i64 * self.multiplier as i64
    }
}

impl From<Interval> for Duration {
    fn from(dws: Interval) -> Self {
        dws.duration
    }
}
