use super::*;

pub(super) const JS_MAX_TIME_MILLIS: i64 = 8_640_000_000_000_000;

#[must_use]
pub(super) fn js_date_from_unix_millis(milliseconds: i64) -> Option<OffsetDateTime> {
    (-JS_MAX_TIME_MILLIS..=JS_MAX_TIME_MILLIS)
        .contains(&milliseconds)
        .then(|| OffsetDateTime::from_unix_millis(milliseconds, UtcOffset::UTC))
}

#[derive(Debug, Clone)]
enum DayjsFormatItem {
    Literal(String),
    Token(DayjsToken),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DayjsToken {
    Year4,
    Year2,
    Month2,
    Month1,
    MonthNameShort,
    MonthNameLong,
    Day2,
    Day1,
    DayOrdinal,
    Hour24_2,
    Hour24_1,
    Hour12_2,
    Hour12_1,
    Minute2,
    Minute1,
    Second2,
    Second1,
    Millis3,
    Millis2,
    Millis1,
    OffsetColon,
    OffsetNoColon,
    AmPmUpper,
    AmPmLower,
    UnixMs,
    UnixSec,
    WeekdayLong,
    WeekdayShort,
}

#[must_use]
fn tokenize_dayjs_format(fmt: &str) -> Vec<DayjsFormatItem> {
    let mut out: Vec<DayjsFormatItem> = Vec::new();

    fn push_lit(out: &mut Vec<DayjsFormatItem>, s: &str) {
        if s.is_empty() {
            return;
        }

        match out.last_mut() {
            Some(DayjsFormatItem::Literal(prev)) => prev.push_str(s),
            _ => out.push(DayjsFormatItem::Literal(s.to_string())),
        }
    }

    let tokens: &[(&str, DayjsToken)] = &[
        ("YYYY", DayjsToken::Year4),
        ("MMMM", DayjsToken::MonthNameLong),
        ("MMM", DayjsToken::MonthNameShort),
        ("MM", DayjsToken::Month2),
        ("M", DayjsToken::Month1),
        ("Do", DayjsToken::DayOrdinal),
        ("DD", DayjsToken::Day2),
        ("D", DayjsToken::Day1),
        ("HH", DayjsToken::Hour24_2),
        ("H", DayjsToken::Hour24_1),
        ("hh", DayjsToken::Hour12_2),
        ("h", DayjsToken::Hour12_1),
        ("mm", DayjsToken::Minute2),
        ("m", DayjsToken::Minute1),
        ("ss", DayjsToken::Second2),
        ("s", DayjsToken::Second1),
        ("SSS", DayjsToken::Millis3),
        ("SS", DayjsToken::Millis2),
        ("S", DayjsToken::Millis1),
        ("ZZ", DayjsToken::OffsetNoColon),
        ("Z", DayjsToken::OffsetColon),
        ("A", DayjsToken::AmPmUpper),
        ("a", DayjsToken::AmPmLower),
        ("x", DayjsToken::UnixMs),
        ("X", DayjsToken::UnixSec),
        ("dddd", DayjsToken::WeekdayLong),
        ("ddd", DayjsToken::WeekdayShort),
        ("YY", DayjsToken::Year2),
    ];

    let bytes = fmt.as_bytes();
    let mut i = 0usize;

    while i < bytes.len() {
        if bytes[i] == b'['
            && let Some(end_rel) = fmt[i + 1..].find(']')
        {
            let inside = &fmt[i + 1..i + 1 + end_rel];
            push_lit(&mut out, inside);
            i = i + 1 + end_rel + 1;
            continue;
        }

        let rest = {
            let optional_rest = fmt.get(i..);

            match optional_rest {
                Some(matched_rest) => matched_rest,
                ::core::option::Option::None => break,
            }
        };

        let mut matched: Option<(&str, DayjsToken)> = None;

        'inner_for_2: for (pat, tok) in tokens {
            if rest.starts_with(pat) {
                matched = Some((*pat, *tok));
                break 'inner_for_2;
            }
        }

        if let Some((pat, tok)) = matched {
            out.push(DayjsFormatItem::Token(tok));
            i += pat.len();
        } else {
            let ch = {
                let optional_ch = rest.chars().next();

                match optional_ch {
                    Some(matched_ch) => matched_ch,
                    ::core::option::Option::None => break,
                }
            };

            push_lit(&mut out, &ch.to_string());
            i += ch.len_utf8();
        }
    }

    out
}

#[derive(Debug, Clone, Default)]
struct DayjsParsedParts {
    year: Option<i32>,
    month: Option<u32>,
    day: Option<u32>,
    hour24: Option<u32>,
    hour12: Option<u32>,
    minute: Option<u32>,
    second: Option<u32>,
    millis: Option<u32>,
    ampm_is_pm: Option<bool>,
    offset_minutes: Option<i32>,
    unix_ms: Option<i64>,
}




#[must_use]
fn split_at_char_boundary(s: &str, byte_idx: usize) -> (&str, &str) {
    let mut idx = byte_idx.min(s.len());

    while idx > 0 && !s.is_char_boundary(idx) {
        idx -= 1;
    }

    s.split_at(idx)
}

