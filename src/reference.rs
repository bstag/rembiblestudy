use std::fmt;

// API.Bible book IDs, English names, and common abbreviations.
const BOOKS: &str = "+GEN Genesis Ge Gn
EXO Exodus Ex
LEV Leviticus Le Lv
NUM Numbers Nu Nm
DEU Deuteronomy Deut Dt
JOS Joshua Josh
JDG Judges Judg Jdg
RUT Ruth Ru
1SA 1Samuel 1Sam
2SA 2Samuel 2Sam
1KI 1Kings 1Kgs
2KI 2Kings 2Kgs
1CH 1Chronicles 1Chron 1Chr
2CH 2Chronicles 2Chron 2Chr
EZR Ezra
NEH Nehemiah
EST Esther Esth
JOB Job
PSA Psalms Psalm Ps Psa
PRO Proverbs Prov Pr
ECC Ecclesiastes Eccl
SNG SongofSongs SongofSolomon Song Canticles
ISA Isaiah Is
JER Jeremiah Je
LAM Lamentations La
EZK Ezekiel Ezek
DAN Daniel Da Dn
HOS Hosea Ho
JOL Joel
AMO Amos Am
OBA Obadiah Obad
JON Jonah Jon
MIC Micah Mi
NAM Nahum Nah
HAB Habakkuk Hab
ZEP Zephaniah Zeph
HAG Haggai
ZEC Zechariah Zech
MAL Malachi Mal
MAT Matthew Matt Mt
MRK Mark Mk
LUK Luke Lk
JHN John Jn
ACT Acts Ac
ROM Romans Ro
1CO 1Corinthians 1Cor
2CO 2Corinthians 2Cor
GAL Galatians Ga
EPH Ephesians Eph
PHP Philippians Phil Php
COL Colossians Col
1TH 1Thessalonians 1Thess 1Thes
2TH 2Thessalonians 2Thess 2Thes
1TI 1Timothy 1Tim
2TI 2Timothy 2Tim
TIT Titus Ti
PHM Philemon Phlm Philem
HEB Hebrews Heb
JAS James Jas Jm
1PE 1Peter 1Pet
2PE 2Peter 2Pet
1JN 1John 1Jhn
2JN 2John 2Jhn
3JN 3John 3Jhn
JUD Jude
REV Revelation Rev Rv";

#[derive(Debug, PartialEq, Eq)]
pub struct Reference {
    book: &'static str,
    start: Verse,
    end: Option<Verse>,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Verse {
    chapter: u16,
    number: u16,
}

fn positive_number(text: &str) -> Option<u16> {
    text.trim().parse::<u16>().ok().filter(|&value| value > 0)
}

impl Reference {
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        let separator = text.find(':')?;
        let prefix = text[..separator].trim_end();
        let book_end = prefix.trim_end_matches(|c: char| c.is_ascii_digit()).len();
        let book_name = prefix[..book_end].replace(|c: char| c.is_whitespace() || c == '.', "");
        let book = BOOKS.lines().find_map(|row| {
            let mut names = row.split_whitespace();
            let id = names.next()?;
            (id.eq_ignore_ascii_case(&book_name)
                || names.any(|name| name.eq_ignore_ascii_case(&book_name)))
            .then_some(id)
        })?;
        let chapter = positive_number(&prefix[book_end..])?;

        let verses = text[separator + 1..].replace(['–', '—'], "-");
        let (first, last) = match verses.split_once('-') {
            Some((first, last)) => (first, Some(last)),
            None => (verses.as_str(), None),
        };
        let start = Verse {
            chapter,
            number: positive_number(first)?,
        };

        let end = if let Some(last) = last {
            let (last_chapter, last_number) = match last.split_once(':') {
                Some((last_chapter, number)) => (positive_number(last_chapter)?, number),
                None => (chapter, last),
            };
            let end = Verse {
                chapter: last_chapter,
                number: positive_number(last_number)?,
            };
            if end < start {
                return None;
            }
            (end != start).then_some(end)
        } else {
            None
        };
        Some(Self { book, start, end })
    }

    pub fn resource(&self) -> &'static str {
        if self.end.is_some() {
            "passages"
        } else {
            "verses"
        }
    }
}

impl fmt::Display for Reference {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}.{}.{}",
            self.book, self.start.chapter, self.start.number
        )?;
        if let Some(end) = &self.end {
            write!(formatter, "-{}.{}.{}", self.book, end.chapter, end.number)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Reference;

    #[test]
    fn converts_notebook_references_to_api_ids() {
        for (input, resource, id) in [
            ("Ephesians 4:13", "verses", "EPH.4.13"),
            ("  eph. 4 : 11–16 ", "passages", "EPH.4.11-EPH.4.16"),
            ("1 Cor 12:31-13:3", "passages", "1CO.12.31-1CO.13.3"),
            ("Song of Songs 2:1", "verses", "SNG.2.1"),
            ("Ps 23:1-1", "verses", "PSA.23.1"),
        ] {
            let reference = Reference::parse(input).unwrap();
            assert_eq!(reference.resource(), resource);
            assert_eq!(reference.to_string(), id);
        }
    }

    #[test]
    fn rejects_unclear_or_reversed_references() {
        for input in [
            "Ephesians",
            "Eph 4:",
            "Eph 4:16-11",
            "Eph 4:13-3:1",
            "Eph 4:0",
            "Eph 4:13-",
            "Eph 4:13,15",
            "Eph 4:13 sermon notes",
            "1:2",
            "Éphé4:13",
        ] {
            assert!(Reference::parse(input).is_none(), "{input}");
        }
    }
}
