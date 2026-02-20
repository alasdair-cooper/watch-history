use crux_http::http::convert::{Deserialize, Serialize};
use std::fmt::Display;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct WatchedFilm {
    pub title: String,
    pub rating: Rating,
    pub year_watched: i16,
    pub month_of_year_watched: MonthOfYear,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum Rating {
    VeryBad,
    Bad,
    Meh,
    Good,
    VeryGood,
    Goat,
}

pub enum TryFromRatingError {
    EmptyString,
    InvalidRating(String),
}

impl TryFrom<&str> for Rating {
    type Error = TryFromRatingError;

    fn try_from(value: &str) -> Result<Self, TryFromRatingError> {
        match value.trim().to_lowercase().as_str() {
            "" => Err(TryFromRatingError::EmptyString),
            "very bad" => Ok(Self::VeryBad),
            "bad" => Ok(Self::Bad),
            "meh" => Ok(Self::Meh),
            "good" => Ok(Self::Good),
            "very good" => Ok(Self::VeryGood),
            "goat" => Ok(Self::Goat),
            _ => Err(TryFromRatingError::InvalidRating(value.to_string())),
        }
    }
}

impl Display for Rating {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let rating_str = match self {
            Self::VeryBad => "very bad",
            Self::Bad => "bad",
            Self::Meh => "meh",
            Self::Good => "good",
            Self::VeryGood => "very good",
            Self::Goat => "goat",
        };
        write!(f, "{}", rating_str)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct MonthOfYear(i8);

pub enum TryFromMonthOfYearError {
    EmptyString,
    InvalidMonth(String),
}

impl TryFrom<&str> for MonthOfYear {
    type Error = TryFromMonthOfYearError;

    fn try_from(value: &str) -> Result<Self, TryFromMonthOfYearError> {
        match value.trim().to_lowercase().as_str() {
            "" => Err(TryFromMonthOfYearError::EmptyString),
            "january" => Ok(Self(1)),
            "february" => Ok(Self(2)),
            "march" => Ok(Self(3)),
            "april" => Ok(Self(4)),
            "may" => Ok(Self(5)),
            "june" => Ok(Self(6)),
            "july" => Ok(Self(7)),
            "august" => Ok(Self(8)),
            "september" => Ok(Self(9)),
            "october" => Ok(Self(10)),
            "november" => Ok(Self(11)),
            "december" => Ok(Self(12)),
            _ => Err(TryFromMonthOfYearError::InvalidMonth(value.to_string())),
        }
    }
}

impl Display for MonthOfYear {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let month_name = match self.0 {
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
            _ => return Err(std::fmt::Error),
        };
        write!(f, "{}", month_name)
    }
}
