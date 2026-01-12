use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Date {
    pub year: i32,
    pub month: u8,
    pub day: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Time {
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub millisecond: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Offset {
    pub hours: i8,
    pub minutes: i8,
    pub seconds: i8,
    pub milliseconds: i16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DateTime {
    pub date: Date,
    pub time: Time,
    pub offset: Offset,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Month {
    January = 1,
    February,
    March,
    April,
    May,
    June,
    July,
    August,
    September,
    October,
    November,
    December,
}

impl From<u8> for Month {
    fn from(month: u8) -> Self {
        match month {
            1 => Month::January,
            2 => Month::February,
            3 => Month::March,
            4 => Month::April,
            5 => Month::May,
            6 => Month::June,
            7 => Month::July,
            8 => Month::August,
            9 => Month::September,
            10 => Month::October,
            11 => Month::November,
            12 => Month::December,
            _ => Month::January,
        }
    }
}

impl From<Month> for u8 {
    fn from(month: Month) -> Self {
        month as u8
    }
}

impl std::fmt::Display for Month {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", *self as u8)
    }
}

impl DateTime {
    pub fn new_in_offset(date: Date, time: Time, offset: Offset) -> Self {
        Self { date, time, offset }
    }

    pub const fn epoch() -> Self {
        Self {
            date: Date {
                year: 1970,
                month: 1,
                day: 1,
            },
            time: Time {
                hour: 0,
                minute: 0,
                second: 0,
                millisecond: 0,
            },
            offset: Offset {
                hours: 0,
                minutes: 0,
                seconds: 0,
                milliseconds: 0,
            },
        }
    }

    // Non-WASM implementation
    #[cfg(not(target_arch = "wasm32"))]
    pub fn now() -> Self {
        use time::OffsetDateTime;

        let now = OffsetDateTime::now_utc();

        Self {
            date: Date {
                year: now.year(),
                month: now.month() as u8,
                day: now.day(),
            },
            time: Time {
                hour: now.hour(),
                minute: now.minute(),
                second: now.second(),
                millisecond: now.millisecond(),
            },
            offset: Offset {
                hours: now.offset().whole_hours() as i8,
                minutes: now.offset().minutes_past_hour() as i8,
                seconds: 0,
                milliseconds: 0,
            },
        }
    }

    // Browser WASM implementation (js-sys enabled, not WASI)
    #[cfg(all(
        feature = "js-sys",
        target_arch = "wasm32",
        not(any(target_env = "p1", target_env = "p2"))
    ))]
    pub fn now() -> Self {
        use js_sys::Date as JsDate;

        let js_date = JsDate::new_0();
        let month = (js_date.get_month() + 1) as u8;
        let tz_offset_minutes = js_date.get_timezone_offset() as i16;
        let offset_hours = -(tz_offset_minutes / 60) as i8;
        let offset_minutes = -(tz_offset_minutes % 60) as i8;

        Self {
            date: Date {
                year: js_date.get_full_year() as i32,
                month,
                day: js_date.get_date() as u8,
            },
            time: Time {
                hour: js_date.get_hours() as u8,
                minute: js_date.get_minutes() as u8,
                second: js_date.get_seconds() as u8,
                millisecond: js_date.get_milliseconds() as u16,
            },
            offset: Offset {
                hours: offset_hours,
                minutes: offset_minutes,
                seconds: 0,
                milliseconds: 0,
            },
        }
    }

    // WASI or non-js-sys WASM implementation
    #[cfg(any(
        all(target_arch = "wasm32", any(target_env = "p1", target_env = "p2")),
        all(not(feature = "js-sys"), target_arch = "wasm32")
    ))]
    pub fn now() -> Self {
        Self::epoch()
    }

    pub fn now_utc() -> Self {
        Self::now()
    }

    pub fn format(&self, _format: &str) -> String {
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03} {:+03}:{:02}:{:02}",
            self.date.year,
            self.date.month,
            self.date.day,
            self.time.hour,
            self.time.minute,
            self.time.second,
            self.time.millisecond,
            self.offset.hours,
            self.offset.minutes,
            self.offset.seconds
        )
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn unix_timestamp(&self) -> i64 {
        use time::{Date as TimeDate, Month as TimeMonth, PrimitiveDateTime, Time as TimeTime};

        let month = match self.date.month {
            1 => TimeMonth::January,
            2 => TimeMonth::February,
            3 => TimeMonth::March,
            4 => TimeMonth::April,
            5 => TimeMonth::May,
            6 => TimeMonth::June,
            7 => TimeMonth::July,
            8 => TimeMonth::August,
            9 => TimeMonth::September,
            10 => TimeMonth::October,
            11 => TimeMonth::November,
            12 => TimeMonth::December,
            _ => return 0,
        };

        let date = match TimeDate::from_calendar_date(self.date.year, month, self.date.day) {
            Ok(d) => d,
            Err(_) => return 0,
        };

        let time = match TimeTime::from_hms_milli(
            self.time.hour,
            self.time.minute,
            self.time.second,
            self.time.millisecond,
        ) {
            Ok(t) => t,
            Err(_) => return 0,
        };

        let offset = match time::UtcOffset::from_hms(
            self.offset.hours as i8,
            self.offset.minutes as i8,
            self.offset.seconds as i8,
        ) {
            Ok(o) => o,
            Err(_) => time::UtcOffset::UTC,
        };

        let primitive_dt = PrimitiveDateTime::new(date, time);
        let offset_dt = primitive_dt.assume_offset(offset);

        offset_dt.unix_timestamp()
    }

    #[cfg(all(
        feature = "js-sys",
        target_arch = "wasm32",
        not(any(target_env = "p1", target_env = "p2"))
    ))]
    pub fn unix_timestamp(&self) -> i64 {
        use js_sys::Date as JsDate;

        let js_date = JsDate::new_with_year_month_day_hr_min_sec(
            self.date.year as u32,
            (self.date.month - 1) as i32,
            self.date.day as i32,
            self.time.hour as i32,
            self.time.minute as i32,
            self.time.second as i32,
        );

        (js_date.get_time() / 1000.0) as i64
    }

    #[cfg(any(
        all(target_arch = "wasm32", any(target_env = "p1", target_env = "p2")),
        all(not(feature = "js-sys"), target_arch = "wasm32")
    ))]
    pub fn unix_timestamp(&self) -> i64 {
        0
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_unix_timestamp(timestamp: i64) -> Result<Self, &'static str> {
        use time::OffsetDateTime;

        let dt = OffsetDateTime::from_unix_timestamp(timestamp).map_err(|_| "Invalid timestamp")?;

        Ok(Self {
            date: Date {
                year: dt.year(),
                month: dt.month() as u8,
                day: dt.day(),
            },
            time: Time {
                hour: dt.hour(),
                minute: dt.minute(),
                second: dt.second(),
                millisecond: dt.millisecond(),
            },
            offset: Offset {
                hours: dt.offset().whole_hours() as i8,
                minutes: dt.offset().minutes_past_hour() as i8,
                seconds: 0,
                milliseconds: 0,
            },
        })
    }

    #[cfg(all(
        feature = "js-sys",
        target_arch = "wasm32",
        not(any(target_env = "p1", target_env = "p2"))
    ))]
    pub fn from_unix_timestamp(timestamp: i64) -> Result<Self, &'static str> {
        use js_sys::Date as JsDate;

        if timestamp < 0 {
            return Err("Invalid timestamp");
        }

        let js_date = JsDate::new(&((timestamp as f64) * 1000.0).into());

        let month = (js_date.get_month() + 1) as u8;
        let tz_offset_minutes = js_date.get_timezone_offset() as i16;
        let offset_hours = -(tz_offset_minutes / 60) as i8;
        let offset_minutes = -(tz_offset_minutes % 60) as i8;

        Ok(Self {
            date: Date {
                year: js_date.get_full_year() as i32,
                month,
                day: js_date.get_date() as u8,
            },
            time: Time {
                hour: js_date.get_hours() as u8,
                minute: js_date.get_minutes() as u8,
                second: js_date.get_seconds() as u8,
                millisecond: js_date.get_milliseconds() as u16,
            },
            offset: Offset {
                hours: offset_hours,
                minutes: offset_minutes,
                seconds: 0,
                milliseconds: 0,
            },
        })
    }

    #[cfg(any(
        all(target_arch = "wasm32", any(target_env = "p1", target_env = "p2")),
        all(not(feature = "js-sys"), target_arch = "wasm32")
    ))]
    pub fn from_unix_timestamp(_timestamp: i64) -> Result<Self, &'static str> {
        Ok(Self::epoch())
    }

    pub fn year(&self) -> i32 {
        self.date.year
    }
    pub fn month(&self) -> Month {
        Month::from(self.date.month)
    }
    pub fn day(&self) -> u8 {
        self.date.day
    }
    pub fn hour(&self) -> u8 {
        self.time.hour
    }
    pub fn minute(&self) -> u8 {
        self.time.minute
    }
    pub fn second(&self) -> u8 {
        self.time.second
    }
    pub fn millisecond(&self) -> u16 {
        self.time.millisecond
    }

    pub fn offset(&self) -> UtcOffset {
        UtcOffset {
            hours: self.offset.hours,
            minutes: self.offset.minutes,
            seconds: self.offset.seconds,
        }
    }
}

