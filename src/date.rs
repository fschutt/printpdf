#[cfg(not(target_arch = "wasm32"))]
pub use time::{OffsetDateTime, UtcOffset};

/// wasm32-unknown-unknown polyfill

#[cfg(all(feature = "js-sys", target_arch = "wasm32"))]
pub use self::js_sys_date::OffsetDateTime;
#[cfg(all(feature = "js-sys", target_arch = "wasm32"))]
mod js_sys_date {
    use js_sys::Date;
    use time::Month;

    #[derive(Debug, Clone, Default, PartialEq, PartialOrd, Ord, Eq, Hash)]
    pub struct OffsetDateTime(Date);

    impl serde::Serialize for OffsetDateTime {
        fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            "1970-01-01 00:00:00.00 +00:00:00".serialize(serializer)
        }
    }

    impl<'de> serde::Deserialize<'de> for OffsetDateTime {
        fn deserialize<D: serde::Deserializer<'de>>(
            deserializer: D,
        ) -> Result<OffsetDateTime, D::Error> {
            let _ = String::deserialize(deserializer)?;
            Ok(OffsetDateTime::now_utc())
        }
    }

    impl OffsetDateTime {
        pub fn unix_timestamp(self) -> i64 {
            0
        }

        pub fn from_unix_timestamp(_: i64) -> Option<Self> {
            Some(Self(Date::new(&(1000.0 * 60.0 * 24.0 * 5.0).into())))
        }

        #[inline(always)]
        pub fn now_utc() -> Self {
            let date = Date::new_0();
            OffsetDateTime(date)
        }

        #[inline(always)]
        pub fn now() -> Self {
            let date = Date::new_0();
            OffsetDateTime(date)
        }

        #[inline(always)]
        pub fn format(&self, format: impl ToString) -> String {
            // TODO
            "".into()
        }

        #[inline(always)]
        pub fn year(&self) -> i32 {
            self.0.get_full_year() as i32
        }

        #[inline(always)]
        pub fn month(&self) -> Month {
            match self.0.get_month() {
                0 => Month::January,
                1 => Month::February,
                2 => Month::March,
                3 => Month::April,
                4 => Month::May,
                5 => Month::June,
                6 => Month::July,
                7 => Month::August,
                8 => Month::September,
                9 => Month::October,
                10 => Month::November,
                11 => Month::December,
                _ => unreachable!(),
            }
        }

        #[inline(always)]
        pub fn day(&self) -> u8 {
            self.0.get_date() as u8
        }

        #[inline(always)]
        pub fn hour(&self) -> u8 {
            self.0.get_hours() as u8
        }

        #[inline(always)]
        pub fn minute(&self) -> u8 {
            self.0.get_minutes() as u8
        }

        #[inline(always)]
        pub fn second(&self) -> u8 {
            self.0.get_seconds() as u8
        }

        #[inline]
        pub fn offset(&self) -> super::UtcOffset {
            super::UtcOffset {
                hours: 0,
                minutes: 0,
                seconds: 0,
            }
        }
    }
}

#[cfg(all(not(feature = "js-sys"), target_arch = "wasm32"))]
pub use self::unix_epoch_stub_date::OffsetDateTime;
#[cfg(all(not(feature = "js-sys"), target_arch = "wasm32"))]
mod unix_epoch_stub_date {
    use time::Month;

    #[derive(Debug, PartialEq, Default, Copy, Clone, Eq, Ord, PartialOrd, Hash)]
    pub struct OffsetDateTime;

    impl serde::Serialize for OffsetDateTime {
        fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            "1970-01-01 00:00:00.00 +00:00:00".serialize(serializer)
        }
    }

    impl<'de> serde::Deserialize<'de> for OffsetDateTime {
        fn deserialize<D: serde::Deserializer<'de>>(
            deserializer: D,
        ) -> Result<OffsetDateTime, D::Error> {
            let _ = String::deserialize(deserializer)?;
            Ok(OffsetDateTime::from_unix_timestamp(0).unwrap_or_default())
        }
    }

    impl OffsetDateTime {
        pub fn from_unix_timestamp(_: usize) -> Result<Self, String> {
            Ok(OffsetDateTime)
        }

        pub fn unix_timestamp(self) -> i64 {
            0
        }

        #[inline(always)]
        pub fn now_utc() -> Self {
            OffsetDateTime
        }

        #[inline(always)]
        pub fn now() -> Self {
            OffsetDateTime
        }

        #[inline(always)]
        pub fn format(&self, _: impl ToString) -> String {
            // TODO
            "".into()
        }

        #[inline(always)]
        pub fn year(&self) -> i32 {
            1970
        }

        #[inline(always)]
        pub fn month(&self) -> Month {
            Month::January
        }

        #[inline(always)]
        pub fn day(&self) -> u8 {
            1
        }

        #[inline(always)]
        pub fn hour(&self) -> u8 {
            0
        }

        #[inline(always)]
        pub fn minute(&self) -> u8 {
            0
        }

        #[inline(always)]
        pub fn second(&self) -> u8 {
            0
        }

        #[inline]
        pub fn offset(&self) -> super::UtcOffset {
            super::UtcOffset {
                hours: 0,
                minutes: 0,
                seconds: 0,
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct UtcOffset {
    hours: i8,
    minutes: i8,
    seconds: i8,
}

#[cfg(target_arch = "wasm32")]
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

    pub const fn is_negative(self) -> bool {
        self.hours < 0 || self.minutes < 0 || self.seconds < 0
    }

    pub const fn minutes_past_hour(self) -> i8 {
        self.minutes
    }
}
