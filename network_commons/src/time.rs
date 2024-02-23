use crate::error::CommonError;
use crate::interval::Interval;
use bebytes::BeBytes;
use core::fmt::{self};
use core::ops::{Add, Sub};
use core::time::Duration;
use libc::{clock_gettime, gmtime, localtime, time, time_t, timespec, tm, CLOCK_REALTIME};
use serde::{Deserialize, Serialize, Serializer};
use std::time::SystemTime;

/// Seconds between Jan 1, 1900 and Jan 1, 1970
pub const NTP_EPOCH: i64 = 2_208_988_800;
/// Number of nanoseconds in 1 second
const NSECS_CONVERSION: f64 = 1_000_000_000.0;
/// NTP fraction conversion factor (2^32)
const FRACTION_CONVERSION: f64 = 4_294_967_296.0;

#[repr(C)]
pub struct ScmTimestamping {
    pub ts_realtime: libc::timespec,
    pub ts_mono: libc::timespec,
    pub ts_raw: libc::timespec,
}

#[derive(Debug, Deserialize, Clone, Copy)]
pub struct DateTime {
    pub sec: u32,
    pub nanos: u32,
}

impl fmt::Display for DateTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Calculate Julian day number
        let jdn = (self.sec as f64 / 86400.0).floor() + 2440587.5;

        // Calculate Gregorian calendar date
        let z = (jdn + 0.5).floor();
        let w = ((z - 1867216.25) / 36524.25).floor();
        let x = (w / 4.0).floor();
        let a = z + 1.0 + w - x;
        let b = a + 1524.0;
        let c = ((b - 122.1) / 365.25).floor();
        let d = (365.25 * c).floor();
        let e = ((b - d) / 30.6001).floor();
        let day_frac = (30.6001 * e).floor();

        let day = (b - d - day_frac) as u8;
        let month = if e < 13.5 {
            (e - 1.0) as u8
        } else {
            (e - 13.0) as u8
        };
        let year = if month as f64 > 2.5 {
            (c - 4716.0) as u16
        } else {
            (c - 4715.0) as u16
        };

        let hour = ((self.sec % 86400) / 3600) as u8;
        let min = ((self.sec % 3600) / 60) as u8;
        let sec = (self.sec % 60) as u8;
        let nanos = self.nanos;
        let nanos_str = format!("{:09}", nanos);

        f.write_fmt(format_args!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{}Z",
            year, month, day, hour, min, sec, nanos_str
        ))
    }
}

impl serde::Serialize for DateTime {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl DateTime {
    pub fn utc_now() -> DateTime {
        let mut ts: timespec = timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };

        unsafe { clock_gettime(CLOCK_REALTIME, &mut ts) };
        DateTime {
            sec: ts.tv_sec as u32,
            nanos: ts.tv_nsec as u32,
        }
    }

    pub fn timestamp(&self) -> f64 {
        self.sec as f64 + (self.nanos as f64 / 1_000_000_000.0)
    }

    pub fn get_sec(&self) -> u32 {
        self.sec
    }

    pub fn set_sec(&mut self, sec: u32) {
        self.sec = sec;
    }

    pub fn get_nanos(&self) -> u32 {
        self.nanos
    }

    pub fn set_nanos(&mut self, nanos: u32) {
        self.nanos = nanos;
    }

    pub fn from_nanos(nanos: u64) -> DateTime {
        DateTime {
            sec: (nanos / 1_000_000_000) as u32,
            nanos: (nanos % 1_000_000_000) as u32,
        }
    }

    pub fn from_timespec(ts: timespec) -> DateTime {
        DateTime {
            sec: ts.tv_sec as u32,
            nanos: ts.tv_nsec as u32,
        }
    }
}

impl Add<Duration> for DateTime {
    type Output = DateTime;

    fn add(self, other: Duration) -> DateTime {
        let secs = self.sec + other.as_secs() as u32;
        let nanos = self.nanos + other.subsec_nanos();
        let secs_overflow = nanos / 1_000_000_000;
        let nanos = nanos % 1_000_000_000;
        DateTime {
            sec: (secs + secs_overflow),
            nanos,
        }
    }
}

impl Sub<Duration> for DateTime {
    type Output = DateTime;

    fn sub(self, other: Duration) -> DateTime {
        // Calculate seconds and nanoseconds difference without absolute value,
        // allowing for negative durations.
        let mut secs = self.sec as i64 - other.as_secs() as i64;
        let mut nanos = self.nanos as i64 - other.subsec_nanos() as i64;

        // If nanos is negative, borrow 1 from secs and adjust nanos accordingly.
        if nanos < 0 {
            secs -= 1;
            nanos += 1_000_000_000; // Adjust nanos after borrowing from secs.
        }

        // Ensure secs does not go negative
        if secs < 0 {
            secs = 0;
            nanos = 0;
        }

        DateTime {
            sec: secs as u32,
            nanos: nanos as u32,
        }
    }
}

