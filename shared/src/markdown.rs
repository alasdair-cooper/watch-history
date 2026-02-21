use crate::film::{MonthOfYear, Rating, WatchedFilm};
use comrak::nodes::{AstNode, NodeHeading, NodeValue};
use comrak::{parse_document, Arena, Options};
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

fn iter_nodes<'a, F>(node: &'a AstNode<'a>, f: &mut F)
where
    F: FnMut(&'a AstNode<'a>),
{
    f(node);
    for child in node.children() {
        iter_nodes(child, f);
    }
}

pub fn parse_films_from_markdown(text: impl Into<String>) -> Vec<WatchedFilm> {
    let arena = Arena::new();
    let text = text.into();
    let root = parse_document(&arena, &text, &Options::default());

    let mut years: Vec<Box<Year>> = vec![];

    iter_nodes(root, &mut |node| match &node.data.borrow().value {
        NodeValue::Heading(NodeHeading { level: 2, .. })
            if let Some(text_node) = node.first_child()
                && let NodeValue::Text(ref text) = text_node.data.borrow().value
                && let Ok(year) = i16::from_str(text.trim()) =>
        {
            let new_year = Year {
                name: year,
                months: vec![],
            };
            years.push(Box::new(new_year));
        }
        NodeValue::Heading(NodeHeading { level: 3, .. })
            if let Some(text_node) = node.first_child()
                && let NodeValue::Text(ref text) = text_node.data.borrow().value
                && let Ok(month) = MonthOfYear::try_from(text.trim())
                && let Some(current_year) = years.last_mut() =>
        {
            let new_month = Month {
                month_of_year: month,
                films: vec![],
            };
            current_year.months.push(new_month);
        }
        NodeValue::List(_)
            if let Some(current_year) = years.last_mut()
                && let Some(current_month) = current_year.months.last_mut() =>
        {
            for list_item in node.children() {
                match list_item.data.borrow().value {
                    NodeValue::Item(_)
                        if let Some(paragraph) = list_item.first_child()
                            && let NodeValue::Paragraph = paragraph.data.borrow().value
                            && let Some(text_node) = paragraph.first_child()
                            && let NodeValue::Text(ref text) = text_node.data.borrow().value
                            && let Some((film, rating_str)) = text.split_once('-')
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
    });

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
