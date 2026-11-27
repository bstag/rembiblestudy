use std::{
    collections::HashSet,
    error::Error,
    io::Read,
    time::{Duration, Instant},
};

use reqwest::{blocking::Client, redirect::Policy};
use scraper::{ElementRef, Html, Selector};
use serde::{Deserialize, Serialize};

const WIKIQUOTE_API: &str = "https://en.wikiquote.org/w/api.php";
const WIKIQUOTE_TIMEOUT: Duration = Duration::from_secs(10);
const MATCH_LIMIT: usize = 5;

#[derive(Serialize)]
pub struct QuoteMatch {
    pub reference: String,
    pub heading: String,
    pub content: String,
    pub source: String,
    pub url: String,
}

#[derive(Deserialize)]
struct Response {
    parse: Option<Page>,
    query: Option<SearchResults>,
    error: Option<ApiError>,
}

#[derive(Deserialize)]
struct SearchResults {
    search: Vec<SearchHit>,
}

#[derive(Deserialize)]
struct SearchHit {
    title: String,
}

#[derive(Deserialize)]
struct ApiError {
    code: String,
}

#[derive(Deserialize)]
struct Page {
    title: String,
    revid: u64,
    text: String,
}

pub fn author_query(input: &str) -> Option<(&str, &str)> {
    ["—", "–", " - ", ":"].iter().find_map(|separator| {
        let (author, fragment) = input.split_once(separator)?;
        let (author, fragment) = (author.trim(), fragment.trim());
        (!author.is_empty() && author.chars().any(char::is_alphabetic) && !fragment.is_empty())
            .then_some((author, fragment))
    })
}

fn words(text: &str) -> String {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn phrase_in(text: &str, phrase: &str) -> bool {
    let phrase = words(phrase);
    !phrase.is_empty() && format!(" {} ", words(text)).contains(&format!(" {phrase} "))
}

fn similar_words(text: &str, phrase: &str) -> bool {
    let stop = [
        "a", "an", "are", "for", "in", "is", "of", "our", "the", "to", "we", "you",
    ];
    let phrase = words(phrase);
    let wanted: HashSet<_> = phrase
        .split_whitespace()
        .filter(|word| !stop.contains(word))
        .map(|word| word.trim_end_matches('s'))
        .collect();
    if wanted.len() < 2 {
        return false;
    }
    let text = words(text);
    let terms: Vec<_> = text
        .split_whitespace()
        .map(|word| word.trim_end_matches('s'))
        .collect();
    let window = (wanted.len() + 6).min(terms.len());
    window >= wanted.len()
        && terms
            .windows(window)
            .any(|part| wanted.iter().all(|word| part.contains(word)))
}

pub fn find_author_quote(author: &str, fragment: &str) -> Result<Vec<QuoteMatch>, Box<dyn Error>> {
    if words(fragment).split_whitespace().count() < 2 {
        return Err("Enter two or more words from the quote.".into());
    }
    let client = wikiquote_client()?;
    Ok(load_page(&client, author, WIKIQUOTE_TIMEOUT)?
        .map(|page| matches_on_page(&page, fragment, true))
        .unwrap_or_default())
}

pub fn find_phrase(fragment: &str) -> Result<(Vec<QuoteMatch>, bool), Box<dyn Error>> {
    let deadline = Instant::now() + WIKIQUOTE_TIMEOUT;
    let client = wikiquote_client()?;
    let query = format!("\"{}\"", words(fragment));
    let reply = wikiquote_request(
        &client,
        &[
            ("action", "query"),
            ("list", "search"),
            ("srsearch", &query),
            ("srnamespace", "0"),
            ("srwhat", "text"),
            ("srlimit", "4"),
            ("srprop", ""),
        ],
        WIKIQUOTE_TIMEOUT,
    )?;
    let Some(results) = reply.query else {
        return Err("Wikiquote could not search for the phrase.".into());
    };
    let mut matches = Vec::new();
    let mut seen = HashSet::new();
    let mut incomplete = false;
    for hit in results.search {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            incomplete = true;
            break;
        }
        match load_page(&client, &hit.title, remaining) {
            Ok(Some(page)) => {
                matches.extend(
                    matches_on_page(&page, fragment, false)
                        .into_iter()
                        .filter(|item| seen.insert((item.url.clone(), words(&item.content)))),
                );
                if matches.len() >= MATCH_LIMIT {
                    break;
                }
            }
            Ok(None) | Err(_) => incomplete = true,
        }
    }
    matches.truncate(MATCH_LIMIT);
    Ok((matches, incomplete))
}

fn wikiquote_client() -> Result<Client, reqwest::Error> {
    Client::builder()
        .redirect(Policy::none())
        .user_agent(concat!(
            "RemBibleStudy/",
            env!("CARGO_PKG_VERSION"),
            " (personal reMarkable notebook quote lookup)"
        ))
        .build()
}

fn wikiquote_request(
    client: &Client,
    query: &[(&str, &str)],
    timeout: Duration,
) -> Result<Response, Box<dyn Error>> {
    let response = client
        .get(WIKIQUOTE_API)
        .timeout(timeout)
        .query(&[("format", "json"), ("formatversion", "2")])
        .query(query)
        .send()
        .and_then(reqwest::blocking::Response::error_for_status)
        .map_err(reqwest::Error::without_url)?;
    let mut body = String::new();
    response.take(4_000_001).read_to_string(&mut body)?;
    if body.len() > 4_000_000 {
        return Err("The Wikiquote response is too large to search.".into());
    }
    Ok(serde_json::from_str(&body)?)
}

