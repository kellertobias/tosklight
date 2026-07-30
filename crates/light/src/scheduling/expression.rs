use std::{collections::BTreeSet, fmt, str::FromStr};

use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

use super::ScheduleValidationError;

/// A validated ToskLight five-field calendar expression.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CalendarExpression(String);

impl CalendarExpression {
    pub fn parse(expression: impl Into<String>) -> Result<Self, ScheduleValidationError> {
        let expression = expression.into();
        ParsedCalendarExpression::parse(&expression)?;
        Ok(Self(expression))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn parsed(&self) -> ParsedCalendarExpression {
        ParsedCalendarExpression::parse(&self.0)
            .expect("CalendarExpression can only contain a validated expression")
    }
}

impl fmt::Display for CalendarExpression {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl FromStr for CalendarExpression {
    type Err = ScheduleValidationError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl Serialize for CalendarExpression {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for CalendarExpression {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(value).map_err(de::Error::custom)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ParsedCalendarExpression {
    minutes: BTreeSet<u8>,
    hours: BTreeSet<u8>,
    days_of_month: BTreeSet<u8>,
    months: BTreeSet<u8>,
    weekdays: ParsedWeekdays,
    day_of_month_any: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ParsedWeekdays {
    Any,
    Values(BTreeSet<u8>),
    Ordinal { weekday: u8, ordinal: u8 },
    Last { weekday: u8 },
}

impl ParsedCalendarExpression {
    pub(crate) fn parse(expression: &str) -> Result<Self, ScheduleValidationError> {
        let fields = expression.split_whitespace().collect::<Vec<_>>();
        if fields.len() != 5 {
            return Err(ScheduleValidationError::new(
                "expression",
                "expected exactly five fields: minute hour day-of-month month weekday; seconds, years, and Quartz fields are unsupported",
            ));
        }
        let day_of_month_any = fields[2] == "*";
        let weekday_any = fields[4] == "*";
        if !day_of_month_any && !weekday_any {
            return Err(ScheduleValidationError::new(
                "weekday",
                "day-of-month and weekday cannot both be constrained",
            ));
        }
        let parsed = Self {
            minutes: parse_numeric_field("minute", fields[0], 0, 59)?,
            hours: parse_numeric_field("hour", fields[1], 0, 23)?,
            days_of_month: parse_numeric_field("day-of-month", fields[2], 1, 31)?,
            months: parse_numeric_field("month", fields[3], 1, 12)?,
            weekdays: parse_weekdays(fields[4])?,
            day_of_month_any,
        };
        if !parsed.has_possible_date() {
            return Err(ScheduleValidationError::new(
                "day-of-month",
                "the selected day and month fields never form a real calendar date",
            ));
        }
        Ok(parsed)
    }

    pub(crate) fn times(&self) -> impl Iterator<Item = (u8, u8)> + '_ {
        self.hours
            .iter()
            .flat_map(|hour| self.minutes.iter().map(move |minute| (*hour, *minute)))
    }

    pub(crate) fn matches_date(&self, date: NaiveDate) -> bool {
        if !self.months.contains(&(date.month() as u8)) {
            return false;
        }
        if !self.day_of_month_any && !self.days_of_month.contains(&(date.day() as u8)) {
            return false;
        }
        match &self.weekdays {
            ParsedWeekdays::Any => true,
            ParsedWeekdays::Values(weekdays) => {
                weekdays.contains(&(date.weekday().num_days_from_sunday() as u8))
            }
            ParsedWeekdays::Ordinal { weekday, ordinal } => {
                date.weekday().num_days_from_sunday() as u8 == *weekday
                    && ((date.day() - 1) / 7 + 1) as u8 == *ordinal
            }
            ParsedWeekdays::Last { weekday } => {
                date.weekday().num_days_from_sunday() as u8 == *weekday
                    && date
                        .checked_add_days(chrono::Days::new(7))
                        .is_some_and(|next| next.month() != date.month())
            }
        }
    }

    fn has_possible_date(&self) -> bool {
        (2000..2400).any(|year| {
            self.months.iter().any(|month| {
                self.days_of_month.iter().any(|day| {
                    NaiveDate::from_ymd_opt(year, u32::from(*month), u32::from(*day))
                        .is_some_and(|date| self.matches_date(date))
                })
            })
        })
    }
}

fn parse_weekdays(value: &str) -> Result<ParsedWeekdays, ScheduleValidationError> {
    if value == "*" {
        return Ok(ParsedWeekdays::Any);
    }
    if let Some((weekday, ordinal)) = value.split_once('#') {
        if value.matches('#').count() != 1 || weekday.contains([',', '-', '/']) {
            return Err(weekday_error(
                "ordinal weekdays must use one form such as 1#1",
            ));
        }
        let weekday = parse_single("weekday", weekday, 0, 6)?;
        let ordinal = ordinal
            .parse::<u8>()
            .map_err(|_| weekday_error("ordinal must be #1 through #4"))?;
        if !(1..=4).contains(&ordinal) {
            return Err(weekday_error("ordinal must be #1 through #4"));
        }
        return Ok(ParsedWeekdays::Ordinal { weekday, ordinal });
    }
    if let Some(weekday) = value.strip_suffix('L') {
        if weekday.is_empty() || weekday.contains([',', '-', '/']) {
            return Err(weekday_error("last weekday must use one form such as 1L"));
        }
        return Ok(ParsedWeekdays::Last {
            weekday: parse_single("weekday", weekday, 0, 6)?,
        });
    }
    Ok(ParsedWeekdays::Values(parse_numeric_field(
        "weekday", value, 0, 6,
    )?))
}

fn parse_numeric_field(
    field: &str,
    value: &str,
    minimum: u8,
    maximum: u8,
) -> Result<BTreeSet<u8>, ScheduleValidationError> {
    if value.is_empty() {
        return Err(field_error(field, "field cannot be empty"));
    }
    if value.bytes().any(|byte| byte.is_ascii_alphabetic()) {
        return Err(field_error(
            field,
            "names and dialect extensions are unsupported",
        ));
    }
    let mut values = BTreeSet::new();
    for part in value.split(',') {
        if part.is_empty() {
            return Err(field_error(field, "list contains an empty item"));
        }
        let (base, step) = match part.split_once('/') {
            Some((base, step)) => {
                if part.matches('/').count() != 1 {
                    return Err(field_error(field, "step contains more than one slash"));
                }
                let step = step
                    .parse::<u8>()
                    .map_err(|_| field_error(field, "step must be a positive integer"))?;
                if step == 0 {
                    return Err(field_error(field, "step must be greater than zero"));
                }
                (base, step)
            }
            None => (part, 1),
        };
        let (start, end) = if base == "*" {
            (minimum, maximum)
        } else if let Some((start, end)) = base.split_once('-') {
            if base.matches('-').count() != 1 {
                return Err(field_error(field, "range contains more than one dash"));
            }
            (
                parse_single(field, start, minimum, maximum)?,
                parse_single(field, end, minimum, maximum)?,
            )
        } else {
            let start = parse_single(field, base, minimum, maximum)?;
            (start, if part.contains('/') { maximum } else { start })
        };
        if start > end {
            return Err(field_error(field, "range start must not exceed its end"));
        }
        let mut current = start;
        loop {
            values.insert(current);
            let Some(next) = current.checked_add(step) else {
                break;
            };
            if next > end {
                break;
            }
            current = next;
        }
    }
    if values.is_empty() {
        return Err(field_error(field, "field selects no values"));
    }
    Ok(values)
}

fn parse_single(
    field: &str,
    value: &str,
    minimum: u8,
    maximum: u8,
) -> Result<u8, ScheduleValidationError> {
    let parsed = value
        .parse::<u8>()
        .map_err(|_| field_error(field, "value must be an integer"))?;
    if !(minimum..=maximum).contains(&parsed) {
        return Err(field_error(
            field,
            format!("value must be within {minimum}-{maximum}"),
        ));
    }
    Ok(parsed)
}

fn field_error(field: &str, message: impl Into<String>) -> ScheduleValidationError {
    ScheduleValidationError::new(field, message)
}

fn weekday_error(message: impl Into<String>) -> ScheduleValidationError {
    field_error("weekday", message)
}