// Rest of the code remains unchanged

impl ToString for DateTime {
    fn to_string(&self) -> String {
        self.format("")
    }
}

impl std::str::FromStr for DateTime {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split_whitespace().collect();
        if parts.len() < 3 {
            return Err("Invalid datetime format".to_string());
        }

        let date_parts: Vec<&str> = parts[0].split('-').collect();
        if date_parts.len() != 3 {
            return Err("Invalid date format".to_string());
        }

        let year = date_parts[0].parse::<i32>().map_err(|_| "Invalid year")?;
        let month = date_parts[1].parse::<u8>().map_err(|_| "Invalid month")?;
        let day = date_parts[2].parse::<u8>().map_err(|_| "Invalid day")?;

        let time_parts: Vec<&str> = parts[1].split(':').collect();
        if time_parts.len() < 2 {
            return Err("Invalid time format".to_string());
        }

        let hour = time_parts[0].parse::<u8>().map_err(|_| "Invalid hour")?;
        let minute = time_parts[1].parse::<u8>().map_err(|_| "Invalid minute")?;

        let (second, millisecond) = if time_parts.len() > 2 {
            let seconds_and_millis: Vec<&str> = time_parts[2].split('.').collect();
            let second = seconds_and_millis[0]
                .parse::<u8>()
                .map_err(|_| "Invalid second")?;
            let millisecond = if seconds_and_millis.len() > 1 {
                seconds_and_millis[1]
                    .parse::<u16>()
                    .map_err(|_| "Invalid millisecond")?
            } else {
                0
            };
            (second, millisecond)
        } else {
            (0, 0)
        };