fn load_page(
    client: &Client,
    title: &str,
    timeout: Duration,
) -> Result<Option<Page>, Box<dyn Error>> {
    let reply = wikiquote_request(
        client,
        &[
            ("action", "parse"),
            ("page", title),
            ("redirects", "1"),
            ("prop", "text|revid"),
        ],
        timeout,
    )?;
    match reply.parse {
        Some(page) => Ok(Some(page)),
        None if reply
            .error
            .is_some_and(|error| error.code == "missingtitle") =>
        {
            Ok(None)
        }
        None => Err("Wikiquote could not load the author's page.".into()),
    }
}

fn inline_text(item: ElementRef<'_>) -> String {
    let mut result = String::new();
    for child in item.children() {
        if let Some(text) = child.value().as_text() {
            result.push_str(text);
        } else if let Some(element) = ElementRef::wrap(child)
            && !matches!(element.value().name(), "ul" | "ol" | "dl" | "sup" | "style")
        {
            result.push_str(&inline_text(element));
        }
    }
    result.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn matches_on_page(page: &Page, fragment: &str, author_lookup: bool) -> Vec<QuoteMatch> {
    let html = Html::parse_fragment(&page.text);
    let elements = Selector::parse("h2, h3, h4, h5, li, figcaption").expect("static selector");
    let citations = Selector::parse("li").expect("static selector");
    let mut sections: Vec<(&str, String)> = Vec::new();
    let mut seen = HashSet::new();
    let mut matches = Vec::new();
    for item in html.select(&elements) {
        let tag = item.value().name();
        if tag.starts_with('h') {
            while sections
                .last()
                .is_some_and(|(previous, _)| *previous >= tag)
            {
                sections.pop();
            }
            sections.push((tag, inline_text(item)));
            continue;
        }
        let nested = item
            .ancestors()
            .filter_map(ElementRef::wrap)
            .any(|parent| parent.value().name() == "li");
        if nested
            && !item
                .child_elements()
                .any(|child| matches!(child.value().name(), "b" | "q" | "blockquote"))
        {
            continue;
        }
        if sections.iter().any(|(_, title)| {
            let title = words(title);
            [
                "misattributed",
                "disputed",
                "unsourced",
                "quotes about",
                "attributed",
            ]
            .iter()
            .any(|excluded| title.starts_with(excluded))
        }) {
            continue;
        }
        let content = inline_text(item);
        let exact = phrase_in(&content, fragment);
        if content.chars().count() > 600
            || !(exact || author_lookup && tag == "li" && similar_words(&content, fragment))
            || !seen.insert(words(&content))
        {
            continue;
        }
        let section = sections
            .iter()
            .find(|(_, title)| !matches!(title.as_str(), "Quotes" | "Sourced"))
            .map(|(_, title)| title.as_str())
            .unwrap_or("Quotes");
        let cited = item
            .select(&citations)
            .map(inline_text)
            .find(|text| !text.is_empty() && !text.to_ascii_lowercase().starts_with("variant"));
        let mut source = if tag == "figcaption" {
            "Wikiquote image caption · original source unverified".to_owned()
        } else if !author_lookup {
            format!("Wikiquote excerpt · {section}")
        } else if exact {
            format!("Wikiquote attribution · {section}")
        } else {
            format!("Similar wording · Wikiquote attribution · {section}")
        };
        if let Some(cited) = cited {
            source.push_str(" · ");
            source.extend(cited.chars().take(320));
        }
        let heading = if !author_lookup {
            format!("{} (Wikiquote page)", page.title)
        } else if tag == "figcaption" || section == "Quotes" {
            format!("{} (Wikiquote attribution)", page.title)
        } else {
            let work: String = section.chars().take(90).collect();
            format!("{} — {} (Wikiquote attribution)", page.title, work)
        };
        matches.push((
            if tag == "figcaption" {
                2
            } else {
                u8::from(!exact)
            },
            QuoteMatch {
                reference: page.title.clone(),
                heading,
                content,
                source,
                url: format!("https://en.wikiquote.org/w/index.php?oldid={}", page.revid),
            },
        ));
    }
    matches.sort_by_key(|(rank, _)| *rank);
    matches
        .into_iter()
        .take(MATCH_LIMIT)
        .map(|(_, result)| result)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_phrase_without_case_or_punctuation_and_skips_disputed_sections() {
        assert!(!phrase_in("hearts", "heart"));
        assert_eq!(
            author_query("Augustine — hearts are restless"),
            Some(("Augustine", "hearts are restless"))
        );
        let page = Page {
            title: "Example Author".into(), revid: 123,
            text: "<h2>Example Work</h2><ul><li>Our hearts are restless.<ul><li>Book I.</li></ul></li></ul><h2>Disputed</h2><h3>Some work</h3><ul><li>Hearts are restless indeed.</li></ul>".into(),
        };
        let matches = matches_on_page(&page, "HEARTS, are restless", true);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].content, "Our hearts are restless.");
        assert!(matches[0].source.contains("Example Work"));
        assert!(matches[0].source.contains("Book I."));
        assert!(matches[0].url.ends_with("oldid=123"));
    }

    #[test]
    fn phrase_discovery_requires_exact_words_and_labels_the_page() {
        let page = Page {
            title: "A theme page".into(),
            revid: 456,
            text: "<ul><li>Our restless hearts cannot sleep.</li><li>Our hearts are restless tonight.</li></ul>".into(),
        };
        assert_eq!(matches_on_page(&page, "hearts are restless", true).len(), 2);
        let matches = matches_on_page(&page, "hearts are restless", false);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].heading, "A theme page (Wikiquote page)");
        assert_eq!(matches[0].content, "Our hearts are restless tonight.");
    }
}