#[must_use]
pub(super) fn parse_dayjs_like_strict(date_format: &str, s: &str) -> Option<OffsetDateTime> {
    let fmt = date_format.trim();

    if fmt.is_empty() {
        return None;
    }

    let items = tokenize_dayjs_format(fmt);

    #[must_use]
    fn parse_signed_i64_prefix(input: &str) -> Option<(i64, &str)> {
        let bytes = input.as_bytes();

        match bytes {
            [] => {
                return None;
            },
            [_, ..] => {},
        }

        let mut i = 0usize;
        let sign: i64 = if bytes[0] == b'-' {
            i = 1;
            -1
        } else {
            1
        };
        let start_digits = i;

        while match <[_]>::get(&*bytes, i) { ::core::option::Option::Some(&checked_element) => checked_element.is_ascii_digit(), ::core::option::Option::None => false } {
            i += 1;
        }

        match { let _explicit_ordering = ::core::cmp::Ord::cmp(&::core::convert::identity(i), &::core::convert::identity(start_digits)); _explicit_ordering } {
            ::core::cmp::Ordering::Equal => {
                return None;
            },
            ::core::cmp::Ordering::Less | ::core::cmp::Ordering::Greater => {},
        }

        let v: i64 = match input[start_digits..i].parse() { ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__), ::core::result::Result::Err(_invalid_integer) => ::core::option::Option::None }?;
        let checked_value = sign.checked_mul(v)?;
        Some((checked_value, &input[i..]))
    }

    #[must_use]
    fn parse_int_exact(s: &str, digits: usize) -> Option<(u32, &str)> {
        if s.len() < digits {
            return None;
        }

        let (head, tail) = split_at_char_boundary(s, digits);

        if head.len() != digits || !head.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }

        let v = match head.parse() { ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__), ::core::result::Result::Err(_invalid_integer) => ::core::option::Option::None }?;
        Some((v, tail))
    }

    #[must_use]
    fn parse_int_var(s: &str, min: usize, max: usize) -> Vec<(u32, &str)> {
        let mut out = Vec::new();

        for digits in (min..=max).rev() {
            if let Some((v, tail)) = parse_int_exact(s, digits) {
                out.push((v, tail));
            }
        }

        out
    }

    #[must_use]
    fn parse_offset(s: &str, with_colon: bool) -> Option<(i32, &str)> {
        let s = s.strip_prefix(|c| c == ' ' || c == '\t').unwrap_or(s);

        if let Some(tail) = s.strip_prefix('Z') {
            return Some((0, tail));
        }

        let (sign, rest) = if let Some(tail) = s.strip_prefix('+') {
            (1i32, tail)
        } else {
            (-1i32, { let checked_value = s.strip_prefix('-')?; checked_value })
        };

        let (hh, rest) = parse_int_exact(rest, 2)?;
        let (mm, rest) = if with_colon {
            let rest = rest.strip_prefix(':')?;
            parse_int_exact(rest, 2)?
        } else {
            parse_int_exact(rest, 2)?
        };
        let hh: i32 = match hh.try_into() { ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__), ::core::result::Result::Err(_outside_integer_range) => ::core::option::Option::None }?;
        let mm: i32 = match mm.try_into() { ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__), ::core::result::Result::Err(_outside_integer_range) => ::core::option::Option::None }?;

        if hh > 23 || mm > 59 {
            return None;
        }

        Some((sign * (hh * 60 + mm), rest))
    }

    #[must_use]
    fn parse_month_name(s: &str) -> Option<(u32, &str)> {
        const MONTHS: [&str; 12] = [
            "january",
            "february",
            "march",
            "april",
            "may",
            "june",
            "july",
            "august",
            "september",
            "october",
            "november",
            "december",
        ];
        const MONTHS_SHORT: [&str; 12] = [
            "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
        ];

        let lower = s.to_lowercase();

        for (i, name) in MONTHS.iter().enumerate() {
            if lower.starts_with(name) {
                let tail = &s[name.len()..];
                return Some(((i as u32) + 1, tail));
            }
        }

        for (i, name) in MONTHS_SHORT.iter().enumerate() {
            if lower.starts_with(name) {
                let tail = &s[name.len()..];
                return Some(((i as u32) + 1, tail));
            }
        }

        None
    }

    #[must_use]
    fn parse_weekday_name(s: &str) -> Option<&str> {
        const DAYS: [&str; 7] = [
            "sunday",
            "monday",
            "tuesday",
            "wednesday",
            "thursday",
            "friday",
            "saturday",
        ];
        const DAYS_SHORT: [&str; 7] = ["sun", "mon", "tue", "wed", "thu", "fri", "sat"];

        let lower = s.to_lowercase();

        for name in DAYS {
            if lower.starts_with(name) {
                return Some(&s[name.len()..]);
            }
        }

        for name in DAYS_SHORT {
            if lower.starts_with(name) {
                return Some(&s[name.len()..]);
            }
        }

        None
    }

    #[must_use]
    fn parse_day_ordinal(s: &str) -> Option<(u32, &str)> {
        let candidates = parse_int_var(s, 1, 2);

        for (day, tail) in candidates {
            let tail_lower = tail.to_lowercase();

            for suffix in ["st", "nd", "rd", "th"] {
                if tail_lower.starts_with(suffix) {
                    return Some((day, &tail[suffix.len()..]));
                }
            }
        }

        None
    }

    #[must_use]
    fn parse_ampm(s: &str) -> Option<(bool, &str)> {
        let lower = s.to_lowercase();

        if lower.starts_with("am") {
            return Some((false, &s[2..]));
        }

        if lower.starts_with("pm") {
            return Some((true, &s[2..]));
        }

        None
    }

    #[must_use]
    fn parse_items<'a>(
        items: &[DayjsFormatItem],
        input: &'a str,
        parts: &DayjsParsedParts,
    ) -> Option<(&'a str, DayjsParsedParts)> {
        match items {
            [] => {
                return Some((input, parts.clone()));
            },
            [_, ..] => {},
        }

        match &items[0] {
            DayjsFormatItem::Literal(lit) => {
                let input = input.strip_prefix(lit.as_str())?;
                parse_items(&items[1..], input, parts)
            }
            DayjsFormatItem::Token(tok) => match tok {
                DayjsToken::Year4 => {
                    let (y, rest) = parse_int_exact(input, 4)?;
                    let mut next = parts.clone();
                    next.year = Some(::core::primitive::u32::cast_signed(y));
                    parse_items(&items[1..], rest, &next)
                }
                DayjsToken::Year2 => {
                    let (y2, rest) = parse_int_exact(input, 2)?;
                    let y2 = ::core::primitive::u32::cast_signed(y2);
                    let year = match { let _explicit_ordering = ::core::cmp::Ord::cmp(&::core::convert::identity(y2), &::core::convert::identity(68)); _explicit_ordering } {
                        ::core::cmp::Ordering::Less | ::core::cmp::Ordering::Equal => { 2000 + y2 },
                        ::core::cmp::Ordering::Greater => { 1900 + y2 },
                    };
                    let mut next = parts.clone();
                    next.year = Some(year);
                    parse_items(&items[1..], rest, &next)
                }
                DayjsToken::Month2 => {
                    let (m, rest) = parse_int_exact(input, 2)?;

                    if !(1..=12).contains(&m) {
                        return None;
                    }

                    let mut next = parts.clone();
                    next.month = Some(m);
                    parse_items(&items[1..], rest, &next)
                }
                DayjsToken::Month1 => {
                    for (m, rest) in parse_int_var(input, 1, 2) {
                        if !(1..=12).contains(&m) {
                            continue;
                        }

                        let mut next = parts.clone();
                        next.month = Some(m);

                        if let Some(r) = parse_items(&items[1..], rest, &next) {
                            return Some(r);
                        }
                    }

                    None
                }
                DayjsToken::MonthNameShort | DayjsToken::MonthNameLong => {
                    let (m, rest) = parse_month_name(input)?;
                    let mut next = parts.clone();
                    next.month = Some(m);
                    parse_items(&items[1..], rest, &next)
                }
                DayjsToken::Day2 => {
                    let (d, rest) = parse_int_exact(input, 2)?;

                    if !(1..=31).contains(&d) {
                        return None;
                    }

                    let mut next = parts.clone();
                    next.day = Some(d);
                    parse_items(&items[1..], rest, &next)
                }
                DayjsToken::Day1 => {
                    for (d, rest) in parse_int_var(input, 1, 2) {
                        if !(1..=31).contains(&d) {
                            continue;
                        }

                        let mut next = parts.clone();
                        next.day = Some(d);

                        if let Some(r) = parse_items(&items[1..], rest, &next) {
                            return Some(r);
                        }
                    }

                    None
                }
                DayjsToken::DayOrdinal => {
                    let (d, rest) = parse_day_ordinal(input)?;

                    if !(1..=31).contains(&d) {
                        return None;
                    }

                    let mut next = parts.clone();
                    next.day = Some(d);
                    parse_items(&items[1..], rest, &next)
                }
                DayjsToken::Hour24_2 => {
                    let (h, rest) = parse_int_exact(input, 2)?;

                    if h > 23 {
                        return None;
                    }

                    let mut next = parts.clone();
                    next.hour24 = Some(h);
                    parse_items(&items[1..], rest, &next)
                }
                DayjsToken::Hour24_1 => {
                    for (h, rest) in parse_int_var(input, 1, 2) {
                        if h > 23 {
                            continue;
                        }

                        let mut next = parts.clone();
                        next.hour24 = Some(h);

                        if let Some(r) = parse_items(&items[1..], rest, &next) {
                            return Some(r);
                        }
                    }

                    None
                }
                DayjsToken::Hour12_2 => {
                    let (h, rest) = parse_int_exact(input, 2)?;

                    if !(1..=12).contains(&h) {
                        return None;
                    }

                    let mut next = parts.clone();
                    next.hour12 = Some(h);
                    parse_items(&items[1..], rest, &next)
                }
                DayjsToken::Hour12_1 => {
                    for (h, rest) in parse_int_var(input, 1, 2) {
                        if !(1..=12).contains(&h) {
                            continue;
                        }

                        let mut next = parts.clone();
                        next.hour12 = Some(h);

                        if let Some(r) = parse_items(&items[1..], rest, &next) {
                            return Some(r);
                        }
                    }

                    None
                }
                DayjsToken::Minute2 => {
                    let (m, rest) = parse_int_exact(input, 2)?;

                    if m > 59 {
                        return None;
                    }

                    let mut next = parts.clone();
                    next.minute = Some(m);
                    parse_items(&items[1..], rest, &next)
                }
                DayjsToken::Minute1 => {
                    for (m, rest) in parse_int_var(input, 1, 2) {
                        if m > 59 {
                            continue;
                        }

                        let mut next = parts.clone();
                        next.minute = Some(m);

                        if let Some(r) = parse_items(&items[1..], rest, &next) {
                            return Some(r);
                        }
                    }

                    None
                }
                DayjsToken::Second2 => {
                    let (sec, rest) = parse_int_exact(input, 2)?;

                    if sec > 59 {
                        return None;
                    }

                    let mut next = parts.clone();
                    next.second = Some(sec);
                    parse_items(&items[1..], rest, &next)
                }
                DayjsToken::Second1 => {
                    for (sec, rest) in parse_int_var(input, 1, 2) {
                        if sec > 59 {
                            continue;
                        }

                        let mut next = parts.clone();
                        next.second = Some(sec);

                        if let Some(r) = parse_items(&items[1..], rest, &next) {
                            return Some(r);
                        }
                    }

                    None
                }
                DayjsToken::Millis3 => {
                    let (ms, rest) = parse_int_exact(input, 3)?;

                    if ms > 999 {
                        return None;
                    }

                    let mut next = parts.clone();
                    next.millis = Some(ms);
                    parse_items(&items[1..], rest, &next)
                }
                DayjsToken::Millis2 => {
                    let (ms, rest) = parse_int_exact(input, 2)?;

                    if ms > 99 {
                        return None;
                    }

                    let mut next = parts.clone();
                    next.millis = Some(ms * 10);
                    parse_items(&items[1..], rest, &next)
                }
                DayjsToken::Millis1 => {
                    let (ms, rest) = parse_int_exact(input, 1)?;

                    if ms > 9 {
                        return None;
                    }

                    let mut next = parts.clone();
                    next.millis = Some(ms * 100);
                    parse_items(&items[1..], rest, &next)
                }
                DayjsToken::OffsetColon => {
                    let (mins, rest) = parse_offset(input, true)?;
                    let mut next = parts.clone();
                    next.offset_minutes = Some(mins);
                    parse_items(&items[1..], rest, &next)
                }
                DayjsToken::OffsetNoColon => {
                    let (mins, rest) = parse_offset(input, false)?;
                    let mut next = parts.clone();
                    next.offset_minutes = Some(mins);
                    parse_items(&items[1..], rest, &next)
                }
                DayjsToken::AmPmUpper | DayjsToken::AmPmLower => {
                    let (is_pm, rest) = parse_ampm(input)?;
                    let mut next = parts.clone();
                    next.ampm_is_pm = Some(is_pm);
                    parse_items(&items[1..], rest, &next)
                }
                DayjsToken::UnixMs => {
                    let (ms, rest) = parse_signed_i64_prefix(input)?;
                    let mut next = parts.clone();
                    next.unix_ms = Some(ms);
                    parse_items(&items[1..], rest, &next)
                }
                DayjsToken::UnixSec => {
                    let (sec, rest) = parse_signed_i64_prefix(input)?;
                    let mut next = parts.clone();
                    let checked_value = sec.checked_mul(1000)?;
                    next.unix_ms = Some(checked_value);
                    parse_items(&items[1..], rest, &next)
                }
                DayjsToken::WeekdayLong | DayjsToken::WeekdayShort => {
                    let rest = parse_weekday_name(input)?;
                    parse_items(&items[1..], rest, parts)
                }
            },
        }
    }

    let parts = DayjsParsedParts::default();
    let (rest, parts_2) = parse_items(&items, s, &parts)?;

    if !rest.is_empty() {
        return None;
    }

    if let Some(ms) = parts_2.unix_ms {
        return js_date_from_unix_millis(ms);
    }

    let base_date = crate::runtime::today_local();






    let year = parts_2.year.unwrap_or(base_date.year());
    let month = match (parts_2.month, parts_2.year) {
        (Some(m), _) => m,
        (None, Some(_)) => 1,
        (None, None) => base_date.month(),
    };
    let day = match (parts_2.day, parts_2.month, parts_2.year) {
        (Some(d), _, _) => d,
        (None, Some(_), _) => 1,
        (None, None, Some(_)) => 1,
        (None, None, None) => base_date.day(),
    };

    let mut hour = match parts_2.hour24 {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => 0,
    };

    if parts_2.hour24.is_none()
        && let Some(h12) = parts_2.hour12
    {
        let mut h = ::core::primitive::u32::wrapping_rem(h12, 12);

        if match parts_2.ampm_is_pm {
            ::core::option::Option::Some(present) => present,
            ::core::option::Option::None => false,
        } {
            h += 12;
        }

        hour = h;
    }

    let minute = match parts_2.minute {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => 0,
    };
    let second = match parts_2.second {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => 0,
    };
    let millis = match parts_2.millis {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => 0,
    };

    let checked_value = CivilDate::new(year, month, day)?;
    let local = checked_value.at_hms_milli(hour, minute, second, millis)?;

    if let Some(mins) = parts_2.offset_minutes {
        OffsetDateTime::from_local(local, { let checked_value_2 = UtcOffset::from_minutes(mins)?; checked_value_2 })
    } else {
        local_from_civil(local)
    }
}

