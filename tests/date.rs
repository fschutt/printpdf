use std::str::FromStr;

use printpdf::date::*;

#[test]
fn test_date_serialization() {
    let date = Date {
        year: 2025,
        month: 3,
        day: 19,
    };
    let serialized = serde_json::to_string(&date).unwrap();
    assert_eq!(serialized, "\"2025-03-19\"");

    let deserialized: Date = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized, date);
}

#[test]
fn test_time_serialization() {
    let time = Time {
        hour: 14,
        minute: 30,
        second: 45,
        millisecond: 123,
    };
    let serialized = serde_json::to_string(&time).unwrap();
    assert_eq!(serialized, "\"14:30:45.123\"");

    let deserialized: Time = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized, time);
}

#[test]
fn test_offset_serialization() {
    let offset = Offset {
        hours: 2,
        minutes: 30,
        seconds: 0,
        milliseconds: 0,
    };
    let serialized = serde_json::to_string(&offset).unwrap();
    assert_eq!(serialized, "\"+02:30:00.000\"");

    let negative_offset = Offset {
        hours: -5,
        minutes: -30,
        seconds: 0,
        milliseconds: 0,
    };
    let serialized = serde_json::to_string(&negative_offset).unwrap();
    assert_eq!(serialized, "\"-05:30:00.000\"");

    let deserialized: Offset = serde_json::from_str("\"+02:30:00.000\"").unwrap();
    assert_eq!(deserialized, offset);
}

#[test]
fn test_datetime_serialization() {
    let dt = DateTime {
        date: Date {
            year: 2025,
            month: 3,
            day: 19,
        },
        time: Time {
            hour: 14,
            minute: 30,
            second: 45,
            millisecond: 123,
        },
        offset: Offset {
            hours: 2,
            minutes: 30,
            seconds: 0,
            milliseconds: 0,
        },
    };

    let serialized = serde_json::to_string(&dt).unwrap();
    assert_eq!(serialized, "\"2025-03-19 14:30:45.123 +02:30:00\"");

    let deserialized: DateTime = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized, dt);
}

#[test]
fn test_datetime_str_parsing() {
    let dt_str = "2025-03-19 14:30:45.123 +02:30:00";
    let dt = DateTime::from_str(dt_str).unwrap();

    assert_eq!(dt.date.year, 2025);
    assert_eq!(dt.date.month, 3);
    assert_eq!(dt.date.day, 19);
    assert_eq!(dt.time.hour, 14);
    assert_eq!(dt.time.minute, 30);
    assert_eq!(dt.time.second, 45);
    assert_eq!(dt.time.millisecond, 123);
    assert_eq!(dt.offset.hours, 2);
    assert_eq!(dt.offset.minutes, 30);
    assert_eq!(dt.offset.seconds, 0);
}

#[test]
fn test_pdf_date_parsing() {
    // Basic date with no timezone
    let pdf_date = "D:20250319143045";
    let dt = parse_pdf_date(pdf_date).unwrap();

    assert_eq!(dt.date.year, 2025);
    assert_eq!(dt.date.month, 3);
    assert_eq!(dt.date.day, 19);
    assert_eq!(dt.time.hour, 14);
    assert_eq!(dt.time.minute, 30);
    assert_eq!(dt.time.second, 45);
    assert_eq!(dt.offset.hours, 0);
    assert_eq!(dt.offset.minutes, 0);

    // With timezone
    let pdf_date = "D:20250319143045+02'30'";
    let dt = parse_pdf_date(pdf_date).unwrap();

    assert_eq!(dt.date.year, 2025);
    assert_eq!(dt.date.month, 3);
    assert_eq!(dt.date.day, 19);
    assert_eq!(dt.time.hour, 14);
    assert_eq!(dt.time.minute, 30);
    assert_eq!(dt.time.second, 45);
    assert_eq!(dt.offset.hours, 2);
    assert_eq!(dt.offset.minutes, 30);

    // Negative timezone
    let pdf_date = "D:20250319143045-05'00'";
    let dt = parse_pdf_date(pdf_date).unwrap();

    assert_eq!(dt.date.year, 2025);
    assert_eq!(dt.date.month, 3);
    assert_eq!(dt.date.day, 19);
    assert_eq!(dt.time.hour, 14);
    assert_eq!(dt.time.minute, 30);
    assert_eq!(dt.time.second, 45);
    assert_eq!(dt.offset.hours, -5);
    assert_eq!(dt.offset.minutes, 0);

    // Without D: prefix
    let pdf_date = "20250319143045+02'30'";
    let dt = parse_pdf_date(pdf_date).unwrap();

    assert_eq!(dt.date.year, 2025);
    assert_eq!(dt.date.month, 3);
    assert_eq!(dt.date.day, 19);
    assert_eq!(dt.time.hour, 14);
    assert_eq!(dt.time.minute, 30);
    assert_eq!(dt.time.second, 45);
    assert_eq!(dt.offset.hours, 2);
    assert_eq!(dt.offset.minutes, 30);
}

