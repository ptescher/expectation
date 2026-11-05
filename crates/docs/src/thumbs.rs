//! Thumbnails of the screenshots, for the state map's cards and the story cards, so a page of
//! many cards doesn't load every full-size screenshot.
//!
//! Expectation uses only the standard library, so it doesn't decode images itself. It asks a
//! tool that is commonly installed: ImageMagick (`magick`, or `mogrify` from version 6), else
//! macOS's `sips`. Without one there are no thumbnails, and the cards show the screenshots
//! themselves (loaded lazily). SVG screenshots need none: the browser draws them at any size.
//! `EXPECTATION_THUMBNAILS=off` turns them off.
//!
//! A thumbnail is made once and kept as `screenshots/thumbs/<rest>` for `screenshots/<rest>`,
//! and only made again when its screenshot is newer.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Where thumbnails go, relative to the docs directory.
pub(crate) const DIR: &str = "screenshots/thumbs/";
/// Their widest side, in pixels: twice a card, for sharp screens.
const SIZE: u32 = 480;
/// The most files given to the tool at once.
const BATCH: usize = 200;

/// A tool that can shrink images, found on `PATH`.
enum Tool {
    /// ImageMagick 7: `magick mogrify`.
    Magick,
    /// ImageMagick 6: `mogrify`.
    Mogrify,
    /// macOS.
    Sips,
}

fn runs(program: &str, args: &[&str], says: &str) -> bool {
    Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .is_ok_and(|o| o.status.success() && String::from_utf8_lossy(&o.stdout).contains(says))
}

fn find_tool() -> Option<Tool> {
    if std::env::var("EXPECTATION_THUMBNAILS").is_ok_and(|v| v == "off") {
        return None;
    }
    if runs("magick", &["-version"], "ImageMagick") {
        Some(Tool::Magick)
    } else if runs("mogrify", &["-version"], "ImageMagick") {
        Some(Tool::Mogrify)
    } else if cfg!(target_os = "macos") && runs("sips", &["--help"], "sips") {
        Some(Tool::Sips)
    } else {
        None
    }
}

impl Tool {
    /// Shrink `files` into `out`, keeping their names. Errors leave the thumbnails it couldn't
    /// make missing.
    fn shrink(&self, files: &[PathBuf], out: &Path) {
        let size = format!("{SIZE}x{SIZE}>");
        let mut cmd = match self {
            Tool::Magick => {
                let mut c = Command::new("magick");
                c.arg("mogrify");
                c
            }
            Tool::Mogrify => Command::new("mogrify"),
            Tool::Sips => Command::new("sips"),
        };
        match self {
            Tool::Magick | Tool::Mogrify => {
                // No metadata and no timestamps, so the same screenshot makes the same thumbnail.
                cmd.arg("-path").arg(out).args([
                    "-strip",
                    "-define",
                    "png:exclude-chunks=date,time",
                    "-thumbnail",
                    &size,
                ]);
                cmd.args(files);
            }
            Tool::Sips => {
                cmd.args(["-Z", &SIZE.to_string()]);
                cmd.args(files).arg("--out").arg(out);
            }
        }
        let _ = cmd
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

fn newer(a: &Path, b: &Path) -> bool {
    let time = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    match (time(a), time(b)) {
        (Some(a), Some(b)) => a > b,
        _ => true,
    }
}

/// Thumbnails for the screenshots `shown` (as the results name them, `screenshots/<rest>`),
/// in the docs directory `dir`: the `<rest>`s that have one, made now if they had to be.
pub(crate) fn make(dir: &Path, shown: &[&str]) -> HashSet<String> {
    let mut wanted: Vec<&str> = shown
        .iter()
        .filter_map(|p| p.strip_prefix("screenshots/"))
        .filter(|rest| {
            let lower = rest.to_ascii_lowercase();
            !rest.starts_with("thumbs/")
                && [".png", ".jpg", ".jpeg"].iter().any(|e| lower.ends_with(e))
        })
        .collect();
    wanted.sort_unstable();
    wanted.dedup();
    let shots = dir.join("screenshots");
    let thumbs = dir.join(DIR);
    let mut have = HashSet::new();
    // Grouped by folder, as the tool keeps names but not folders.
    let mut stale: BTreeMap<PathBuf, Vec<PathBuf>> = BTreeMap::new();
    for rest in &wanted {
        let (from, to) = (shots.join(rest), thumbs.join(rest));
        if !from.is_file() {
            continue;
        }
        if to.is_file() && !newer(&from, &to) {
            have.insert(rest.to_string());
        } else if let Some(parent) = to.parent() {
            stale.entry(parent.to_path_buf()).or_default().push(from);
        }
    }
    if stale.is_empty() {
        return have;
    }
    let Some(tool) = find_tool() else {
        return have;
    };
    for (out, files) in &stale {
        if std::fs::create_dir_all(out).is_err() {
            continue;
        }
        for batch in files.chunks(BATCH) {
            tool.shrink(batch, out);
        }
    }
    for rest in wanted {
        let to = thumbs.join(rest);
        if to.is_file() && !newer(&shots.join(rest), &to) {
            have.insert(rest.to_string());
        }
    }
    have
}