pub(super) fn parse_js_date_fallback(s: &str) -> Result<OffsetDateTime> {
    let s_2 = s.trim();

    if let Some(dt) = parse_js_like_ymd_datetime(s_2).or_else(|| parse_js_like_mdy_hm_datetime(s_2)) {
        let year = dt.year();

        if !(-10000..=10000).contains(&year) {
            return Err(Error::diagram_parse_fallback(
                "gantt".to_string(),
                format!("Invalid date:{s}", s = s_2),
            ));
        }

        return Ok(dt);
    }

    if is_ascii_digits(s_2) {
        let n: i32 = s_2.parse().map_err(|_| {
            Error::diagram_parse_fallback("gantt".to_string(), format!("Invalid date:{s}", s = s_2))
        })?;
        let year = if s_2.len() <= 2 { 2000 + n } else { n };

        if !(-10000..=10000).contains(&year) {
            return Err(Error::diagram_parse_fallback(
                "gantt".to_string(),
                format!("Invalid date:{s}", s = s_2),
            ));
        }

        let date = CivilDate::new(year, 1, 1).ok_or_else(|| {
            Error::diagram_parse_fallback("gantt".to_string(), format!("Invalid date:{s}", s = s_2))
        })?;
        return local_from_civil(date.at_midnight()).ok_or_else(|| {
            Error::diagram_parse_fallback("gantt".to_string(), format!("Invalid date:{s}", s = s_2))
        });
    }

    Err(Error::diagram_parse_fallback(
        "gantt".to_string(),
        format!("Invalid date:{s}", s = s_2),
    ))
}