        let offset_str = parts[2];
        if offset_str.len() < 2 {
            return Err("Invalid offset format".to_string());
        }

        let offset_sign = if offset_str.starts_with('-') { -1 } else { 1 };
        let offset_parts: Vec<&str> = offset_str[1..].split(':').collect();

        if offset_parts.is_empty() {
            return Err("Invalid offset format".to_string());
        }

        let offset_hours = offset_sign
            * offset_parts[0]
                .parse::<i8>()
                .map_err(|_| "Invalid offset hours")?;

        let offset_minutes = if offset_parts.len() > 1 {
            offset_sign
                * offset_parts[1]
                    .parse::<i8>()
                    .map_err(|_| "Invalid offset minutes")?
        } else {
            0
        };

        let offset_seconds = if offset_parts.len() > 2 {
            offset_sign
                * offset_parts[2]
                    .parse::<i8>()
                    .map_err(|_| "Invalid offset seconds")?
        } else {
            0
        };

        let dt = DateTime {
            date: Date { year, month, day },
            time: Time {
                hour,
                minute,
                second,
                millisecond,
            },
            offset: Offset {
                hours: offset_hours,
                minutes: offset_minutes,
                seconds: offset_seconds,
                milliseconds: 0,
            },
        };

        if let Err(e) = check_date_valid(&dt) {
            Err(e)
        } else {
            Ok(dt)
        }
    }
}

impl Serialize for DateTime {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.to_string().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for DateTime {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

pub type OffsetDateTime = DateTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct UtcOffset {
    pub hours: i8,
    pub minutes: i8,
    pub seconds: i8,
}

impl UtcOffset {
    pub const fn from_hms(hours: i8, minutes: i8, seconds: i8) -> Result<Self, &'static str> {
        Ok(Self {
            hours,
            minutes,
            seconds,
        })
    }

    pub const fn whole_hours(self) -> i8 {
        self.hours
    }
    pub const fn minutes_past_hour(self) -> i8 {
        self.minutes
    }
    pub const fn seconds_past_minute(self) -> i8 {
        self.seconds
    }
    pub const fn is_negative(self) -> bool {
        self.hours < 0 || self.minutes < 0 || self.seconds < 0
    }
}

// Serialization implementations remain unchanged

impl Serialize for Date {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Date {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let parts: Vec<&str> = s.split('-').collect();

