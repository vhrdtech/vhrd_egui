//! The desktop wallpaper for the emulated transparency (BACK-2): where it is ([`find`]) and a small, blurred
//! copy of it ([`load`], [`Small::frosted`]). [`find`] and [`load`] block (files, a decoder, sometimes a
//! command) and run on the worker thread of `glass.rs`, never in `ui()`.
//!
//! There is no standard way to ask a Linux desktop for its wallpaper, so [`find`] is a list of known places;
//! an app or the user can always name the file instead (`Backdrop::set_wallpaper`, `VE_WALLPAPER`).

use std::path::{Path, PathBuf};

/// Environment variable naming the wallpaper file; wins over every lookup.
pub const WALLPAPER_ENV: &str = "VE_WALLPAPER";

/// Box-blur radius at `blur = 1`, as a fraction of the copy's width; three passes make it about a Gaussian.
const MAX_BLUR: f32 = 0.06;
/// A downsample that throws away less than this many pixels of the deliberate blur's own radius
/// ([`MAX_BLUR`] * `blur`, itself a fraction of the width, so the radius in display pixels does not depend
/// on the working resolution chosen) is not visible under it. Chosen close to the old fixed 320 px working
/// width at the old default blur of 0.7, so [`working_width`] refines that behaviour rather than jumping it.
const DOWNSAMPLE_MARGIN: f32 = 16.0;

/// How wide to decode the wallpaper, in pixels, for a given [`crate::Settings::glass_blur`]: `full` (the
/// window's monitor resolution, capped by [`crate::Settings::glass_max_side`]) when sharp, shrinking towards
/// `min_width` ([`crate::Settings::glass_min_width`]) as the blur grows, since a stronger blur hides finer
/// detail anyway. See [`DOWNSAMPLE_MARGIN`].
pub(crate) fn working_width(blur: f32, full: u32, min_width: u32) -> u32 {
    let full = full.max(1);
    let floor = min_width.min(full);
    if blur <= 0.0 {
        return full;
    }
    let by_blur = (DOWNSAMPLE_MARGIN / (MAX_BLUR * blur)).round().max(1.0) as u32;
    by_blur.clamp(floor, full)
}

/// A small copy of a wallpaper, not blurred yet.
#[derive(Clone, Debug, PartialEq)]
pub struct Small {
    /// Width and height in pixels.
    pub size: [usize; 2],
    /// RGB as floats (0-255), row by row.
    rgb: Vec<f32>,
    /// Width / height of the original image.
    pub aspect: f32,
}

/// A small blurred copy of a wallpaper.
#[derive(Clone, Debug, PartialEq)]
pub struct Frosted {
    /// Width and height in pixels.
    pub size: [usize; 2],
    /// Opaque RGBA, row by row.
    pub rgba: Vec<u8>,
    /// Width / height of the original image.
    pub aspect: f32,
}

/// Decode the image at `path` and shrink it to `target_width` (never upscaled past the image's own size).
/// This is the slow part (a big JPEG takes a moment). `max_side` and `max_alloc` are decode safety caps
/// ([`crate::Settings::glass_max_side`], [`crate::Settings::glass_max_alloc`]).
pub fn load(
    path: &Path,
    target_width: u32,
    max_side: u32,
    max_alloc: u64,
) -> Result<Small, String> {
    let mut reader = image::ImageReader::open(path)
        .map_err(|e| e.to_string())?
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(max_side);
    limits.max_image_height = Some(max_side);
    limits.max_alloc = Some(max_alloc);
    reader.limits(limits);
    let image = reader.decode().map_err(|e| e.to_string())?;
    let (w, h) = (image.width(), image.height());
    if w == 0 || h == 0 {
        return Err("the image is empty".to_owned());
    }
    let aspect = w as f32 / h as f32;
    let small_w = target_width.max(1).min(w);
    let small_h = ((small_w as f32 / aspect).round() as u32).clamp(1, max_side);
    let small = image.thumbnail_exact(small_w, small_h).to_rgb8();
    Ok(Small {
        size: [small_w as usize, small_h as usize],
        rgb: small.as_raw().iter().map(|c| f32::from(*c)).collect(),
        aspect,
    })
}