#[must_use]
fn parse_js_like_ymd_datetime(s: &str) -> Option<OffsetDateTime> {
    #[must_use]
    fn parse_u32(s: &str) -> Option<u32> {
        if s.is_empty() || !s.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }

        match s.parse() { ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__), ::core::result::Result::Err(_invalid_integer) => ::core::option::Option::None }
    }

    #[must_use]
    fn split_once(s: &str, ch: char) -> Option<(&str, &str)> {
        let idx = s.find(ch)?;
        Some((&s[..idx], &s[idx + 1..]))
    }

    #[must_use]
    fn parse_timezone_offset_minutes(s: &str) -> Option<(i32, &str)> {
        let s = s.trim_start();

        if let Some(rest) = s.strip_prefix(['Z', 'z']) {
            return Some((0, rest));
        }

        let (sign, rest) = if let Some(rest) = s.strip_prefix('+') {
            (1i32, rest)
        } else {
            (-1i32, { let checked_value = s.strip_prefix('-')?; checked_value })
        };

        let (hh_str, rest) = split_at_char_boundary(rest, 2);
        let checked_value_2 = parse_u32(hh_str)?;
        let hh = ::core::primitive::u32::cast_signed(checked_value_2);

        let (mm, rest) = if let Some(rest) = rest.strip_prefix(':') {
            let (mm_str, rest) = split_at_char_boundary(rest, 2);
            let checked_value_3 = parse_u32(mm_str)?;
            (::core::primitive::u32::cast_signed(checked_value_3), rest)
        } else {
            let (mm_str, rest) = split_at_char_boundary(rest, 2);
            let checked_value_4 = parse_u32(mm_str)?;
            (::core::primitive::u32::cast_signed(checked_value_4), rest)
        };

        if hh > 23 || mm > 59 {
            return None;
        }

        Some((sign * (hh * 60 + mm), rest))
    }

    #[must_use]
    fn js_date_only_is_iso_utc(year_str: &str, month_str: &str, day_str: &str) -> bool {


        let year_digits = year_str.strip_prefix(['+', '-']).unwrap_or(year_str);
        let year_is_iso = if year_str.starts_with(['+', '-']) {
            year_digits.len() == 6
        } else {
            year_digits.len() == 4
        };
        year_is_iso && month_str.len() == 2 && day_str.len() == 2
    }

    let (date_part, mut rest) = {
        let mut end = s.len();

        for (i, c) in s.char_indices() {
            if c == 'T' || c == 't' || c.is_whitespace() {
                end = i;
                break;
            }
        }

        (&s[..end], &s[end..])
    };

    let sep = if date_part.contains('-') {
        '-'
    } else if date_part.contains('/') {
        '/'
    } else {
        return None;
    };

    let (year_month, day_str) = date_part.rsplit_once(sep)?;
    let (year_str, month_str) = year_month.rsplit_once(sep)?;
    let year_digits = year_str.strip_prefix(['+', '-']).unwrap_or(year_str);
    let signed_year = year_str.starts_with(['+', '-']);
    let valid_year_shape = if signed_year {
        sep == '-' && year_digits.len() == 6
    } else {
        (1..=4).contains(&year_digits.len())
    };

    if !valid_year_shape {
        return None;
    }

    if !year_digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }



    if year_str == "-000000" {
        return None;
    }

    let year: i32 = match year_str.parse() { ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__), ::core::result::Result::Err(_invalid_integer) => ::core::option::Option::None }?;
    let month = parse_u32(month_str)?;
    let day = parse_u32(day_str)?;
    let date = CivilDate::new(year, month, day)?;

    let mut second: u32 = 0;
    let mut millis: u32 = 0;
    let mut tz_minutes: Option<i32> = None;

    rest = rest.trim_start();

    if rest.is_empty() {
        let local = date.at_midnight();

        if sep == '-' && js_date_only_is_iso_utc(year_str, month_str, day_str) {
            return OffsetDateTime::from_local(local, UtcOffset::UTC);
        }

        return local_from_civil(local);
    }

    if let Some(r) = rest.strip_prefix(['T', 't']) {
        rest = r;
    }

    rest = rest.trim_start();

    let (hh_str, rest2) = split_once(rest, ':')?;
    let hour = parse_u32(hh_str)?;
    let (mm_str, mut rest3) = {
        let (mm_str, rest_2) = split_at_char_boundary(rest2, 2);
        (mm_str, rest_2)
    };
    let minute = parse_u32(mm_str)?;

    if let Some(r) = rest3.strip_prefix(':') {
        let (ss_str, mut rest4) = {
            let (ss_str, rest_3) = split_at_char_boundary(r, 2);
            (ss_str, rest_3)
        };
        let checked_value = parse_u32(ss_str)?;
        second = checked_value;

        if let Some(r_2) = rest4.strip_prefix('.') {
            let fraction_len = r_2.bytes().take_while(u8::is_ascii_digit).count();
            let ms_digits: String = r_2.chars().take(fraction_len.min(3)).collect();

            if ms_digits.is_empty() {
                return None;
            }

            millis = match ms_digits.len() {
                1 => parse_u32(&ms_digits)? * 100,
                2 => parse_u32(&ms_digits)? * 10,
                _ => parse_u32(&ms_digits)?,
            };
            rest4 = &r_2[fraction_len..];
        }

        rest3 = rest4;
    }

    rest3 = rest3.trim_start();

    if !rest3.is_empty() {
        if let Some((mins, tail)) = parse_timezone_offset_minutes(rest3) {
            tz_minutes = Some(mins);

            if !tail.trim().is_empty() {
                return None;
            }
        } else {
            return None;
        }
    }

    if hour > 23 || minute > 59 || second > 59 {
        return None;
    }

    let local = date.at_hms_milli(hour, minute, second, millis)?;

    if let Some(mins) = tz_minutes {
        return OffsetDateTime::from_local(local, { let checked_value_2 = UtcOffset::from_minutes(mins)?; checked_value_2 });
    }

    local_from_civil(local)
}

