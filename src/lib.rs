mod reference;

pub use reference::Reference;

use reqwest::{blocking::Client, header::HeaderValue, redirect::Policy};
use serde::Deserialize;
use std::{error::Error, time::Duration};

const API_URL: &str = "https://rest.api.bible/v1/bibles";
pub const NIV_BIBLE_ID: &str = "78a9f6124f344018-01";

#[derive(Deserialize)]
struct Response<T> {
    data: T,
}

#[derive(Deserialize)]
pub struct Translation {
    pub id: String,
    pub abbreviation: String,
    pub name: String,
}

#[derive(Deserialize)]
pub struct Passage {
    pub reference: String,
    pub content: String,
    pub copyright: String,
}

#[derive(Deserialize)]
pub struct VerseMatch {
    pub text: String,
    pub reference: String,
}

#[derive(Deserialize)]
struct SearchResponse {
    #[serde(default)]
    verses: Vec<VerseMatch>,
}

pub struct BibleApi {
    client: Client,
    key: HeaderValue,
}

impl BibleApi {
    pub fn new(key: &str) -> Result<Self, Box<dyn Error>> {
        let key = key.trim();
        if key.is_empty() {
            return Err("API_BIBLE_KEY is empty".into());
        }
        let mut key = HeaderValue::from_str(key)?;
        key.set_sensitive(true);
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .redirect(Policy::none())
            .build()?;
        Ok(Self { client, key })
    }

    pub fn translations(&self) -> Result<Vec<Translation>, Box<dyn Error>> {
        self.get(API_URL, &[])
    }

    pub fn search_niv(&self, phrase: &str) -> Result<Vec<VerseMatch>, Box<dyn Error>> {
        let url = format!("{API_URL}/{NIV_BIBLE_ID}/search");
        let results: SearchResponse = self.get(&url, &[("query", phrase), ("limit", "5")])?;
        Ok(results.verses)
    }

    pub fn passage(
        &self,
        bible_id: &str,
        reference: &Reference,
    ) -> Result<Passage, Box<dyn Error>> {
        if bible_id.is_empty()
            || !bible_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        {
            return Err("Use a translation ID from the translations command".into());
        }

        let resource = reference.resource();
        let url = format!("{API_URL}/{bible_id}/{resource}/{reference}");
        let passage: Passage = self.get(
            &url,
            &[
                ("content-type", "text"),
                ("include-titles", "false"),
                ("include-verse-numbers", "false"),
                ("use-org-id", "true"),
            ],
        )?;
        if passage.content.trim().is_empty() {
            return Err("API.Bible returned an empty passage".into());
        }
        Ok(passage)
    }

    fn get<T: serde::de::DeserializeOwned>(
        &self,
        url: &str,
        query: &[(&str, &str)],
    ) -> Result<T, Box<dyn Error>> {
        let response = self
            .client
            .get(url)
            .query(query)
            .header("api-key", &self.key)
            .send()
            .map_err(reqwest::Error::without_url)?;
        let status = response.status();
        if !status.is_success() {
            let message = match status.as_u16() {
                401 | 403 => "Check your key and access to this translation",
                404 => "The translation or reference was unavailable",
                429 => "The API request limit was reached",
                _ => "The API did not return Scripture",
            };
            return Err(format!("API.Bible returned {status}. {message}.").into());
        }
        Ok(response
            .json::<Response<T>>()
            .map_err(reqwest::Error::without_url)?
            .data)
    }
}
