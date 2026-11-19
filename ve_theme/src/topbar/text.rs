//! Cutting text to fit: a file name in the middle, keeping its extension (E2: `bench-1…26.efp`); a name at
//! its end.

/// `name` cut in the middle with "…" so that `width(cut) <= max`, keeping the extension and as much of both
/// ends as fits; the whole name when it fits. `width` measures a candidate (points in a font). When not even
/// "…" and the extension fit, gives "…".
pub(crate) fn fit_middle(name: &str, max: f32, width: impl Fn(&str) -> f32) -> String {
    if width(name) <= max {
        return name.to_owned();
    }
    let chars: Vec<char> = name.chars().collect();
    let ext_at = name
        .rfind('.')
        .filter(|&i| i > 0)
        .map_or(chars.len(), |i| name[..i].chars().count());
    let (stem, ext) = chars.split_at(ext_at);
    let make = |keep: usize| -> String {
        let head = keep.div_ceil(2).min(stem.len());
        let tail = (keep - head).min(stem.len() - head);
        let mut s: String = stem[..head].iter().collect();
        s.push('…');
        s.extend(&stem[stem.len() - tail..]);
        s.extend(ext);
        s
    };
    // The most stem characters that still fit; widths grow with `keep`, so a binary search finds it.
    let (mut lo, mut hi) = (0usize, stem.len());
    if width(&make(0)) > max {
        return "…".to_owned();
    }
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        if width(&make(mid)) <= max {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    make(lo)
}

/// `text` cut at its end with "…" so that `width(cut) <= max`; the whole text when it fits, "…" when not even
/// one character and "…" fit.
pub(crate) fn fit_end(text: &str, max: f32, width: impl Fn(&str) -> f32) -> String {
    if width(text) <= max {
        return text.to_owned();
    }
    let chars: Vec<char> = text.chars().collect();
    let make = |keep: usize| -> String {
        let mut s: String = chars[..keep].iter().collect();
        s.push('…');
        s
    };
    let (mut lo, mut hi) = (0usize, chars.len());
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        if width(&make(mid)) <= max {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    make(lo)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chars(s: &str) -> f32 {
        s.chars().count() as f32
    }

    #[test]
    fn keeps_the_extension_and_both_ends() {
        assert_eq!(
            fit_middle("bench-18nov2026.efp", 100.0, chars),
            "bench-18nov2026.efp"
        );
        assert_eq!(
            fit_middle("bench-18nov2026.efp", 14.0, chars),
            "bench…2026.efp"
        );
        assert_eq!(fit_middle("bench-18nov2026.efp", 5.0, chars), "….efp");
        assert_eq!(fit_middle("bench-18nov2026.efp", 3.0, chars), "…");
        assert_eq!(
            fit_middle("Makefile-without-extension", 10.0, chars),
            "Makef…sion"
        );
        assert_eq!(fit_middle(".hidden-file", 6.0, chars), ".hi…le");
    }

    #[test]
    fn cuts_a_name_at_its_end() {
        assert_eq!(fit_end("eventfull", 20.0, chars), "eventfull");
        assert_eq!(fit_end("eventfull", 4.0, chars), "eve…");
        assert_eq!(fit_end("eventfull", 0.5, chars), "…");
    }

    #[test]
    fn never_wider_than_asked() {
        let name = "a-very-long-name-of-a-capture-file.efp";
        for max in 1..50 {
            let cut = fit_middle(name, max as f32, chars);
            assert!(chars(&cut) <= (max as f32).max(1.0), "{max}: {cut}");
        }
    }
}
