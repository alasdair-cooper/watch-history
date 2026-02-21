use crate::film::{MonthOfYear, Rating, WatchedFilm};
use comrak::nodes::{AstNode, NodeHeading, NodeValue};
use comrak::{format_commonmark, parse_document, Arena, Options};
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

fn get_films_from_ast<'a>(root: &'a AstNode<'a>) -> Vec<WatchedFilm> {
    let mut years: Vec<Year> = Vec::new();

    for node in root.children() {
        match &node.data.borrow().value {
            NodeValue::Heading(NodeHeading { level: 2, .. })
                if let Some(text_node) = node.first_child()
                    && let NodeValue::Text(ref text) = text_node.data.borrow().value
                    && let Ok(year) = i16::from_str(text.trim()) =>
            {
                let new_year = Year {
                    name: year,
                    months: vec![],
                };
                years.push(new_year);
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
                                && let NodeValue::Text(ref text) =
                                    text_node.data.borrow().value
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

pub fn parse_films_from_markdown(markdown: impl Into<String>) -> Vec<WatchedFilm> {
    let arena = Arena::new();
    let markdown = markdown.into();
    let ast = parse_document(&arena, &markdown, &Options::default());

    get_films_from_ast(ast)
}

pub fn _write_film_to_markdown(markdown: impl Into<String>, film: WatchedFilm) -> String {
    let arena = Arena::new();
    let markdown = markdown.into();
    let ast = parse_document(&arena, &markdown, &Options::default());

    let mut films = get_films_from_ast(ast);

    films.sort_by(|a, b| {
        a.year_watched
            .cmp(&b.year_watched)
            .then(a.month_of_year_watched.cmp(&b.month_of_year_watched))
    });

    let most_recent_film = films.last();

    let now = jiff::Zoned::now().date();
    let current_year = now.year();
    let current_month = MonthOfYear::try_from(now.month()).unwrap();

    let year_to_add = match most_recent_film {
        Some(film) if film.year_watched == current_year => None,
        _ => Some(current_year),
    };

    let month_to_add = match most_recent_film {
        Some(film) if film.month_of_year_watched == current_month => None,
        _ => Some(current_month),
    };

    if let Some(year_to_add) = year_to_add {
        let heading = arena.alloc(AstNode::from(NodeValue::Heading(NodeHeading {
            level: 2,
            ..NodeHeading::default()
        })));

        let text = arena.alloc(AstNode::from(NodeValue::Text(
            year_to_add.to_string().into(),
        )));

        heading.append(text);

        ast.append(heading);
    }

    if let Some(month_to_add) = month_to_add {
        let heading = arena.alloc(AstNode::from(NodeValue::Heading(NodeHeading {
            level: 3,
            ..NodeHeading::default()
        })));

        let text = arena.alloc(AstNode::from(NodeValue::Text(
            month_to_add.to_string().into(),
        )));

        heading.append(text);

        ast.append(heading);
    }

    let list_item = arena.alloc(AstNode::from(NodeValue::Item(Default::default())));

    let paragraph = arena.alloc(AstNode::from(NodeValue::Paragraph));
    let text = arena.alloc(AstNode::from(NodeValue::Text(
        format!("{} - {}", film.title, film.rating).into(),
    )));

    paragraph.append(text);
    list_item.append(paragraph);

    match ast.last_child() {
        Some(list) if matches!(list.data.borrow().value, NodeValue::List(_)) => {
            list.append(list_item)
        }
        _ => {
            let list = arena.alloc(AstNode::from(NodeValue::List(Default::default())));
            list.append(list_item);
            ast.append(list);
        }
    };

    let mut output = "".to_string();

    format_commonmark(ast, &Options::default(), &mut output).expect("failed to format");

    output
}