        if parts.len() != 3 {
            return Err(serde::de::Error::custom("Invalid date format"));
        }

        let year = parts[0]
            .parse::<i32>()
            .map_err(|_| serde::de::Error::custom("Invalid year"))?;
        let month = parts[1]
            .parse::<u8>()
            .map_err(|_| serde::de::Error::custom("Invalid month"))?;
        let day = parts[2]
            .parse::<u8>()
            .map_err(|_| serde::de::Error::custom("Invalid day"))?;

        Ok(Date { year, month, day })
    }
}

impl Serialize for Time {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        format!(
            "{:02}:{:02}:{:02}.{:03}",
            self.hour, self.minute, self.second, self.millisecond
        )
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Time {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let parts: Vec<&str> = s.split(':').collect();

        if parts.len() < 2 {
            return Err(serde::de::Error::custom("Invalid time format"));
        }

        let hour = parts[0]
            .parse::<u8>()
            .map_err(|_| serde::de::Error::custom("Invalid hour"))?;
        let minute = parts[1]
            .parse::<u8>()
            .map_err(|_| serde::de::Error::custom("Invalid minute"))?;

        let (second, millisecond) = if parts.len() > 2 {
            let sec_parts: Vec<&str> = parts[2].split('.').collect();
            let second = sec_parts[0]
                .parse::<u8>()
                .map_err(|_| serde::de::Error::custom("Invalid second"))?;
            let millisecond = if sec_parts.len() > 1 {
                sec_parts[1]
                    .parse::<u16>()
                    .map_err(|_| serde::de::Error::custom("Invalid millisecond"))?
            } else {
                0
            };
            (second, millisecond)
        } else {
            (0, 0)
        };

        Ok(Time {
            hour,
            minute,
            second,
            millisecond,
        })
    }
}

impl Offset {
    fn is_negative(&self) -> bool {
        self.hours < 0 || self.minutes < 0 || self.seconds < 0 || self.milliseconds < 0
    }
}

impl Serialize for Offset {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let sign = if self.is_negative() { '-' } else { '+' };
        format!(
            "{}{:02}:{:02}:{:02}.{:03}",
            sign,
            self.hours.abs(),
            self.minutes.abs(),
            self.seconds.abs(),
            self.milliseconds.abs()
        )
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Offset {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;

        if s.len() < 2 {
            return Err(serde::de::Error::custom("Invalid offset format"));
        }

        let offset_sign = if s.starts_with('-') { -1 } else { 1 };
        let offset_parts: Vec<&str> = s[1..].split(':').collect();

        if offset_parts.is_empty() {
            return Err(serde::de::Error::custom("Invalid offset format"));
        }

        let hours = offset_sign
            * offset_parts[0]
                .parse::<i8>()
                .map_err(|_| serde::de::Error::custom("Invalid offset hours"))?;

        let minutes = if offset_parts.len() > 1 {
            offset_sign
                * offset_parts[1]
                    .parse::<i8>()
                    .map_err(|_| serde::de::Error::custom("Invalid offset minutes"))?
        } else {
            0
        };

        let (seconds, milliseconds) = if offset_parts.len() > 2 {
            let sec_parts: Vec<&str> = offset_parts[2].split('.').collect();
            let seconds = offset_sign
                * sec_parts[0]
                    .parse::<i8>()
                    .map_err(|_| serde::de::Error::custom("Invalid offset seconds"))?;
            let milliseconds = if sec_parts.len() > 1 {
                offset_sign as i16
                    * sec_parts[1]
                        .parse::<i16>()
                        .map_err(|_| serde::de::Error::custom("Invalid offset milliseconds"))?
            } else {
                0
            };
            (seconds, milliseconds)
        } else {
            (0, 0)
        };

        Ok(Offset {
            hours,
            minutes,
            seconds,
            milliseconds,
        })
    }
}

impl Serialize for Month {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        (*self as u8).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Month {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let month = u8::deserialize(deserializer)?;
        Ok(Month::from(month))
    }
}

impl Serialize for UtcOffset {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        format!("{:+03}:{:02}:{:02}", self.hours, self.minutes, self.seconds).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for UtcOffset {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;

        if s.len() < 2 {
            return Err(serde::de::Error::custom("Invalid offset format"));
        }

        let offset_sign = if s.starts_with('-') { -1 } else { 1 };
        let offset_parts: Vec<&str> = s[1..].split(':').collect();