#[must_use]
fn parse_js_like_mdy_hm_datetime(s: &str) -> Option<OffsetDateTime> {







    let s_2 = s.trim();
    let mut parts = s_2.splitn(4, '-');
    let month_str = parts.next()?;
    let day_str = parts.next()?;
    let year_str = parts.next()?;
    let time_str = parts.next()?;

    #[must_use]
    fn parse_u32(s: &str) -> Option<u32> {
        if s.is_empty() || !s.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }

        match s.parse() { ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__), ::core::result::Result::Err(_invalid_integer) => ::core::option::Option::None }
    }

    let month = parse_u32(month_str)?;
    let day = parse_u32(day_str)?;

    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }

    let year_raw: i32 = match year_str.parse() { ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__), ::core::result::Result::Err(_invalid_integer) => ::core::option::Option::None }?;
    let year = if year_str.len() == 2 {
        if (0..=49).contains(&year_raw) {
            2000 + year_raw
        } else if (50..=99).contains(&year_raw) {
            1900 + year_raw
        } else {
            return None;
        }
    } else {
        year_raw
    };

    let (hour_str, rest) = time_str.split_once(':')?;
    let minute_str = rest.get(..2).unwrap_or(rest);
    let rest_2 = rest.get(2..).unwrap_or("");
    let minute = parse_u32(minute_str)?;
    let hour = parse_u32(hour_str)?;

    if hour > 23 || minute > 59 {
        return None;
    }

    let (second, millis) = if let Some(rest_3) = rest_2.strip_prefix(':') {
        let second_str = rest_3.get(..2).unwrap_or(rest_3);
        let rest_4 = rest_3.get(2..).unwrap_or("");
        let second = parse_u32(second_str)?;

        if second > 59 {
            return None;
        }

        let millis = if let Some(rest_5) = rest_4.strip_prefix('.') {
            let ms_str = rest_5
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .collect::<String>();

            if ms_str.is_empty() {
                0
            } else {
                let ms = match parse_u32(&ms_str) {
                    ::core::option::Option::Some(option_value) => option_value,
                    ::core::option::Option::None => 0,
                };
                ms.min(999)
            }
        } else {
            0
        };
        (second, millis)
    } else {
        (0u32, 0u32)
    };

    let checked_value = CivilDate::new(year, month, day)?;
    let local = checked_value.at_hms_milli(hour, minute, second, millis)?;
    local_from_civil(local)
}

