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
    #[error("illegal magnitude, only s, m, h, d, w, M and y allowed")]
    IllegalMagnitude,
    #[error("value with magnitude overflows")]
    Overflow,
    #[error("duplicate expiration values")]
    DuplicateExpirations,
}

/// Single expiration value that can be the default in a set of values.
#[derive(Clone, Copy, Debug, Ord, Eq, PartialEq, PartialOrd)]
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

        let secs = if let Some(mag_pos) = secs.find(|c: char| !char::is_ascii_digit(&c)) {
            let (val, mag) = secs.split_at(mag_pos);

            let val = val.parse::<u64>().map_err(Error::ParsingNumber)?;

            let mag = match mag {
                "s" => 1,
                "m" => 60,
                "h" => 60 * 60,
                "d" => 24 * 60 * 60,
                "w" => 7 * 24 * 60 * 60,
                "M" => MONTH_SECS,
                "y" => YEAR_SECS,
                _ => Err(Error::IllegalMagnitude)?,
            };

            val.checked_mul(mag).ok_or(Error::Overflow)?
        } else {
            secs.parse::<u64>().map_err(Error::ParsingNumber)?
        };

        let default = parts.next().map_or(Ok(false), |p| {
            if parts.next().is_some() {
                Err(Error::IllegalModifier)
            } else if p == "d" {
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

        const SEPARATOR: &str = ", ";
        let mut sep = "";
        let mut secs = self.duration.as_secs();

        if secs == 0 {
            return write!(f, "never");
        }

        if let Some((years, rem)) = div(secs, YEAR_SECS) {
            if years > 1 {
                write!(f, "{years} years")?;
            } else {
                write!(f, "1 year")?;
            }
            secs = rem;
            sep = SEPARATOR;
        }

        if let Some((months, rem)) = div(secs, MONTH_SECS) {
            if months > 1 {
                write!(f, "{sep}{months} months")?;
            } else {
                write!(f, "{sep}1 month")?;
            }
            secs = rem;
            sep = SEPARATOR;
        }

        if let Some((weeks, rem)) = div(secs, 60 * 60 * 24 * 7) {
            if weeks > 1 {
                write!(f, "{sep}{weeks} weeks")?;
            } else {
                write!(f, "{sep}1 week")?;
            }
            secs = rem;
            sep = SEPARATOR;
        }

        if let Some((days, rem)) = div(secs, 60 * 60 * 24) {
            if days > 1 {
                write!(f, "{sep}{days} days")?;
            } else {
                write!(f, "{sep}1 day")?;
            }
            secs = rem;
            sep = SEPARATOR;
        }

        if let Some((hours, rem)) = div(secs, 60 * 60) {
            if hours > 1 {
                write!(f, "{sep}{hours} hours")?;
            } else {
                write!(f, "{sep}1 hour")?;
            }
            secs = rem;
            sep = SEPARATOR;
        }

        if let Some((minutes, rem)) = div(secs, 60) {
            if minutes > 1 {
                write!(f, "{sep}{minutes} minutes")?;
            } else {
                write!(f, "{sep}1 minute")?;
            }
            secs = rem;
            sep = SEPARATOR;
        }

        if secs > 1 {
            write!(f, "{sep}{secs} seconds")?;
        } else if secs == 1 {
            write!(f, "{sep}1 second")?;
        }

        Ok(())
    }
}

impl FromStr for ExpirationSet {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut values: Vec<Expiration> = s
            .split(',')
            .map(FromStr::from_str)
            .collect::<Result<_, _>>()?;

        if values.iter().filter(|exp| exp.default).count() > 1 {
            return Err(Error::MultipleDefaults);
        }

        values.sort();

        if values.windows(2).any(|w| w[0].duration == w[1].duration) {
            Err(Error::DuplicateExpirations)?;
        }

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
    fn expiration_with_magnitude() {
        let expiration = "60s".parse::<Expiration>().unwrap();
        assert_eq!(expiration.duration, Duration::from_secs(60));

        let expiration = "59m".parse::<Expiration>().unwrap();
        assert_eq!(expiration.duration, Duration::from_secs(59 * 60));

        let expiration = "13h".parse::<Expiration>().unwrap();
        assert_eq!(expiration.duration, Duration::from_secs(13 * 60 * 60));

        let expiration = "4d".parse::<Expiration>().unwrap();
        assert_eq!(expiration.duration, Duration::from_secs(4 * 24 * 60 * 60));

        let expiration = "40w".parse::<Expiration>().unwrap();
        assert_eq!(
            expiration.duration,
            Duration::from_secs(40 * 7 * 24 * 60 * 60)
        );

        let expiration = "12M".parse::<Expiration>().unwrap();
        assert_eq!(expiration.duration, Duration::from_secs(12 * MONTH_SECS));

        let expiration = "80y".parse::<Expiration>().unwrap();
        assert_eq!(expiration.duration, Duration::from_secs(80 * YEAR_SECS));

        let expiration = "0y".parse::<Expiration>().unwrap();
        assert_eq!(expiration.duration, Duration::from_secs(0));
    }

    #[test]
    fn expiration_with_illegal_magnitude() {
        assert!(matches!(
            "1x".parse::<ExpirationSet>(),
            Err(Error::IllegalMagnitude)
        ));
        assert!(matches!(
            "1W".parse::<ExpirationSet>(),
            Err(Error::IllegalMagnitude)
        ));
        assert!(matches!(
            "1dd".parse::<ExpirationSet>(),
            Err(Error::IllegalMagnitude)
        ));
        assert!(matches!(
            "1dh".parse::<ExpirationSet>(),
            Err(Error::IllegalMagnitude)
        ));
        assert!(matches!(
            "1d ".parse::<ExpirationSet>(),
            Err(Error::IllegalMagnitude)
        ));
        assert!(matches!(
            "1d0".parse::<ExpirationSet>(),
            Err(Error::IllegalMagnitude)
        ));
        assert!(matches!(
            "d".parse::<ExpirationSet>(),
            Err(Error::ParsingNumber(_))
        ));
        assert!(matches!(
            "d0".parse::<ExpirationSet>(),
            Err(Error::ParsingNumber(_))
        ));
        assert!(matches!(
            "999999999999y".parse::<ExpirationSet>(),
            Err(Error::Overflow)
        ));
    }

    #[test]
    fn default_expiration() {
        let expiration = "60=d".parse::<Expiration>().unwrap();
        assert_eq!(expiration.duration, Duration::from_secs(60));
        assert!(expiration.default);
    }

    #[test]
    fn illegal_modifier() {
        assert!(matches!(
            "60==d".parse::<ExpirationSet>(),
            Err(Error::IllegalModifier)
        ));

        assert!(matches!(
            "60= d".parse::<ExpirationSet>(),
            Err(Error::IllegalModifier)
        ));

        assert!(matches!(
            "60=e".parse::<ExpirationSet>(),
            Err(Error::IllegalModifier)
        ));

        assert!(matches!(
            "60=d=".parse::<ExpirationSet>(),
            Err(Error::IllegalModifier)
        ));

        assert!(matches!(
            "60=d=d".parse::<ExpirationSet>(),
            Err(Error::IllegalModifier)
        ));

        assert!(matches!(
            "60==".parse::<ExpirationSet>(),
            Err(Error::IllegalModifier)
        ));
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
        assert!(matches!(
            "3600=d,60=d,48000".parse::<ExpirationSet>(),
            Err(Error::MultipleDefaults)
        ));
    }

    #[test]
    fn duplicate_expirations() {
        assert!(matches!(
            "60,60=d".parse::<ExpirationSet>(),
            Err(Error::DuplicateExpirations)
        ));
        assert!(matches!(
            "3600,60,48000,60=d,3600".parse::<ExpirationSet>(),
            Err(Error::DuplicateExpirations)
        ));
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
            "1 month, 3 weeks, 5 days"
        );
        assert_eq!(
            format!("{}", Expiration::from_secs(60 * 60 * 24 * 7 * 4 * 12)),
            "11 months, 6 days"
        );
        assert_eq!(
            format!("{}", Expiration::from_secs(60 * 60 * 24 * 7 * 4 * 24)),
            "1 year, 10 months, 1 week"
        );
        assert_eq!(
            format!(
                "{}",
                Expiration::from_secs(
                    60 * 60 * 24 * 7 * 4 * 24
                        + 60 * 60 * 24 * 7 * 8
                        + 60 * 60 * 24 * 7 * 2
                        + 60 * 60 * 24 * 2
                        + 3 * 60 * 60 * 24
                        + 23 * 60 * 60
                        + 59 * 60
                        + 42
                )
            ),
            "2 years, 2 weeks, 3 days, 23 hours, 59 minutes, 42 seconds"
        );
        assert_eq!(
            format!(
                "{}",
                Expiration::from_secs(60 * 60 * 24 * 7 * 8 + 60 * 60 * 24 * 2 + 23 * 60 * 60 + 42)
            ),
            "1 month, 4 weeks, 23 hours, 42 seconds"
        );

        assert_eq!(
            format!("{}", "1".parse::<Expiration>().unwrap()),
            "1 second"
        );

        assert_eq!(
            format!("{}", "30s".parse::<Expiration>().unwrap()),
            "30 seconds"
        );
        assert_eq!(
            format!("{}", "59m".parse::<Expiration>().unwrap()),
            "59 minutes"
        );
        assert_eq!(
            format!("{}", "3h".parse::<Expiration>().unwrap()),
            "3 hours"
        );
        assert_eq!(format!("{}", "1d".parse::<Expiration>().unwrap()), "1 day");
        assert_eq!(
            format!("{}", "4w".parse::<Expiration>().unwrap()),
            "4 weeks"
        );
        assert_eq!(
            format!("{}", "12M".parse::<Expiration>().unwrap()),
            "12 months"
        );
        assert_eq!(format!("{}", "1y".parse::<Expiration>().unwrap()), "1 year");
    }
}
