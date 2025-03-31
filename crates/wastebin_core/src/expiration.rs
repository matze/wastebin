use std::fmt::Display;
use std::str::FromStr;
use std::time::Duration;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("expiration value is empty")]
    Empty,
    #[error("failed to parse number: {0}")]
    ParsingNumber(std::num::ParseIntError),
    #[error("illegal modifier, only =d allowed")]
    IllegalModifier,
    #[error("multiple default values")]
    MultipleDefaults,
}

/// Single expiration value that can be the default in a set of values.
#[derive(Clone, Debug, Ord, Eq, PartialEq, PartialOrd)]
pub struct Expiration {
    pub duration: Duration,
    pub default: bool,
}

/// Multiple expiration values in ordered fashion.
pub struct ExpirationSet(Vec<Expiration>);

/// Rough number of seconds in a month
const MONTH_SECS: u64 = 30 * 24 * 60 * 60; // 30 days
/// Rough number of seconds in a year
const YEAR_SECS: u64 = 365 * 24 * 60 * 60; // 365 days

/// A single [`Expiration`] can either be an unsigned number or an unsigned number followed by `=d`
/// to denote a default expiration.
impl FromStr for Expiration {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split('=');

        let Some(secs) = parts.next() else {
            return Err(Error::Empty);
        };

        let secs = secs.parse::<u64>().map_err(Error::ParsingNumber)?;

        let default = parts.next().map_or(Ok(false), |p| {
            if p == "d" {
                Ok(true)
            } else {
                Err(Error::IllegalModifier)
            }
        })?;

        Ok(Self {
            duration: Duration::from_secs(secs),
            default,
        })
    }
}

/// Print human-readable duration in a very rough approximation.
impl Display for Expiration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        /// Computes `dividend` / `divisor` and returns `Some(fraction)` if > 0.
        fn div(dividend: u64, divisor: u64) -> Option<(u64, u64)> {
            let r = dividend / divisor;
            (r > 0).then_some((r, dividend % divisor))
        }

        let mut secs = self.duration.as_secs();

        if secs == 0 {
            return write!(f, "never");
        }

        let mut parts = Vec::new();

        if let Some((years, rem)) = div(secs, YEAR_SECS) {
            if years > 1 {
                parts.push(format!("{years} years"));
            } else {
                parts.push(String::from("1 year"));
            }
            secs = rem;
        }

        if let Some((months, rem)) = div(secs, MONTH_SECS) {
            if months > 1 {
                parts.push(format!("{months} months"));
            } else {
                parts.push(String::from("1 month"));
            }
            secs = rem;
        }

        if let Some((weeks, rem)) = div(secs, 60 * 60 * 24 * 7) {
            if weeks > 1 {
                parts.push(format!("{weeks} weeks"));
            } else {
                parts.push(String::from("1 week"));
            }
            secs = rem;
        }

        if let Some((days, rem)) = div(secs, 60 * 60 * 24) {
            if days > 1 {
                parts.push(format!("{days} days"));
            } else {
                parts.push(String::from("1 day"));
            }
            secs = rem;
        }

        if let Some((hours, rem)) = div(secs, 60 * 60) {
            if hours > 1 {
                parts.push(format!("{hours} hours"));
            } else {
                parts.push(String::from("1 hour"));
            }
            secs = rem;
        }

        if let Some((minutes, rem)) = div(secs, 60) {
            if minutes > 1 {
                parts.push(format!("{minutes} minutes"));
            } else {
                parts.push(String::from("1 minute"));
            }
            secs = rem;
        }

        if secs > 0 {
            parts.push(format!("{secs} seconds"));
        }

        f.write_str(&parts.join(" "))
    }
}

impl FromStr for ExpirationSet {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut values: Vec<Expiration> = s
            .split(',')
            .map(FromStr::from_str)
            .collect::<Result<_, _>>()?;

        if values.iter().map(|exp| u64::from(exp.default)).sum::<u64>() > 1 {
            return Err(Error::MultipleDefaults);
        }

        values.sort();

        Ok(ExpirationSet(values))
    }
}

impl ExpirationSet {
    /// Retrieve sorted vector of [`Expiration`] values.
    pub fn into_inner(self) -> Vec<Expiration> {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl Expiration {
        fn from_secs(secs: u64) -> Self {
            Self {
                duration: Duration::from_secs(secs),
                default: false,
            }
        }
    }

    #[test]
    fn non_default_expiration() {
        let expiration = "60".parse::<Expiration>().unwrap();
        assert_eq!(expiration.duration, Duration::from_secs(60));
        assert!(!expiration.default);
    }

    #[test]
    fn default_expiration() {
        let expiration = "60=d".parse::<Expiration>().unwrap();
        assert_eq!(expiration.duration, Duration::from_secs(60));
        assert!(expiration.default);
    }

    #[test]
    fn expiration_set() {
        let expirations = "3600,60=d,48000"
            .parse::<ExpirationSet>()
            .unwrap()
            .into_inner();

        assert_eq!(expirations.len(), 3);

        assert_eq!(expirations[0].duration, Duration::from_secs(60));
        assert_eq!(expirations[1].duration, Duration::from_secs(3600));
        assert_eq!(expirations[2].duration, Duration::from_secs(48000));

        assert!(expirations[0].default);
        assert!(!expirations[1].default);
        assert!(!expirations[2].default);
    }

    #[test]
    fn multiple_defaults() {
        assert!("3600=d,60=d,48000".parse::<ExpirationSet>().is_err());
    }

    #[test]
    fn formatting() {
        assert_eq!(format!("{}", Expiration::from_secs(30)), "30 seconds");
        assert_eq!(format!("{}", Expiration::from_secs(60)), "1 minute");
        assert_eq!(format!("{}", Expiration::from_secs(60 * 2)), "2 minutes");
        assert_eq!(format!("{}", Expiration::from_secs(60 * 60)), "1 hour");
        assert_eq!(format!("{}", Expiration::from_secs(60 * 60 * 2)), "2 hours");
        assert_eq!(format!("{}", Expiration::from_secs(60 * 60 * 24)), "1 day");
        assert_eq!(
            format!("{}", Expiration::from_secs(60 * 60 * 24 * 2)),
            "2 days"
        );
        assert_eq!(
            format!("{}", Expiration::from_secs(60 * 60 * 24 * 7)),
            "1 week"
        );
        assert_eq!(
            format!("{}", Expiration::from_secs(60 * 60 * 24 * 7 * 2)),
            "2 weeks"
        );
        assert_eq!(
            format!("{}", Expiration::from_secs(60 * 60 * 24 * 7 * 4)),
            "4 weeks"
        );
        assert_eq!(
            format!("{}", Expiration::from_secs(60 * 60 * 24 * 7 * 8)),
            "1 month 3 weeks 5 days"
        );
        assert_eq!(
            format!("{}", Expiration::from_secs(60 * 60 * 24 * 7 * 4 * 12)),
            "11 months 6 days"
        );
        assert_eq!(
            format!("{}", Expiration::from_secs(60 * 60 * 24 * 7 * 4 * 24)),
            "1 year 10 months 1 week"
        );
    }
}