#[must_use]
fn is_ascii_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

#[must_use]
fn is_js_regex_whitespace(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{000B}' | '\u{000C}' | '\r' | ' ' | '\u{00A0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200A}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202F}'
                | '\u{205F}'
                | '\u{3000}'
                | '\u{FEFF}'
    )
}

#[must_use]
fn is_gantt_ref_id_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || b == b' '
}

#[must_use]
fn gantt_ref_id_prefix_len(s: &str) -> usize {
    s.bytes().take_while(|b| is_gantt_ref_id_byte(*b)).count()
}

#[must_use]
pub(super) fn relative_ref_ids_range(s: &str, keyword: &str) -> Option<std::ops::Range<usize>> {
    let rest = s.strip_prefix(keyword)?;
    let rest_start = keyword.len();

    let ws_len: usize = rest
        .chars()
        .take_while(|c| is_js_regex_whitespace(*c))
        .map(char::len_utf8)
        .sum();

    if ws_len == 0 {
        return None;
    }

    let mut start = ws_len;

    loop {
        let refs = &rest[start..];
        let id_len = gantt_ref_id_prefix_len(refs);

        if id_len > 0 {
            let start_2 = rest_start + start;
            return Some(start_2..start_2 + id_len);
        }

        let (previous_start, _) = rest[..start].char_indices().next_back()?;

        match previous_start {
            0 => {
                return None;
            },
            _ => {},
        }

        start = previous_start;
    }
}

#[must_use]
pub(super) fn relative_ref_ids<'a>(s: &'a str, keyword: &str) -> Option<&'a str> {
    let range = relative_ref_ids_range(s, keyword)?;
    Some(&s[range])
}

pub(super) fn get_start_date(
    db: &GanttDb,
    date_format: &str,
    raw: &str,
) -> Result<Option<OffsetDateTime>> {
    let s = raw.trim();

    if let Some(ids) = relative_ref_ids(s, "after") {
        let mut latest: Option<Option<OffsetDateTime>> = None;

        for id in ids.split(' ') {
            let id_2 = id.trim();

            if id_2.is_empty() {
                continue;
            }

            let task = {
                let optional_task = db.find_task_by_id(id_2);

                match optional_task {
                    Some(matched_task_2) => matched_task_2,
                    ::core::option::Option::None => continue,
                }
            };

            if latest.is_none() {
                latest = Some(task.end_time);
                continue;
            }

            let current_best = match latest {
                Some(matched_current_best) => matched_current_best,
                ::core::option::Option::None => continue,
            };

            let (task_end, best_end) = {
                let outcome_task_end = (task.end_time, current_best);

                match outcome_task_end {
                    (Some(matched_task_end), Some(matched_best_end)) => (matched_task_end, matched_best_end),
                    (::core::option::Option::None, _)
                    | (_, ::core::option::Option::None) => continue,
                }
            };

            if task_end > best_end {
                latest = Some(Some(task_end));
            }
        }

        return Ok(match latest {
            Some(end) => end,
            None => today_midnight_local(),
        });
    }




    let fmt = date_format.trim();

    if (fmt == "x" || fmt == "X")
        && is_ascii_digits(s)
        && let Ok(ms) = s.parse::<i64>()
        && let Some(dt) = js_date_from_unix_millis(ms)
    {
        return Ok(Some(dt));
    }

    if let Some(dt) = parse_dayjs_like_strict(date_format, s) {
        return Ok(Some(dt));
    }

    let dt = parse_js_date_fallback(s)?;
    let year = dt.year();

    if !(-10000..=10000).contains(&year) {
        return Err(Error::diagram_parse_fallback(
            "gantt".to_string(),
            format!("Invalid date:{s}"),
        ));
    }

    Ok(Some(dt))
}

