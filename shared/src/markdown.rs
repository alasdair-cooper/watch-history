use crate::film::{MonthOfYear, Rating, WatchedFilm};
use std::str::FromStr;

struct Film {
    title: String,
    rating: Rating,
}

struct Month {
    month_of_year: MonthOfYear,
    films: Vec<Film>,
}

struct Year {
    name: i16,
    months: Vec<Month>,
}

pub fn parse_films_from_markdown(text: impl Into<String>) -> Vec<WatchedFilm> {
    let root = markdown::to_mdast(text.into().as_str(), &markdown::ParseOptions::default())
        .unwrap_or_else(|err| panic!("Failed parsing markdown: {:?}", err));

    let mut years: Vec<Box<Year>> = vec![];

    for curr in root.children().unwrap() {
        use markdown::mdast::*;

        match curr {
            Node::Heading(Heading {
                depth: 2, children, ..
            }) if let Some(Node::Text(Text { value, .. })) = children.first()
                && let Ok(year) = i16::from_str(value.trim()) =>
            {
                let new_year = Year {
                    name: year,
                    months: vec![],
                };

                years.push(Box::new(new_year));
            }
            Node::Heading(Heading {
                depth: 3, children, ..
            }) if let Some(Node::Text(Text {
                value: month_str, ..
            })) = children.first()
                && let Ok(month) = MonthOfYear::try_from(month_str.as_str())
                && let Some(current_year) = years.last_mut() =>
            {
                let new_month = Month {
                    month_of_year: month,
                    films: vec![],
                };

                current_year.months.push(new_month);
            }
            Node::List(List { children, .. })
                if let Some(current_year) = years.last_mut()
                    && let Some(current_month) = current_year.months.last_mut() =>
            {
                for child in children {
                    match child {
                        Node::ListItem(ListItem { children, .. })
                            if let Some(Node::Paragraph(Paragraph { children, .. })) =
                                children.first()
                                && let Some(Node::Text(Text { value, .. })) = children.first()
                                && let Some((film, rating_str)) = value.split_once('-')
                                && let Ok(rating) = Rating::try_from(rating_str) =>
                        {
                            let film = Film {
                                title: film.trim().to_string(),
                                rating,
                            };

                            current_month.films.push(film);
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    years
        .iter()
        .flat_map(|year| {
            year.months.iter().flat_map(|month| {
                month.films.iter().map(|film| WatchedFilm {
                    title: film.title.clone(),
                    rating: film.rating.clone(),
                    year_watched: year.name,
                    month_of_year_watched: month.month_of_year.clone(),
                })
            })
        })
        .collect()
}
