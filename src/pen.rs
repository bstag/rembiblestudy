use crate::ink::{Point, SCREEN_HEIGHT, SCREEN_WIDTH, Stroke};
use signal_hook::{consts::SIGTERM, flag};
use std::{
    error::Error,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

const KEY: u16 = 1;
const ABS: u16 = 3;
const TOOL_PEN: u16 = 320;
const TOUCH: u16 = 330;
const PRESSURE: u16 = 24;
const DISTANCE: u16 = 25;

fn marker() -> io::Result<PathBuf> {
    for dir in fs::read_dir("/sys/class/input")? {
        let dir = dir?;
        let name = dir.file_name();
        if name.to_string_lossy().starts_with("event")
            && fs::read_to_string(dir.path().join("device/name"))
                .is_ok_and(|value| value.trim() == "Elan marker input")
        {
            return Ok(PathBuf::from("/dev/input").join(name));
        }
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "Paper Pro marker input unavailable",
    ))
}

struct Pen(File);

impl Pen {
    fn frame(&mut self, events: &[(u16, u16, i32)]) -> io::Result<()> {
        let mut buffer = [0u8; 24 * 8];
        for (slot, &(event_type, code, value)) in buffer.chunks_exact_mut(24).zip(events) {
            slot[16..18].copy_from_slice(&event_type.to_ne_bytes());
            slot[18..20].copy_from_slice(&code.to_ne_bytes());
            slot[20..24].copy_from_slice(&value.to_ne_bytes());
        }
        // The final all-zero input_event is SYN_REPORT.
        self.0.write_all(&buffer[..24 * (events.len() + 1)])
    }

    fn begin_stroke(
        &mut self,
        [x, y]: Point,
        first: bool,
        cancelled: &AtomicBool,
    ) -> io::Result<()> {
        let x = x * 11180 / SCREEN_WIDTH;
        let y = y * 15340 / SCREEN_HEIGHT;
        // Keep hover off the contact coordinate so evdev forwards both axes
        // in the contact report, even for a perfectly vertical first stroke.
        self.frame(&[
            (KEY, TOOL_PEN, 1),
            (ABS, DISTANCE, 20),
            (ABS, 0, x.abs_diff(1) as i32),
            (ABS, 1, y.abs_diff(1) as i32),
        ])?;
        thread::sleep(Duration::from_millis(if first { 120 } else { 2 }));
        if cancelled.load(Ordering::Relaxed) {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "Ink insertion stopped.",
            ));
        }
        self.frame(&[
            (ABS, 0, x),
            (ABS, 1, y),
            (ABS, DISTANCE, 0),
            (ABS, PRESSURE, 2300),
            (KEY, TOUCH, 1),
        ])
    }

    fn move_to(&mut self, [x, y]: Point) -> io::Result<()> {
        self.frame(&[
            (ABS, 0, x * 11180 / SCREEN_WIDTH),
            (ABS, 1, y * 15340 / SCREEN_HEIGHT),
            (ABS, PRESSURE, 2300),
        ])
    }

    fn lift(&mut self) -> io::Result<()> {
        self.frame(&[(KEY, TOUCH, 0), (ABS, PRESSURE, 0), (ABS, DISTANCE, 20)])
    }
}

impl Drop for Pen {
    fn drop(&mut self) {
        let _ = self.lift();
        let _ = self.frame(&[(KEY, TOOL_PEN, 0), (ABS, DISTANCE, 100)]);
    }
}

pub fn write_ink(strokes: &[Stroke]) -> Result<(), Box<dyn Error>> {
    if !fs::read_to_string("/etc/os-release")?
        .lines()
        .any(|line| line == "IMG_VERSION=\"3.29.1.0\"")
        || fs::read("/sys/firmware/devicetree/base/model")? != b"reMarkable Ferrari\0"
    {
        return Err("Ink writing is supported on Paper Pro firmware 3.29.1.0.".into());
    }
    let cancelled = Arc::new(AtomicBool::new(false));
    flag::register(SIGTERM, Arc::clone(&cancelled))?;
    let lock = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open("/run/rembiblestudy-ink.lock")?;
    lock.try_lock()
        .map_err(|_| "Bible Study is already writing ink.")?;
    let mut pen = Pen(OpenOptions::new().write(true).open(marker()?)?);

    let check_cancelled = || {
        if cancelled.load(Ordering::Relaxed) {
            Err("Ink insertion stopped. Some strokes may already be on the page.")
        } else {
            Ok(())
        }
    };
    for (index, stroke) in strokes.iter().enumerate() {
        check_cancelled()?;
        let Some((&first, rest)) = stroke.split_first() else {
            continue;
        };
        pen.begin_stroke(first, index == 0, &cancelled)?;
        let mut previous = first;
        for &point in rest {
            let dx = point[0] - previous[0];
            let dy = point[1] - previous[1];
            let steps = ((dx.abs().max(dy.abs()) + 2) / 3).max(1);
            for step in 1..=steps {
                check_cancelled()?;
                pen.move_to([
                    previous[0] + dx * step / steps,
                    previous[1] + dy * step / steps,
                ])?;
                thread::sleep(Duration::from_millis(1));
            }
            previous = point;
        }
        thread::sleep(Duration::from_millis(7));
        pen.lift()?;
    }
    Ok(())
}