impl Sub<DateTime> for DateTime {
    type Output = Interval;
    fn sub(self, other: DateTime) -> Interval {
        let secs_diff = self.sec as i64 - other.sec as i64;
        let nanos_diff = self.nanos as i64 - other.nanos as i64;

        // Combine the seconds and nanoseconds differences into a total nanoseconds difference
        let total_nanos_diff = secs_diff * 1_000_000_000 + nanos_diff;

        // Determine the sign and absolute value of the total difference
        let sign = if total_nanos_diff < 0 { -1 } else { 1 };
        let abs_nanos_diff = total_nanos_diff.abs();

        // Convert the absolute nanoseconds difference into seconds and nanoseconds
        let duration_secs = (abs_nanos_diff / 1_000_000_000) as u64;
        let duration_nanos = (abs_nanos_diff % 1_000_000_000) as u32;

        Interval::new(Duration::new(duration_secs, duration_nanos), sign)
    }
}

// In memory representation of an NTP timestamp
/// See [RFC5905](https://www.rfc-editor.org/rfc/rfc5905)
#[derive(BeBytes, Debug, PartialEq, Eq, Clone, Copy, Serialize)]
pub struct NtpTimestamp {
    /// The number of seconds since the NTP epoch, which is January 1, 1900.
    pub seconds: u32,
    /// The fractional part of a second, with a resolution of about 232 picoseconds (2^-32 seconds).
    pub fraction: u32,
}

impl NtpTimestamp {
    /// Returns the current NTP timestamp.
    pub fn now() -> Self {
        // The difference between the UNIX epoch (January 1, 1970) and the NTP epoch (January 1, 1900) in seconds.
        const EPOCH_DIFFERENCE: u64 = 2_208_988_800;

        // Get the current time since the UNIX epoch.
        let unix_duration = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("Current time is earlier than UNIX epoch");

        // Convert the time to NTP format.
        let ntp_seconds = (unix_duration.as_secs() + EPOCH_DIFFERENCE) as u32;
        let ntp_fraction =
            (unix_duration.subsec_nanos() as f64 / 1_000_000_000.0 * (1u64 << 32) as f64) as u32;

        NtpTimestamp {
            seconds: ntp_seconds,
            fraction: ntp_fraction,
        }
    }

    /// Converts the system timestamp to the NTP timestamp format.
    pub fn ntp_from_timespec(
        sec_since_unix_epoch: u64,
        nanosec_since_last_sec: u64,
    ) -> NtpTimestamp {
        let ntp_epoch_offset = NTP_EPOCH as u64;
        let ntp_ts = (
            (sec_since_unix_epoch + ntp_epoch_offset) as f64,
            nanosec_since_last_sec as f64,
        );
        let seconds = ntp_ts.0 as u32;
        let fraction = ((ntp_ts.1 / 1_000_000_000.0) * (2.0_f64.powi(32))) as u32;

        NtpTimestamp { seconds, fraction }
    }

    /// Retrieves the Local - GM time offset in minutes
    pub fn get_timezone_offset(&self) -> i32 {
        let mut now: time_t = 0;
        unsafe {
            time(&mut now as *mut _);
            let local_tm: *mut tm = localtime(&now as *const _);
            let gmt_tm: *mut tm = gmtime(&now as *const _);

            let hour_offset = (*local_tm).tm_hour - (*gmt_tm).tm_hour;
            let min_offset = (*local_tm).tm_min - (*gmt_tm).tm_min;

            hour_offset * 60 + min_offset
        }
    }
}

impl From<DateTime> for NtpTimestamp {
    fn from(dt: DateTime) -> Self {
        let seconds = dt.timestamp() as u32 + NTP_EPOCH as u32;
        let fraction = ((dt.get_nanos() as f64) / NSECS_CONVERSION * FRACTION_CONVERSION) as u32;
        log::debug!("FN {}.{}", seconds, fraction);
        Self { seconds, fraction }
    }
}

impl TryFrom<NtpTimestamp> for DateTime {
    type Error = CommonError;

    fn try_from(timestamp: NtpTimestamp) -> Result<Self, CommonError> {
        let seconds = timestamp.seconds as i64 - NTP_EPOCH;
        let nsecs =
            (timestamp.fraction as f64 * NSECS_CONVERSION / FRACTION_CONVERSION).round() as u32;

        let datetime = DateTime {
            sec: seconds as u32,
            nanos: nsecs,
        };

        Ok(datetime)
    }
}
