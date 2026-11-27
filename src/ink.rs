use serde::Deserialize;

pub const SCREEN_WIDTH: i32 = 1620;
pub const SCREEN_HEIGHT: i32 = 2160;
pub type Point = [i32; 2];
pub type Stroke = Vec<Point>;

pub fn stroke_bounds(strokes: &[Stroke]) -> Option<[i32; 4]> {
    let mut points = strokes.iter().flatten();
    let &[x, y] = points.next()?;
    Some(points.fold([x, y, x + 1, y + 1], |bounds, &[x, y]| {
        [
            bounds[0].min(x),
            bounds[1].min(y),
            bounds[2].max(x + 1),
            bounds[3].max(y + 1),
        ]
    }))
}

const FONT: &str = include_str!("../assets/futural.jhf");
const LINE_HEIGHT: i32 = 48;

#[derive(Deserialize)]
pub struct InkRequest {
    pub reference: String,
    pub content: String,
    pub bounds: [i32; 4],
    pub heading: Option<String>,
}

struct Glyph<'a> {
    left: i32,
    width: i32,
    points: &'a [u8],
}

fn coordinate(value: u8) -> i32 {
    i32::from(value) - i32::from(b'R')
}

fn font() -> Vec<Glyph<'static>> {
    FONT.lines()
        .map(|line| {
            let bytes = line.as_bytes();
            let left = coordinate(bytes[8]);
            Glyph {
                left,
                width: coordinate(bytes[9]) - left,
                points: &bytes[10..],
            }
        })
        .collect()
}

fn printable(value: char) -> u8 {
    match value {
        '’' | '‘' => b'\'',
        '“' | '”' => b'"',
        '–' | '—' | '‑' => b'-',
        c if (' '..='~').contains(&c) => c as u8,
        _ => b'?',
    }
}

fn scaled(value: i32) -> i32 {
    value * 5 / 4
}

pub fn layout(request: &InkRequest) -> Result<Vec<Stroke>, &'static str> {
    let [left, top, right, bottom] = request.bounds;
    if !(0..SCREEN_WIDTH).contains(&left)
        || !(0..SCREEN_HEIGHT).contains(&top)
        || !(0..=SCREEN_WIDTH).contains(&right)
        || !(0..=SCREEN_HEIGHT).contains(&bottom)
        || right - left < 200
        || bottom - top < LINE_HEIGHT
    {
        return Err("Leave more space below the reference before inserting ink.");
    }
    if request.reference.trim().is_empty()
        || request.content.trim().is_empty()
        || request.reference.len() > 100
        || request.content.chars().count() > 600
    {
        return Err("Select a shorter passage for ink insertion.");
    }

    let glyphs = font();
    let lookup = |value: u8| &glyphs[usize::from(value - b' ')];
    let space = scaled(lookup(b' ').width);
    let mut strokes = Vec::new();
    let mut y = top;

    let heading = request
        .heading
        .clone()
        .unwrap_or_else(|| format!("{} (NIV)", request.reference.trim()));
    if heading.trim().is_empty() || heading.chars().count() > 200 {
        return Err("Use a shorter heading for ink insertion.");
    }
    for paragraph in [heading.as_str(), request.content.as_str()] {
        let mut x = left;
        for word in paragraph.split_whitespace() {
            let word: Vec<_> = word.chars().map(printable).collect();
            let width: i32 = word.iter().map(|&c| scaled(lookup(c).width)).sum();
            if width > right - left - 4 {
                return Err("A word is too wide for this placement.");
            }
            if x + width > right - 4 {
                x = left;
                y += LINE_HEIGHT;
            }
            if y + LINE_HEIGHT > bottom {
                return Err(
                    "The passage will not fit below the reference. Leave more room or select fewer verses.",
                );
            }
            for value in word {
                let glyph = lookup(value);
                let mut stroke = Vec::new();
                for pair in glyph.points.as_chunks::<2>().0 {
                    if pair == b" R" {
                        if !stroke.is_empty() {
                            strokes.push(std::mem::take(&mut stroke));
                        }
                    } else {
                        let px = x + scaled(coordinate(pair[0]) - glyph.left);
                        let py = y + scaled(coordinate(pair[1]) + 16);
                        if px < left || px >= right || py < top || py >= bottom {
                            return Err("The passage extends outside the available writing area.");
                        }
                        stroke.push([px, py]);
                    }
                }
                if !stroke.is_empty() {
                    strokes.push(stroke);
                }
                x += scaled(glyph.width);
            }
            x += space;
        }
        y += LINE_HEIGHT;
    }
    Ok(strokes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_heading_and_wraps_text_inside_the_requested_region() {
        let request = InkRequest {
            reference: "Ephesians 4:13".into(),
            content: "Test words “with punctuation” — and a second line.".into(),
            bounds: [100, 250, 500, 650],
            heading: None,
        };
        let strokes = layout(&request).unwrap();
        // The first stroke is the full upright of the heading's capital E.
        assert_eq!(strokes[0], [[105, 255], [105, 281]]);
        assert!(strokes.len() > 40);
        assert!(strokes.iter().flatten().any(|p| p[1] > 335));
        assert!(
            strokes
                .iter()
                .flatten()
                .all(|p| { (100..500).contains(&p[0]) && (250..650).contains(&p[1]) })
        );
    }

    #[test]
    fn refuses_to_write_a_partial_passage_when_the_region_is_too_short() {
        let request = InkRequest {
            reference: "Eph 4:13".into(),
            content: "This should not fit in the same line as the heading.".into(),
            bounds: [100, 250, 500, 305],
            heading: None,
        };
        assert!(layout(&request).is_err());
    }

    #[test]
    fn spaces_the_passage_below_the_heading_by_the_line_height() {
        let request = InkRequest {
            reference: "E".into(),
            content: "E".into(),
            bounds: [100, 250, 500, 400],
            heading: None,
        };
        let strokes = layout(&request).unwrap();
        assert_eq!(strokes.last().unwrap()[0][1], 281 + LINE_HEIGHT);
    }

    #[test]
    fn placement_bounds_cover_strokes_without_including_unused_page_space() {
        let strokes = vec![vec![[110, 225], [300, 240]], vec![[128, 270], [180, 251]]];
        assert_eq!(stroke_bounds(&strokes), Some([110, 225, 301, 271]));
        assert_eq!(stroke_bounds(&[]), None);
    }
}