impl Small {
    /// The copy blurred by `blur` (0 sharp, 1 strongest). Fast: a millisecond or so.
    pub fn frosted(&self, blur: f32) -> Frosted {
        let mut rgb = self.rgb.clone();
        let radius = (blur.clamp(0.0, 1.0) * MAX_BLUR * self.size[0] as f32).round() as usize;
        blur_rgb(&mut rgb, self.size, radius);
        let mut rgba = Vec::with_capacity(rgb.len() / 3 * 4);
        for px in rgb.as_chunks::<3>().0 {
            rgba.extend(px.iter().map(|c| c.round().clamp(0.0, 255.0) as u8));
            rgba.push(255);
        }
        Frosted {
            size: self.size,
            rgba,
            aspect: self.aspect,
        }
    }
}

/// Three box blurs of `radius` in each direction (close to a Gaussian), edges repeated.
pub(crate) fn blur_rgb(rgb: &mut [f32], size: [usize; 2], radius: usize) {
    if radius == 0 {
        return;
    }
    let [w, h] = size;
    let mut line = vec![0.0; w.max(h) * 3];
    for _ in 0..3 {
        for y in 0..h {
            box_blur_line(rgb, y * w * 3, 3, w, radius, &mut line);
        }
        for x in 0..w {
            box_blur_line(rgb, x * 3, w * 3, h, radius, &mut line);
        }
    }
}

/// Box-blur one row or column of `n` RGB pixels starting at `start`, `stride` apart, with a running sum.
fn box_blur_line(
    rgb: &mut [f32],
    start: usize,
    stride: usize,
    n: usize,
    radius: usize,
    line: &mut [f32],
) {
    let at = |i: isize| start + (i.clamp(0, n as isize - 1) as usize) * stride;
    let r = radius as isize;
    let window = (2 * radius + 1) as f32;
    for c in 0..3 {
        let mut sum: f32 = (-r..=r).map(|i| rgb[at(i) + c]).sum();
        for i in 0..n as isize {
            line[i as usize * 3 + c] = sum / window;
            sum += rgb[at(i + r + 1) + c] - rgb[at(i - r) + c];
        }
    }
    for i in 0..n {
        let p = start + i * stride;
        rgb[p..p + 3].copy_from_slice(&line[i * 3..i * 3 + 3]);
    }
}

/// The wallpaper of this desktop, as far as it can be found: `VE_WALLPAPER`, then the known places of the
/// platform. `dark` picks the dark variant where a desktop has two (GNOME), `output` the monitor where each
/// has its own (hyprpaper).
pub fn find(dark: bool, output: Option<&str>) -> Option<PathBuf> {
    if let Some(path) = std::env::var_os(WALLPAPER_ENV).map(PathBuf::from)
        && path.is_file()
    {
        return Some(path);
    }
    platform::find(dark, output)
}

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

/// `~/x` with the home directory filled in.
fn expand_home(path: &str) -> PathBuf {
    match (path.strip_prefix("~/"), home()) {
        (Some(rest), Some(home)) => home.join(rest),
        _ => PathBuf::from(path),
    }
}

/// A local path from a `file://` URI or a plain path, quotes and percent-escapes removed
/// (`'file:///home/u/My%20Pictures/a.jpg'`).
pub(crate) fn path_from_uri(text: &str) -> Option<PathBuf> {
    let text = text.trim().trim_matches(['\'', '"']);
    let path = text.strip_prefix("file://").unwrap_or(text);
    if path.is_empty() || path.contains("://") {
        return None;
    }
    let mut bytes = Vec::with_capacity(path.len());
    let mut rest = path.as_bytes();
    while let [first, tail @ ..] = rest {
        let hex = (*first == b'%' && tail.len() >= 2)
            .then(|| std::str::from_utf8(&tail[..2]).ok())
            .flatten()
            .and_then(|h| u8::from_str_radix(h, 16).ok());
        match hex {
            Some(byte) => {
                bytes.push(byte);
                rest = &tail[2..];
            }
            None => {
                bytes.push(*first);
                rest = tail;
            }
        }
    }
    Some(expand_home(&String::from_utf8(bytes).ok()?))
}