#[must_use]
pub(super) fn parse_duration(str_: &str) -> (f64, String) {
    let s = str_.trim();
    let bytes = s.as_bytes();

    let mut end = 0usize;

    while match <[_]>::get(&*bytes, end) { ::core::option::Option::Some(&checked_element) => checked_element.is_ascii_digit(), ::core::option::Option::None => false } {
        end += 1;
    }

    match end {
        0 => {
            return (f64::NAN, "ms".to_string());
        },
        _ => {},
    }

    if bytes.get(end) == Some(&b'.') {
        end += 1;
        let fraction_start = end;

        while match <[_]>::get(&*bytes, end) { ::core::option::Option::Some(&checked_element) => checked_element.is_ascii_digit(), ::core::option::Option::None => false } {
            end += 1;
        }

        match { let _explicit_ordering = ::core::cmp::Ord::cmp(&::core::convert::identity(end), &::core::convert::identity(fraction_start)); _explicit_ordering } {
            ::core::cmp::Ordering::Equal => {
                return (f64::NAN, "ms".to_string());
            },
            ::core::cmp::Ordering::Less | ::core::cmp::Ordering::Greater => {},
        }
    }

    let value = &s[..end];
    let unit = &s[end..];

    if !matches!(unit, "M" | "d" | "h" | "m" | "s" | "w" | "y" | "ms") {
        return (f64::NAN, "ms".to_string());
    }

    (value.parse().unwrap_or(f64::NAN), unit.to_string())
}

#[must_use]
fn trunc_f64_to_i64(value: f64) -> Option<i64> {
    const I64_MIN_AS_F64: f64 = -9_223_372_036_854_775_808.0;
    const I64_MAX_EXCLUSIVE_AS_F64: f64 = 9_223_372_036_854_775_808.0;

    if !value.is_finite() {
        return None;
    }

    let truncated = value.trunc();

    if !(I64_MIN_AS_F64..I64_MAX_EXCLUSIVE_AS_F64).contains(&truncated) {
        return None;
    }

    Some(truncated as i64)
}

#[must_use]
fn add_milliseconds(dt: OffsetDateTime, AddMillisecondsArguments { value, scale }: AddMillisecondsArguments) -> Option<OffsetDateTime> {
    let milliseconds = trunc_f64_to_i64(value * scale)?;
    dt.checked_add_millis(milliseconds)
}

struct AddMillisecondsArguments {
    value: f64,
    scale: f64,
}

#[must_use]
fn add_duration(dt: OffsetDateTime, value: f64, unit: &str) -> Option<OffsetDateTime> {
    if !value.is_finite() {
        return None;
    }

    match unit {
        "ms" => add_milliseconds(dt, self::AddMillisecondsArguments { value, scale: 1.0 }),
        "s" => add_milliseconds(dt, self::AddMillisecondsArguments { value, scale: 1_000.0 }),
        "m" => add_milliseconds(dt, self::AddMillisecondsArguments { value, scale: 60_000.0 }),
        "h" => add_milliseconds(dt, self::AddMillisecondsArguments { value, scale: 3_600_000.0 }),
        "d" => {
            if ::core::matches!(::core::primitive::f64::classify(value.fract()), ::core::num::FpCategory::Zero) {
                add_days_local(dt, { let checked_value = trunc_f64_to_i64(value)?; checked_value })
            } else {
                add_milliseconds(dt, self::AddMillisecondsArguments { value, scale: 86_400_000.0 })
            }
        }
        "w" => {
            if ::core::matches!(::core::primitive::f64::classify(value.fract()), ::core::num::FpCategory::Zero) {
                let checked_value_2 = trunc_f64_to_i64(value)?;
                let days = checked_value_2.checked_mul(7)?;
                add_days_local(dt, days)
            } else {
                add_milliseconds(dt, self::AddMillisecondsArguments { value, scale: 604_800_000.0 })
            }
        }
        "M" => {
            if ::core::matches!(::core::primitive::f64::classify(value.fract()), ::core::num::FpCategory::Zero) {
                add_months_local(dt, { let checked_value_3 = trunc_f64_to_i64(value)?; checked_value_3 })
            } else {
                None
            }
        }
        "y" => {
            if ::core::matches!(::core::primitive::f64::classify(value.fract()), ::core::num::FpCategory::Zero) {
                add_years_local(dt, { let checked_value_4 = trunc_f64_to_i64(value)?; checked_value_4 })
            } else {
                None
            }
        }
        _ => None,
    }
}

pub(super) fn get_end_date(
    db: &GanttDb,
    prev_time: OffsetDateTime,
    date_format: &str,
    raw: &str,
    inclusive: bool,
) -> Result<Option<OffsetDateTime>> {
    let s = raw.trim();

    if let Some(ids) = relative_ref_ids(s, "until") {
        let mut earliest: Option<Option<OffsetDateTime>> = None;

        for id in ids.split(' ') {
            let id_2 = id.trim();

            if id_2.is_empty() {
                continue;
            }

            let task = {
                let optional_task = db.find_task_by_id(id_2);

                match optional_task {
                    Some(matched_task_2) => matched_task_2,
                    ::core::option::Option::None => continue,
                }
            };

            if earliest.is_none() {
                earliest = Some(task.start_time);
                continue;
            }

            let current_best = match earliest {
                Some(matched_current_best) => matched_current_best,
                ::core::option::Option::None => continue,
            };

            let (task_start, best_start) = {
                let outcome_task_start = (task.start_time, current_best);

                match outcome_task_start {
                    (Some(matched_task_start), Some(matched_best_start)) => (matched_task_start, matched_best_start),
                    (::core::option::Option::None, _)
                    | (_, ::core::option::Option::None) => continue,
                }
            };

            if task_start < best_start {
                earliest = Some(Some(task_start));
            }
        }

        return Ok(match earliest {
            Some(start) => start,
            None => today_midnight_local(),
        });
    }

    if let Some(mut dt) = parse_dayjs_like_strict(date_format, s) {
        if inclusive {
            dt = match add_days_local(dt, 1) {
                ::core::option::Option::Some(option_value) => option_value,
                ::core::option::Option::None => dt,
            };
        }

        return Ok(Some(dt));
    }

    let (value, unit) = parse_duration(s);

    if value.is_finite()
        && let Some(new_dt) = add_duration(prev_time, value, &unit)
    {
        return Ok(Some(new_dt));
    }

    Ok(Some(prev_time))
}

