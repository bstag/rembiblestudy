mod ink;
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
mod pen;
mod quotes;

use rembiblestudy::{BibleApi, NIV_BIBLE_ID, Reference};
use serde::Serialize;
use serde_json::json;
use std::{env, error::Error, fs, process::ExitCode};

const USAGE: &str = "Usage:\n  rembiblestudy translations\n  rembiblestudy lookup [translation-id] \"Ephesians 4:13\"\n  rembiblestudy quote \"our hearts are restless\"\n  rembiblestudy quote \"Augustine - our hearts are restless\"\n\nScripture lookups default to NIV.";
const TABLET_KEY_FILE: &str = "/home/root/.config/rembiblestudy/api-key";

#[derive(Serialize)]
struct QuoteResults {
    matches: Vec<quotes::QuoteMatch>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    warnings: Vec<String>,
}

fn run() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.as_slice() {
        [] => println!("{USAGE}"),
        [help] if help == "--help" || help == "-h" => println!("{USAGE}"),
        [version] if version == "--version" => {
            println!("rembiblestudy {}", env!("CARGO_PKG_VERSION"))
        }
        [command] if command == "translations" => {
            for translation in api()?.translations()? {
                println!(
                    "{}\t{}\t{}",
                    translation.id, translation.abbreviation, translation.name
                );
            }
        }
        [command, text] if command == "lookup" => lookup(NIV_BIBLE_ID, text)?,
        [command, bible_id, text] if command == "lookup" => lookup(bible_id, text)?,
        [command, text] if command == "tablet-lookup" => {
            let result = match tablet_lookup(text) {
                Ok(passage) => json!({
                    "reference": passage.reference,
                    "content": passage.content.trim(),
                    "copyright": passage.copyright.trim()
                }),
                Err(error) => json!({ "error": error.to_string() }),
            };
            println!("{result}");
        }
        [command, text] if command == "quote" => {
            let result = search_quotes(text, api)?;
            for warning in result.warnings {
                eprintln!("{warning}");
            }
            if result.matches.is_empty() {
                println!("No matching quotes.");
            }
            for item in result.matches {
                println!("{}\n{}\n{}", item.heading, item.content, item.source);
                if !item.url.is_empty() {
                    println!("{}", item.url);
                }
                println!();
            }
        }
        [command, text] if command == "tablet-quote" => match search_quotes(text, tablet_api) {
            Ok(result) => println!("{}", serde_json::to_string(&result)?),
            Err(error) => println!("{}", json!({ "error": error.to_string() })),
        },
        [command, request] if command == "tablet-ink-check" || command == "tablet-ink-write" => {
            let result = match ink_command(command, request) {
                Ok(result) => result,
                Err(error) => json!({ "error": error.to_string() }),
            };
            println!("{result}");
        }
        _ => return Err(USAGE.into()),
    }
    Ok(())
}

fn ink_command(command: &str, request: &str) -> Result<serde_json::Value, Box<dyn Error>> {
    let request = serde_json::from_str(request)?;
    let strokes = ink::layout(&request)?;
    if command == "tablet-ink-write" {
        #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
        pen::write_ink(&strokes)?;
        #[cfg(not(all(target_os = "linux", target_arch = "aarch64")))]
        return Err("Ink writing is available on the Paper Pro.".into());
    }
    Ok(json!({ "strokes": strokes.len(), "bounds": ink::stroke_bounds(&strokes) }))
}

fn lookup(bible_id: &str, text: &str) -> Result<(), Box<dyn Error>> {
    let reference = parse_reference(text)?;
    let passage = api()?.passage(bible_id, &reference)?;
    if bible_id == NIV_BIBLE_ID {
        println!("{} (NIV)", passage.reference);
    } else {
        println!("{}", passage.reference);
    }
    println!("\n{}", passage.content.trim());
    if !passage.copyright.trim().is_empty() {
        println!("\n{}", passage.copyright.trim());
    }
    Ok(())
}

fn tablet_lookup(text: &str) -> Result<rembiblestudy::Passage, Box<dyn Error>> {
    let reference = parse_reference(text)?;
    tablet_api()?.passage(NIV_BIBLE_ID, &reference)
}

fn tablet_api() -> Result<BibleApi, Box<dyn Error>> {
    let key = fs::read_to_string(TABLET_KEY_FILE)
        .map_err(|_| "Set up your API.Bible key for Bible Study on this tablet")?;
    BibleApi::new(&key)
}

fn search_quotes(
    text: &str,
    bible: fn() -> Result<BibleApi, Box<dyn Error>>,
) -> Result<QuoteResults, Box<dyn Error>> {
    let text = text.trim();
    if text.split_whitespace().count() < 2 || text.chars().count() > 200 {
        return Err("Enter a short quote fragment with at least two words.".into());
    }
    if Reference::parse(text).is_none()
        && let Some((author, fragment)) = quotes::author_query(text)
    {
        return Ok(QuoteResults {
            matches: quotes::find_author_quote(author, fragment)?,
            warnings: Vec::new(),
        });
    }
    let (verses, wikiquote) = std::thread::scope(|scope| {
        let scripture = scope.spawn(|| search_niv(text, bible).map_err(|err| err.to_string()));
        let wikiquote = quotes::find_phrase(text);
        (
            scripture.join().expect("Scripture search panicked"),
            wikiquote,
        )
    });
    let mut warnings = Vec::new();
    let mut matches = match verses {
        Ok(verses) => verses,
        Err(err) => {
            warnings.push(format!("NIV search unavailable. {err}"));
            Vec::new()
        }
    };
    match wikiquote {
        Ok((results, incomplete)) => {
            matches.extend(results);
            if incomplete {
                warnings.push("Some Wikiquote pages could not be checked.".into());
            }
        }
        Err(err) => {
            if !warnings.is_empty() {
                return Err(
                    format!("{} Wikiquote search failed. {err}", warnings.join(" ")).into(),
                );
            }
            warnings.push("Wikiquote search unavailable.".into());
        }
    }
    matches.sort_by_key(|item| !quotes::phrase_in(&item.content, text));
    Ok(QuoteResults { matches, warnings })
}

fn search_niv(
    text: &str,
    bible: fn() -> Result<BibleApi, Box<dyn Error>>,
) -> Result<Vec<quotes::QuoteMatch>, Box<dyn Error>> {
    Ok(bible()?
        .search_niv(text)?
        .into_iter()
        .filter(|verse| !verse.text.trim().is_empty())
        .map(|verse| quotes::QuoteMatch {
            reference: format!("{} (NIV)", verse.reference),
            heading: format!("{} (NIV)", verse.reference),
            content: verse.text,
            source: "NIV · API.Bible".into(),
            url: String::new(),
        })
        .collect())
}

fn parse_reference(text: &str) -> Result<Reference, Box<dyn Error>> {
    Reference::parse(text)
        .ok_or_else(|| "Enter a reference such as Ephesians 4:13 or Eph 4:11-16".into())
}

fn api() -> Result<BibleApi, Box<dyn Error>> {
    let key = env::var("API_BIBLE_KEY").map_err(|_| "Set API_BIBLE_KEY in your environment")?;
    BibleApi::new(&key)
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