/// The image a hyprpaper.conf shows on `output`: its `wallpaper = monitor,path` line, else the line for
/// every monitor (`wallpaper = ,path`), else the last one.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) fn parse_hyprpaper(conf: &str, output: Option<&str>) -> Option<PathBuf> {
    let entries: Vec<(&str, PathBuf)> = conf
        .lines()
        .filter_map(|line| {
            let (key, value) = line.split('#').next()?.split_once('=')?;
            (key.trim() == "wallpaper").then_some(value)
        })
        .filter_map(|value| {
            let (monitor, path) = value.rsplit_once(',').unwrap_or(("", value));
            let path = path.trim();
            let path = path.strip_prefix("contain:").unwrap_or(path);
            (!path.is_empty()).then(|| (monitor.trim(), expand_home(path)))
        })
        .collect();
    let on = |monitor: &str| entries.iter().rev().find(|(m, _)| *m == monitor);
    output
        .and_then(on)
        .or_else(|| on(""))
        .or(entries.last())
        .map(|(_, path)| path.clone())
}

/// The last `Image=` entry of Plasma's `plasma-org.kde.plasma.desktop-appletsrc`.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) fn parse_plasma(conf: &str) -> Option<PathBuf> {
    conf.lines()
        .filter_map(|line| line.trim().strip_prefix("Image="))
        .filter_map(path_from_uri)
        .next_back()
}

/// The image a `swaybg` command line shows (`-i FILE` / `--image FILE`), arguments NUL-separated as in
/// `/proc/PID/cmdline`.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) fn parse_swaybg(cmdline: &[u8]) -> Option<PathBuf> {
    let mut args = cmdline
        .split(|b| *b == 0)
        .map(|a| String::from_utf8_lossy(a).into_owned());
    if !args.next()?.ends_with("swaybg") {
        return None;
    }
    let args: Vec<String> = args.collect();
    args.windows(2)
        .find(|pair| pair[0] == "-i" || pair[0] == "--image")
        .map(|pair| expand_home(&pair[1]))
}

#[cfg(target_os = "linux")]
mod platform {
    use super::*;

    /// A Plasma wallpaper may be a package directory: take its biggest image.
    fn plasma_image(path: PathBuf) -> Option<PathBuf> {
        if !path.is_dir() {
            return Some(path);
        }
        std::fs::read_dir(path.join("contents/images"))
            .ok()?
            .flatten()
            .filter_map(|e| Some((e.metadata().ok()?.len(), e.path())))
            .max_by_key(|(len, _)| *len)
            .map(|(_, path)| path)
    }

    fn gnome(dark: bool) -> Option<PathBuf> {
        let key = if dark {
            "picture-uri-dark"
        } else {
            "picture-uri"
        };
        let out = std::process::Command::new("gsettings")
            .args(["get", "org.gnome.desktop.background", key])
            .output()
            .ok()?;
        path_from_uri(std::str::from_utf8(&out.stdout).ok()?)
    }

