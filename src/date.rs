/// wasm32-unknown-unknown polyfill

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
pub use self::js_sys_date::OffsetDateTime;
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
pub use time::OffsetDateTime;

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
mod js_sys_date {
    use js_sys::Date;
    #[derive(Debug, Clone)]
    pub struct OffsetDateTime(Date);
    impl OffsetDateTime {
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
        pub fn year(&self) -> u32 {
            self.0.get_full_year()
        }

        #[inline(always)]
        pub fn month(&self) -> u32 {
            self.0.get_month() + 1u32
        }

        #[inline(always)]
        pub fn day(&self) -> u32 {
            self.0.get_date()
        }

        #[inline(always)]
        pub fn hour(&self) -> u32 {
            self.0.get_hours()
        }

        #[inline(always)]
        pub fn minute(&self) -> u32 {
            self.0.get_minutes()
        }

        #[inline(always)]
        pub fn second(&self) -> u32 {
            self.0.get_seconds()
        }
    }
}