#[must_use]
pub(super) fn is_strict_yyyy_mm_dd(s: &str) -> bool {
    let s_2 = s.trim();
    let bytes = s_2.as_bytes();

    if bytes.len() != 10
        || !bytes[..4].iter().all(u8::is_ascii_digit)
        || bytes[4] != b'-'
        || !bytes[5..7].iter().all(u8::is_ascii_digit)
        || bytes[7] != b'-'
        || !bytes[8..].iter().all(u8::is_ascii_digit)
    {
        return false;
    }

    s_2.parse::<CivilDate>().is_ok()
}

#[must_use]
pub(super) fn month_short_name(month: u32) -> &'static str {
    match month {
        1 => "Jan",
        2 => "Feb",
        3 => "Mar",
        4 => "Apr",
        5 => "May",
        6 => "Jun",
        7 => "Jul",
        8 => "Aug",
        9 => "Sep",
        10 => "Oct",
        11 => "Nov",
        12 => "Dec",
        _ => "",
    }
}

#[must_use]
pub(super) fn month_long_name(month: u32) -> &'static str {
    match month {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => "",
    }
}

#[must_use]
pub(super) fn ordinal_suffix(n: u32) -> &'static str {
    let n_mod_100 = ::core::primitive::u32::wrapping_rem(n, 100);

    if (11..=13).contains(&n_mod_100) {
        return "th";
    }

    match ::core::primitive::u32::wrapping_rem(n, 10) {
        1 => "st",
        2 => "nd",
        3 => "rd",
        _ => "th",
    }
}

#[must_use]
pub(super) fn format_dayjs_like(dt: OffsetDateTime, fmt: &str) -> String {
    let fmt_2 = fmt.trim();

    if fmt_2.is_empty() {
        return String::new();
    }

    let items = tokenize_dayjs_format(fmt_2);
    let local = crate::runtime::datetime_to_local(dt);
    let civil = local.local_datetime();

    let mut out = String::new();

    for item in items {
        match item {
            DayjsFormatItem::Literal(s) => out.push_str(&s),
            DayjsFormatItem::Token(tok) => match tok {
                DayjsToken::Year4 => out.push_str(&format!("{:04}", civil.year())),
                DayjsToken::Year2 => {
                    out.push_str(&format!("{:02}", (civil.year().rem_euclid(100))))
                }
                DayjsToken::Month2 => out.push_str(&format!("{:02}", civil.month())),
                DayjsToken::Month1 => out.push_str(&format!("{}", civil.month())),
                DayjsToken::MonthNameShort => out.push_str(month_short_name(civil.month())),
                DayjsToken::MonthNameLong => out.push_str(month_long_name(civil.month())),
                DayjsToken::Day2 => out.push_str(&format!("{:02}", civil.day())),
                DayjsToken::Day1 => out.push_str(&format!("{}", civil.day())),
                DayjsToken::DayOrdinal => {
                    let d = civil.day();
                    out.push_str(&format!("{d}{}", ordinal_suffix(d)));
                }
                DayjsToken::Hour24_2 => out.push_str(&format!("{:02}", civil.hour())),
                DayjsToken::Hour24_1 => out.push_str(&format!("{}", civil.hour())),
                DayjsToken::Hour12_2 => {
                    let mut h = ::core::primitive::u32::wrapping_rem(civil.hour(), 12);

                    match h {
                        0 => {
                            h = 12;
                        },
                        _ => {},
                    }

                    out.push_str(&format!("{:02}", h));
                }
                DayjsToken::Hour12_1 => {
                    let mut h = ::core::primitive::u32::wrapping_rem(civil.hour(), 12);

                    match h {
                        0 => {
                            h = 12;
                        },
                        _ => {},
                    }

                    out.push_str(&format!("{}", h));
                }
                DayjsToken::Minute2 => out.push_str(&format!("{:02}", civil.minute())),
                DayjsToken::Minute1 => out.push_str(&format!("{}", civil.minute())),
                DayjsToken::Second2 => out.push_str(&format!("{:02}", civil.second())),
                DayjsToken::Second1 => out.push_str(&format!("{}", civil.second())),
                DayjsToken::Millis3 => out.push_str(&format!("{:03}", civil.millisecond())),
                DayjsToken::Millis2 => out.push_str(&format!("{:02}", ::core::primitive::u32::wrapping_div(civil.millisecond(), 10))),
                DayjsToken::Millis1 => out.push_str(&format!("{}", ::core::primitive::u32::wrapping_div(civil.millisecond(), 100))),
                DayjsToken::OffsetColon | DayjsToken::OffsetNoColon => {
                    let secs = local.offset().seconds();
                    let sign = if secs < 0 { '-' } else { '+' };
                    let secs_2 = secs.abs();
                    let hh = ::core::primitive::i32::wrapping_div(secs_2, 3600);
                    let mm = ::core::primitive::i32::wrapping_div(::core::primitive::i32::wrapping_rem(secs_2, 3600), 60);

                    match tok {
                        DayjsToken::OffsetColon => out.push_str(&format!("{sign}{hh:02}:{mm:02}")),
                        DayjsToken::OffsetNoColon => out.push_str(&format!("{sign}{hh:02}{mm:02}")),
                        _ => {}
                    }
                }
                DayjsToken::AmPmUpper | DayjsToken::AmPmLower => {
                    let is_pm = civil.hour() >= 12;
                    let s = if is_pm { "PM" } else { "AM" };

                    match tok {
                        DayjsToken::AmPmUpper => out.push_str(s),
                        DayjsToken::AmPmLower => out.push_str(&s.to_lowercase()),
                        _ => {}
                    }
                }
                DayjsToken::UnixMs => out.push_str(&dt.timestamp_millis().to_string()),
                DayjsToken::UnixSec => out.push_str(&dt.timestamp_seconds().to_string()),
                DayjsToken::WeekdayLong => out.push_str(civil.weekday().full_name()),
                DayjsToken::WeekdayShort => out.push_str(civil.weekday().short_name()),
            },
        }
    }

    out
}