#[test]
fn test_epoch() {
    let epoch = DateTime::epoch();

    assert_eq!(epoch.date.year, 1970);
    assert_eq!(epoch.date.month, 1);
    assert_eq!(epoch.date.day, 1);
    assert_eq!(epoch.time.hour, 0);
    assert_eq!(epoch.time.minute, 0);
    assert_eq!(epoch.time.second, 0);
    assert_eq!(epoch.time.millisecond, 0);
    assert_eq!(epoch.offset.hours, 0);
    assert_eq!(epoch.offset.minutes, 0);
    assert_eq!(epoch.offset.seconds, 0);
}

#[test]
fn test_month_conversion() {
    assert_eq!(u8::from(Month::January), 1);
    assert_eq!(u8::from(Month::February), 2);
    assert_eq!(u8::from(Month::December), 12);

    assert_eq!(Month::from(1u8), Month::January);
    assert_eq!(Month::from(5u8), Month::May);
    assert_eq!(Month::from(12u8), Month::December);

    // Test invalid month defaults to January
    assert_eq!(Month::from(13u8), Month::January);
    assert_eq!(Month::from(0u8), Month::January);
}

#[test]
fn test_utc_offset() {
    let offset = UtcOffset {
        hours: 2,
        minutes: 30,
        seconds: 15,
    };

    assert_eq!(offset.whole_hours(), 2);
    assert_eq!(offset.minutes_past_hour(), 30);
    assert_eq!(offset.seconds_past_minute(), 15);
    assert_eq!(offset.is_negative(), false);

    let negative_offset = UtcOffset {
        hours: -5,
        minutes: 0,
        seconds: 0,
    };

    assert_eq!(negative_offset.whole_hours(), -5);
    assert_eq!(negative_offset.is_negative(), true);

    // Test from_hms constructor
    let created_offset = UtcOffset::from_hms(2, 30, 15).unwrap();
    assert_eq!(created_offset, offset);
}

#[test]
fn test_edge_cases() {
    // Leap year
    let leap_date = Date {
        year: 2024,
        month: 2,
        day: 29,
    };
    let serialized = serde_json::to_string(&leap_date).unwrap();
    let deserialized: Date = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized, leap_date);

    // Midnight rollover
    let dt = DateTime {
        date: Date {
            year: 2025,
            month: 12,
            day: 31,
        },
        time: Time {
            hour: 23,
            minute: 59,
            second: 59,
            millisecond: 999,
        },
        offset: Offset {
            hours: 0,
            minutes: 0,
            seconds: 0,
            milliseconds: 0,
        },
    };
    let serialized = serde_json::to_string(&dt).unwrap();
    let deserialized: DateTime = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized, dt);

    // Error handling: invalid date
    let invalid_date_str = "2025-02-30 12:00:00 +00:00:00";
    let result = DateTime::from_str(invalid_date_str);
    assert_eq!(result, Err("Invalid date: 2025-02-30".to_string()));

    // Error handling: invalid time
    let invalid_time_str = "2025-01-01 25:00:00 +00:00:00";
    let result = DateTime::from_str(invalid_time_str);
    assert_eq!(result, Err("Invalid hour: 25".to_string()));

    // Error handling: invalid PDF date
    let invalid_pdf_date = "D:202503";
    let result = parse_pdf_date(invalid_pdf_date);
    assert!(result.is_err());
}