        if offset_parts.is_empty() {
            return Err(serde::de::Error::custom("Invalid offset format"));
        }

        let hours = offset_sign
            * offset_parts[0]
                .parse::<i8>()
                .map_err(|_| serde::de::Error::custom("Invalid offset hours"))?;

        let minutes = if offset_parts.len() > 1 {
            offset_sign
                * offset_parts[1]
                    .parse::<i8>()
                    .map_err(|_| serde::de::Error::custom("Invalid offset minutes"))?
        } else {
            0
        };

        let seconds = if offset_parts.len() > 2 {
            offset_sign
                * offset_parts[2]
                    .parse::<i8>()
                    .map_err(|_| serde::de::Error::custom("Invalid offset seconds"))?
        } else {
            0
        };

        Ok(UtcOffset {
            hours,
            minutes,
            seconds,
        })
    }
}

/// A simple parser for PDF date strings (e.g. "D:20170505150224+02'00'")
pub fn parse_pdf_date(s: &str) -> Result<OffsetDateTime, String> {
    let d = parse_pdf_date_inner(s)?;
    if let Err(e) = check_date_valid(&d) {
        Err(e)
    } else {
        Ok(d)
    }
}

fn parse_pdf_date_inner(s: &str) -> Result<OffsetDateTime, String> {
    // Remove a leading "D:" if present.
    let s = if s.starts_with("D:") { &s[2..] } else { s };
    if s.len() < 14 {
        return Err("Date string too short".to_string());
    }

    let year: i32 = s[0..4].parse::<i32>().map_err(|e| e.to_string())?;
    let month: u8 = s[4..6].parse::<u8>().map_err(|e| e.to_string())?;
    let day: u8 = s[6..8].parse::<u8>().map_err(|e| e.to_string())?;
    let hour: u8 = s[8..10].parse::<u8>().map_err(|e| e.to_string())?;
    let minute: u8 = s[10..12].parse::<u8>().map_err(|e| e.to_string())?;
    let second: u8 = s[12..14].parse::<u8>().map_err(|e| e.to_string())?;

    // Parse timezone offset if available
    let mut offset_hours = 0i8;
    let mut offset_minutes = 0i8;
    let mut offset_seconds = 0i8;

    if s.len() > 14 {
        let tz_sign = match &s[14..15] {
            "+" => 1i8,
            "-" => -1i8,
            _ => 0i8,
        };

        if s.len() >= 17 {
            offset_hours = tz_sign * s[15..17].parse::<i8>().unwrap_or(0);

            // Check for minutes offset (format: +02'00')
            if s.len() >= 20 && s.chars().nth(17) == Some('\'') {
                offset_minutes = tz_sign * s[18..20].parse::<i8>().unwrap_or(0);

                // Check for seconds offset if present
                if s.len() >= 23 && s.chars().nth(20) == Some('\'') {
                    offset_seconds = tz_sign * s[21..23].parse::<i8>().unwrap_or(0);
                }
            }
        }
    }

    let date = Date { year, month, day };
    let time = Time {
        hour,
        minute,
        second,
        millisecond: 0,
    };
    let offset = Offset {
        hours: offset_hours,
        minutes: offset_minutes,
        seconds: offset_seconds,
        milliseconds: 0,
    };

    Ok(OffsetDateTime::new_in_offset(date, time, offset))
}

fn check_date_valid(d: &DateTime) -> Result<(), String> {
    if !is_valid_date(d.date.year, d.date.month, d.date.day) {
        return Err(format!(
            "Invalid date: {}-{:02}-{:02}",
            d.date.year, d.date.month, d.date.day
        ));
    }

    if d.time.hour > 23 {
        return Err(format!("Invalid hour: {}", d.time.hour));
    }

    if d.time.minute > 59 {
        return Err(format!("Invalid minute: {}", d.time.minute));
    }

    if d.time.second > 59 {
        return Err(format!("Invalid second: {}", d.time.second));
    }

    if d.time.millisecond > 999 {
        return Err(format!("Invalid millisecond: {}", d.time.millisecond));
    }

    Ok(())
}

// Add this function at the module level
fn is_valid_date(year: i32, month: u8, day: u8) -> bool {
    if month < 1 || month > 12 {
        return false;
    }

    let days_in_month = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            // February - check for leap year
            if (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0) {
                29
            } else {
                28
            }
        }
        _ => unreachable!(),
    };

    day >= 1 && day <= days_in_month
}