    fn swaybg() -> Option<PathBuf> {
        std::fs::read_dir("/proc")
            .ok()?
            .flatten()
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .bytes()
                    .all(|b| b.is_ascii_digit())
            })
            .filter_map(|e| std::fs::read(e.path().join("cmdline")).ok())
            .find_map(|cmdline| parse_swaybg(&cmdline))
    }

    pub fn find(dark: bool, output: Option<&str>) -> Option<PathBuf> {
        let home = home()?;
        let config = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".config"));
        let desktop = std::env::var("XDG_CURRENT_DESKTOP")
            .unwrap_or_default()
            .to_lowercase();
        let read = |path: PathBuf| std::fs::read_to_string(path).ok();

        let mut candidates: Vec<Box<dyn Fn() -> Option<PathBuf>>> = vec![
            // omarchy keeps a link to the current background
            Box::new(|| Some(home.join(".local/state/omarchy/current/background"))),
            Box::new(|| Some(config.join("omarchy/current/background"))),
            Box::new(|| parse_hyprpaper(&read(config.join("hypr/hyprpaper.conf"))?, output)),
            Box::new(swaybg),
        ];
        if desktop.contains("kde") {
            candidates.push(Box::new(|| {
                let conf = read(config.join("plasma-org.kde.plasma.desktop-appletsrc"))?;
                plasma_image(parse_plasma(&conf)?)
            }));
        }
        if ["gnome", "unity", "cinnamon", "budgie", "pantheon"]
            .iter()
            .any(|d| desktop.contains(d))
        {
            candidates.push(Box::new(move || gnome(dark)));
        }
        candidates
            .iter()
            .filter_map(|candidate| candidate())
            .find(|path| path.is_file())
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use super::*;

    /// Windows keeps a copy of the current wallpaper (a JPEG without extension) in the profile.
    pub fn find(_dark: bool, _output: Option<&str>) -> Option<PathBuf> {
        let path = PathBuf::from(std::env::var_os("APPDATA")?)
            .join("Microsoft/Windows/Themes/TranscodedWallpaper");
        path.is_file().then_some(path)
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use super::*;

    pub fn find(_dark: bool, _output: Option<&str>) -> Option<PathBuf> {
        let out = std::process::Command::new("osascript")
            .args([
                "-e",
                "tell application \"System Events\" to get picture of current desktop",
            ])
            .output()
            .ok()?;
        let path = PathBuf::from(std::str::from_utf8(&out.stdout).ok()?.trim());
        path.is_file().then_some(path)
    }
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
mod platform {
    use super::*;

    pub fn find(_dark: bool, _output: Option<&str>) -> Option<PathBuf> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uris_and_paths() {
        assert_eq!(
            path_from_uri("'file:///home/u/My%20Pictures/a.jpg'\n"),
            Some(PathBuf::from("/home/u/My Pictures/a.jpg"))
        );
        assert_eq!(
            path_from_uri("/usr/share/w.png"),
            Some(PathBuf::from("/usr/share/w.png"))
        );
        assert_eq!(path_from_uri("''"), None);
        assert_eq!(path_from_uri("https://example.org/w.png"), None);
        // A stray percent sign is kept as it is.
        assert_eq!(
            path_from_uri("/tmp/100%.png"),
            Some(PathBuf::from("/tmp/100%.png"))
        );
    }

    #[test]
    fn hyprpaper_conf() {
        let conf = "preload = /a/one.png\n# wallpaper = ,/a/old.png\nwallpaper = ,/a/one.png\n\
                    wallpaper = DP-1, contain:/a/two.jpg # the big screen\n";
        let two = Some(PathBuf::from("/a/two.jpg"));
        let one = Some(PathBuf::from("/a/one.png"));
        assert_eq!(parse_hyprpaper(conf, Some("DP-1")), two);
        assert_eq!(
            parse_hyprpaper(conf, Some("eDP-1")),
            one,
            "the line for every monitor"
        );
        assert_eq!(parse_hyprpaper(conf, None), one);
        assert_eq!(
            parse_hyprpaper("wallpaper = DP-1,/a/two.jpg\n", Some("HDMI-A-1")),
            two
        );
        assert_eq!(parse_hyprpaper("splash = false\n", None), None);
    }

    #[test]
    fn plasma_conf() {
        let conf = "[Containments][1][Wallpaper][org.kde.image][General]\n\
                    Image=file:///usr/share/wallpapers/Next/\nSlidePaths=/x\n";
        assert_eq!(
            parse_plasma(conf),
            Some(PathBuf::from("/usr/share/wallpapers/Next/"))
        );
        assert_eq!(parse_plasma("[General]\n"), None);
    }

    #[test]
    fn swaybg_command_line() {
        assert_eq!(
            parse_swaybg(b"/usr/bin/swaybg\0-o\0*\0-i\0/a/w.png\0-m\0fill\0"),
            Some(PathBuf::from("/a/w.png"))
        );
        assert_eq!(parse_swaybg(b"swaybg\0-c\0#000000\0"), None);
        assert_eq!(parse_swaybg(b"bash\0-i\0/a/w.png\0"), None);
    }

    #[test]
    fn blur_keeps_the_mean_and_flattens() {
        // Left half black, right half white.
        let size = [40, 8];
        let mut rgb: Vec<f32> = (0..size[0] * size[1])
            .flat_map(|i| [if i % size[0] < 20 { 0.0 } else { 255.0 }; 3])
            .collect();
        let mean = |v: &[f32]| v.iter().sum::<f32>() / v.len() as f32;
        let before = mean(&rgb);
        blur_rgb(&mut rgb, size, 3);
        assert!((mean(&rgb) - before).abs() < 0.5);
        let px = |x: usize| rgb[x * 3];
        assert!(px(0) < 1.0 && px(39) > 254.0, "far from the edge unchanged");
        assert!(px(19) > 60.0 && px(20) < 195.0, "the edge is soft");
        assert!((0..39).all(|x| px(x) <= px(x + 1) + 1e-3), "and monotonic");
    }

    #[test]
    fn frosted_reads_shrinks_and_blurs_a_file() {
        let dir = std::env::temp_dir().join(format!("ve_backdrop_frost_{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("wall.png");
        let image = image::RgbImage::from_fn(800, 400, |x, _| {
            if x < 400 {
                image::Rgb([20, 40, 200])
            } else {
                image::Rgb([30, 180, 90])
            }
        });
        image.save(&path).expect("write the test wallpaper");

        let small = load(&path, 320, 16_384, 768 * 1024 * 1024).expect("decodes");
        let sharp = small.frosted(0.0);
        assert_eq!(sharp.size, [320, 160]);
        assert_eq!(sharp.aspect, 2.0);
        assert_eq!(&sharp.rgba[..4], &[20, 40, 200, 255]);

        let soft = small.frosted(1.0);
        let mid = (80 * 320 + 160) * 4;
        assert!(
            soft.rgba[mid + 1] > 60 && soft.rgba[mid + 1] < 160,
            "blurred edge"
        );

        // Never upscaled past the image's own size.
        let full = load(&path, 10_000, 16_384, 768 * 1024 * 1024).expect("decodes");
        assert_eq!(full.size, [800, 400]);

        assert!(load(&dir.join("missing.png"), 320, 16_384, 768 * 1024 * 1024).is_err());
        std::fs::write(dir.join("junk.png"), b"not an image").expect("write");
        assert!(load(&dir.join("junk.png"), 320, 16_384, 768 * 1024 * 1024).is_err());
        std::fs::remove_dir_all(&dir).expect("clean up");
    }

    #[test]
    fn working_width_follows_the_blur() {
        let (full, min_width) = (2560, 320);
        assert_eq!(
            working_width(0.0, full, min_width),
            full,
            "sharp: the full (monitor) resolution"
        );
        let default_blur = working_width(0.7, full, min_width);
        assert!(
            min_width <= default_blur && default_blur < full,
            "{default_blur}"
        );
        // Blurrier shrinks further, never below the floor.
        assert!(working_width(1.0, full, min_width) <= default_blur);
        assert_eq!(working_width(1.0, min_width - 1, min_width), min_width - 1);
    }

    /// What this desktop gives: `cargo test -p ve_backdrop real_desktop -- --ignored --nocapture`
    #[test]
    #[ignore = "depends on the desktop it runs on"]
    fn real_desktop() {
        let path = find(true, None).expect("a wallpaper is found on this desktop");
        let start = std::time::Instant::now();
        let small = load(&path, 2560, 16_384, 768 * 1024 * 1024).expect("it decodes");
        println!(
            "{} -> {:?}, aspect {}, read in {:?}",
            path.display(),
            small.size,
            small.aspect,
            start.elapsed()
        );
        let start = std::time::Instant::now();
        let _ = small.frosted(0.7);
        println!("blurred in {:?}", start.elapsed());
    }
}
