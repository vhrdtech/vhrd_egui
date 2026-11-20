//! SVG icon to coverage masks, one per paint-order layer of a colour slot (CHR-7).
//!
//! The SVG is parsed once ([`Shapes::parse`]): the XML is checked against the slot convention (see the
//! [module docs](super)), then usvg resolves it with a style sheet that paints each slot class in a code
//! colour, so every shape's slot is read back from its paint. Each shape becomes one filled outline: a stroke
//! is converted to its outline by tiny-skia's stroker before rasterising, so a stroke of one device pixel or
//! less is drawn as wide as it is (tiny-skia's hairline path would draw it thinner). Consecutive shapes of one
//! slot share a mask; a slot that comes back after another starts a new mask, so masks painted in order keep
//! the SVG's z-order.

use std::fmt;

use resvg::tiny_skia::{self, FillRule, Mask, PathStroker, StrokeDash, Transform};
use resvg::usvg::{self, Node, Paint, roxmltree};

use super::Slot;

/// The largest icon side in pixels that is rasterised.
pub const ICON_PX_MAX: u32 = 1024;

/// The longest SVG text accepted, bytes.
pub const SVG_BYTES_MAX: usize = 64 * 1024;

/// The most elements an icon may have.
pub const ELEMENTS_MAX: usize = 512;

/// The most masks (runs of shapes of one slot) an icon may have.
pub const LAYERS_MAX: usize = 16;

/// The most texture bytes one raster may give (`layers * size_px * size_px * 4`), well below the cache's
/// bound; while rendering, one supersampled layer of up to `(ICON_PX_MAX * SUPERSAMPLE)^2` bytes is alive.
pub const RASTER_BYTES_MAX: usize = 4 * 1024 * 1024;

/// Each mask is rendered at this many times its side and box-filtered down: tiny-skia's own anti-aliasing
/// misses a thin stroke's area by up to 17 % at 12 px, 4 times over by 3 % at most.
pub const SUPERSAMPLE: u32 = 4;

/// The green and blue of the code colours that mark a slot in the parsed tree (red is the slot's index + 1).
const CODE_GB: (u8, u8) = (0x5a, 0xa5);

/// One layer's coverage: `size_px` by `size_px` alpha values, row by row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlotMask {
    /// The slot the mask is tinted with.
    pub slot: Slot,
    /// The mask's side in pixels.
    pub size_px: u32,
    /// Coverage, 0 (none) to 255 (full), `size_px * size_px` values.
    pub alpha: Vec<u8>,
}

/// Why an icon could not be rasterised.
#[derive(Debug)]
pub enum IconError {
    /// A `class` that is not one `f-SLOT` or `s-SLOT` with a known slot name.
    Class(String),
    /// A shape in an icon with slots whose paint does not come from a slot class.
    Unslotted,
    /// A view box that is not square, its width and height.
    NotSquare(f32, f32),
    /// An SVG feature the icon raster does not draw: text, images, gradients, opacity, clips, masks, filters.
    Unsupported(&'static str),
    /// The SVG text is longer than [`SVG_BYTES_MAX`] or has more than [`ELEMENTS_MAX`] elements.
    TooLarge,
    /// The size is 0 or more than [`ICON_PX_MAX`].
    Size(u32),
    /// More masks than [`LAYERS_MAX`]: the count.
    Layers(usize),
    /// Layers at a size that give more than [`RASTER_BYTES_MAX`] of texture: the layers and the size in pixels.
    RasterTooLarge(usize, u32),
    /// The text is not XML.
    Xml(roxmltree::Error),
    /// usvg refuses the SVG.
    Parse(usvg::Error),
}

impl fmt::Display for IconError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Class(c) => write!(
                f,
                "class {c:?} is not a slot: use one f-NAME or s-NAME, NAME one of ink, muted, accent, good, warn, crit"
            ),
            Self::Unslotted => write!(f, "a shape has no slot class in an icon with slots"),
            Self::NotSquare(w, h) => write!(f, "the view box is {w} by {h}, not square"),
            Self::Unsupported(what) => write!(f, "{what} is not supported in an icon"),
            Self::TooLarge => write!(
                f,
                "more than {SVG_BYTES_MAX} bytes or {ELEMENTS_MAX} elements"
            ),
            Self::Size(px) => write!(f, "icon size {px} px is outside 1 to {ICON_PX_MAX}"),
            Self::Layers(n) => write!(
                f,
                "{n} layers; an icon has at most {LAYERS_MAX} runs of one slot"
            ),
            Self::RasterTooLarge(layers, px) => write!(
                f,
                "{layers} layers at {px} px exceed {RASTER_BYTES_MAX} bytes of texture"
            ),
            Self::Xml(e) => write!(f, "not XML: {e}"),
            Self::Parse(e) => write!(f, "SVG does not parse: {e}"),
        }
    }
}

impl std::error::Error for IconError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Xml(e) => Some(e),
            Self::Parse(e) => Some(e),
            _ => None,
        }
    }
}

impl From<usvg::Error> for IconError {
    fn from(e: usvg::Error) -> Self {
        Self::Parse(e)
    }
}

impl From<roxmltree::Error> for IconError {
    fn from(e: roxmltree::Error) -> Self {
        Self::Xml(e)
    }
}

/// The style sheet that paints every slot class in its code colour.
fn style_sheet() -> String {
    let (g, b) = CODE_GB;
    let mut css = String::new();
    for (i, s) in Slot::ALL.into_iter().enumerate() {
        let n = s.name();
        let r = i + 1;
        css.push_str(&format!(
            ".f-{n}{{fill:#{r:02x}{g:02x}{b:02x};stroke:none}}.s-{n}{{fill:none;stroke:#{r:02x}{g:02x}{b:02x}}}"
        ));
    }
    css
}

/// Checks the XML against the convention; true when the icon uses slot classes.
fn check_xml(doc: &roxmltree::Document) -> Result<bool, IconError> {
    let mut slotted = false;
    for (i, node) in doc.descendants().filter(|n| n.is_element()).enumerate() {
        if i >= ELEMENTS_MAX {
            return Err(IconError::TooLarge);
        }
        match node.tag_name().name() {
            "text" => return Err(IconError::Unsupported("text")),
            "image" => return Err(IconError::Unsupported("an image")),
            "style" => return Err(IconError::Unsupported("a style element")),
            _ => {}
        }
        let Some(class) = node.attribute("class") else {
            continue;
        };
        let mut tokens = class.split_whitespace();
        let (Some(token), None) = (tokens.next(), tokens.next()) else {
            return Err(IconError::Class(class.to_owned()));
        };
        token
            .strip_prefix("f-")
            .or_else(|| token.strip_prefix("s-"))
            .and_then(Slot::from_name)
            .ok_or_else(|| IconError::Class(token.to_owned()))?;
        slotted = true;
    }
    Ok(slotted)
}

/// The slot a paint stands for: its code colour in an icon with slots, ink in one without.
fn slot_of(paint: &Paint, opacity: f32, slotted: bool) -> Result<Slot, IconError> {
    if opacity < 1.0 {
        return Err(IconError::Unsupported("opacity"));
    }
    let Paint::Color(c) = paint else {
        return Err(IconError::Unsupported("a gradient or pattern"));
    };
    if !slotted {
        return Ok(Slot::Ink);
    }
    if (c.green, c.blue) != CODE_GB {
        return Err(IconError::Unslotted);
    }
    Slot::ALL
        .get(usize::from(c.red).wrapping_sub(1))
        .copied()
        .ok_or(IconError::Unslotted)
}

/// One filled outline of a shape, in view box units, with its slot.
struct Shape {
    slot: Slot,
    outline: tiny_skia::Path,
    rule: FillRule,
    transform: Transform,
    anti_alias: bool,
}

/// An icon parsed once: its shapes as filled outlines in paint order, ready to rasterise at any size.
pub struct Shapes {
    side: f32,
    shapes: Vec<Shape>,
    layers: usize,
}

impl Shapes {
    /// Parse and check `svg`.
    ///
    /// # Errors
    ///
    /// [`IconError`] for text that is not XML or SVG, breaks the slot convention, or uses what an icon may not.
    pub fn parse(svg: &str) -> Result<Self, IconError> {
        if svg.len() > SVG_BYTES_MAX {
            return Err(IconError::TooLarge);
        }
        let doc = roxmltree::Document::parse(svg)?;
        let slotted = check_xml(&doc)?;
        let opt = usvg::Options {
            style_sheet: slotted.then(style_sheet),
            ..usvg::Options::default()
        };
        let tree = usvg::Tree::from_xmltree(&doc, &opt)?;
        let size = tree.size();
        if size.width() != size.height() {
            return Err(IconError::NotSquare(size.width(), size.height()));
        }
        let mut shapes = Vec::new();
        // Outlines are flattened for the largest size an icon is drawn at, so curves stay smooth there.
        let walk = Walk {
            slotted,
            px_per_unit_max: ICON_PX_MAX as f32 / size.width(),
        };
        walk.group(tree.root(), &mut shapes)?;
        let layers = shapes
            .iter()
            .enumerate()
            .filter(|(i, sh)| *i == 0 || shapes[i - 1].slot != sh.slot)
            .count();
        if layers > LAYERS_MAX {
            return Err(IconError::Layers(layers));
        }
        Ok(Self {
            side: size.width(),
            shapes,
            layers,
        })
    }

    /// How many masks a raster gives.
    pub fn layers(&self) -> usize {
        self.layers
    }

    /// Rasterise at `size_px` by `size_px`: one mask per run of shapes of one slot, in paint order.
    ///
    /// # Errors
    ///
    /// [`IconError::Size`] for a size outside 1 to [`ICON_PX_MAX`], [`IconError::RasterTooLarge`] past
    /// [`RASTER_BYTES_MAX`].
    pub fn render(&self, size_px: u32) -> Result<Vec<SlotMask>, IconError> {
        if size_px == 0 || size_px > ICON_PX_MAX {
            return Err(IconError::Size(size_px));
        }
        let side = size_px as usize;
        if self.layers * side * side * 4 > RASTER_BYTES_MAX {
            return Err(IconError::RasterTooLarge(self.layers, size_px));
        }
        let mut out = Vec::with_capacity(self.layers);
        self.each_layer(size_px * SUPERSAMPLE, |slot, mask| {
            out.push(SlotMask {
                slot,
                size_px,
                alpha: box_filter(mask.data(), size_px, SUPERSAMPLE),
            });
        })?;
        Ok(out)
    }

    /// Each layer as a tiny-skia mask of `side` by `side` pixels, one at a time, in paint order.
    fn each_layer(&self, side: u32, mut f: impl FnMut(Slot, &Mask)) -> Result<(), IconError> {
        let scale = side as f32 / self.side;
        let mut shapes = self.shapes.iter().peekable();
        while let Some(first) = shapes.next() {
            let mut mask = Mask::new(side, side).ok_or(IconError::Size(side))?;
            let mut shape = first;
            loop {
                let ts = shape.transform.post_scale(scale, scale);
                mask.fill_path(&shape.outline, shape.rule, shape.anti_alias, ts);
                match shapes.next_if(|next| next.slot == first.slot) {
                    Some(next) => shape = next,
                    None => break,
                }
            }
            f(first.slot, &mask);
        }
        Ok(())
    }
}

/// Each `k` by `k` block of a square `side * k` mask averaged, rounded to the nearest value.
fn box_filter(big: &[u8], side: u32, k: u32) -> Vec<u8> {
    let (side, k) = (side as usize, k as usize);
    let stride = side * k;
    debug_assert_eq!(big.len(), stride * stride, "the mask is k times the side");
    let n = (k * k) as u32;
    let mut out = Vec::with_capacity(side * side);
    for y in 0..side {
        for x in 0..side {
            let mut sum = 0_u32;
            for row in big[(y * k) * stride..(y * k + k) * stride].chunks_exact(stride) {
                sum += row[x * k..x * k + k]
                    .iter()
                    .map(|&a| u32::from(a))
                    .sum::<u32>();
            }
            let avg = (sum + n / 2) / n;
            out.push(u8::try_from(avg).expect("an average of u8 values fits a u8"));
        }
    }
    out
}

/// What every shape of one icon is read with.
struct Walk {
    slotted: bool,
    px_per_unit_max: f32,
}

impl Walk {
    fn group(&self, group: &usvg::Group, out: &mut Vec<Shape>) -> Result<(), IconError> {
        if group.opacity().get() < 1.0 {
            return Err(IconError::Unsupported("opacity"));
        }
        if group.clip_path().is_some() || group.mask().is_some() {
            return Err(IconError::Unsupported("a clip path or mask"));
        }
        if !group.filters().is_empty() {
            return Err(IconError::Unsupported("a filter"));
        }
        for node in group.children() {
            match node {
                Node::Group(g) => self.group(g, out)?,
                Node::Path(p) => self.path(p, out)?,
                Node::Image(_) => return Err(IconError::Unsupported("an image")),
                Node::Text(_) => return Err(IconError::Unsupported("text")),
            }
        }
        Ok(())
    }

    fn path(&self, p: &usvg::Path, out: &mut Vec<Shape>) -> Result<(), IconError> {
        if !p.is_visible() {
            return Ok(());
        }
        let transform = p.abs_transform();
        let anti_alias = p.rendering_mode() != usvg::ShapeRendering::CrispEdges;
        let fill = match p.fill() {
            Some(f) => Some(Shape {
                slot: slot_of(f.paint(), f.opacity().get(), self.slotted)?,
                outline: p.data().clone(),
                rule: match f.rule() {
                    usvg::FillRule::NonZero => FillRule::Winding,
                    usvg::FillRule::EvenOdd => FillRule::EvenOdd,
                },
                transform,
                anti_alias,
            }),
            None => None,
        };
        let stroke = match p.stroke() {
            Some(s) => {
                let slot = slot_of(s.paint(), s.opacity().get(), self.slotted)?;
                let res = PathStroker::compute_resolution_scale(&transform) * self.px_per_unit_max;
                let mut data = p.data().clone();
                if let Some(dash) = s.dasharray() {
                    data = StrokeDash::new(dash.to_vec(), s.dashoffset())
                        .and_then(|d| data.dash(&d, res))
                        .ok_or(IconError::Unsupported("this dash pattern"))?;
                }
                // The stroke as its filled outline: never tiny-skia's hairline path, which draws a stroke of
                // one device pixel or less thinner than it is.
                data.stroke(&s.to_tiny_skia(), res).map(|outline| Shape {
                    slot,
                    outline,
                    rule: FillRule::Winding,
                    transform,
                    anti_alias,
                })
            }
            None => None,
        };
        let (first, second) = match p.paint_order() {
            usvg::PaintOrder::FillAndStroke => (fill, stroke),
            usvg::PaintOrder::StrokeAndFill => (stroke, fill),
        };
        out.extend(first);
        out.extend(second);
        Ok(())
    }
}

/// Rasterise `svg` at `size_px` by `size_px`: [`Shapes::parse`], then [`Shapes::render`].
///
/// # Errors
///
/// [`IconError`] from either step.
pub fn rasterise(svg: &str, size_px: u32) -> Result<Vec<SlotMask>, IconError> {
    if size_px == 0 || size_px > ICON_PX_MAX {
        return Err(IconError::Size(size_px));
    }
    Shapes::parse(svg)?.render(size_px)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::icons::own;

    fn svg(body: &str) -> String {
        format!(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">{body}</svg>"#)
    }

    fn slots(masks: &[SlotMask]) -> Vec<Slot> {
        masks.iter().map(|m| m.slot).collect()
    }

    fn ink(m: &SlotMask) -> u64 {
        m.alpha.iter().map(|&a| u64::from(a)).sum()
    }

    /// Each layer's ink at `k` times the size per pixel of `px`: its area, all but exactly.
    fn reference(shapes: &Shapes, px: u32, k: u32) -> Vec<f64> {
        let mut out = Vec::new();
        shapes
            .each_layer(px * k, |_, m| {
                let sum: u64 = m.data().iter().map(|&a| u64::from(a)).sum();
                out.push(sum as f64 / f64::from(k * k));
            })
            .unwrap();
        out
    }

    /// Strokes are drawn as wide as they are at every size, also where a stroke is exactly one device pixel
    /// (1.5 units at 16 px), where tiny-skia's hairline path drew up to a quarter too little ink; and the
    /// supersample keeps each layer's ink within 3.5 % of its area, the whole icon within 2 %.
    #[test]
    fn ink_matches_a_sixteen_times_supersampled_reference() {
        for icon in own::ALL {
            let shapes = Shapes::parse(icon.svg).expect(icon.name);
            for px in 12..=32 {
                let masks = shapes.render(px).unwrap();
                let want = reference(&shapes, px, 16);
                assert_eq!(masks.len(), want.len());
                let got: Vec<f64> = masks.iter().map(|m| ink(m) as f64).collect();
                for ((m, g), w) in masks.iter().zip(&got).zip(&want) {
                    assert!(
                        (g - w).abs() <= w * 0.035,
                        "{} at {px} px, {:?}: ink {g} vs area {w}",
                        icon.name,
                        m.slot
                    );
                }
                let (g, w): (f64, f64) = (got.iter().sum(), want.iter().sum());
                assert!(
                    (g - w).abs() <= w * 0.02,
                    "{} at {px} px: ink {g} vs area {w}",
                    icon.name
                );
            }
        }
    }

    #[test]
    fn box_filter_averages_blocks() {
        let big = [0, 255, 255, 255, 0, 0, 0, 1, 10, 10, 0, 0, 10, 10, 0, 0];
        assert_eq!(box_filter(&big, 2, 2), [64, 128, 10, 0]);
    }

    #[test]
    fn own_icons_rasterise_with_their_slots() {
        for icon in own::ALL {
            let masks = rasterise(icon.svg, 24).expect(icon.name);
            assert!(masks.len() >= 2, "{} has several slots", icon.name);
            for m in &masks {
                assert_eq!(m.alpha.len(), 24 * 24);
                assert!(
                    m.alpha.contains(&255),
                    "{} {:?} covers whole pixels",
                    icon.name,
                    m.slot
                );
            }
        }
    }

    #[test]
    fn a_mask_holds_only_its_shapes() {
        let icon = svg(
            r#"<rect class="f-ink" width="12" height="24"/><rect class="f-accent" x="12" width="12" height="24"/>"#,
        );
        let masks = rasterise(&icon, 4).unwrap();
        assert_eq!(slots(&masks), [Slot::Ink, Slot::Accent]);
        assert_eq!(masks[0].alpha[..4], [255, 255, 0, 0]);
        assert_eq!(masks[1].alpha[..4], [0, 0, 255, 255]);
    }

    /// A slot that comes back after another is a new mask, so painting the masks in order keeps the z-order.
    #[test]
    fn interleaved_slots_keep_their_z_order() {
        let icon = svg(
            r#"<rect class="f-ink" width="24" height="24"/><rect class="f-accent" width="24" height="24"/><rect class="f-ink" width="12" height="24"/>"#,
        );
        let masks = rasterise(&icon, 4).unwrap();
        assert_eq!(slots(&masks), [Slot::Ink, Slot::Accent, Slot::Ink]);
        assert_eq!(masks[2].alpha[..4], [255, 255, 0, 0]);
    }

    #[test]
    fn single_quoted_classes_are_slots() {
        let icon = svg("<rect class='f-accent' width='24' height='24'/>");
        assert_eq!(slots(&rasterise(&icon, 4).unwrap()), [Slot::Accent]);
    }

    #[test]
    fn a_class_on_a_group_slots_its_children() {
        let icon = svg(r#"<g class="s-muted"><path d="M2 12h20" stroke-width="2"/></g>"#);
        assert_eq!(slots(&rasterise(&icon, 8).unwrap()), [Slot::Muted]);
    }

    #[test]
    fn a_plain_svg_is_one_ink_mask() {
        let icon = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 2 2" fill="currentColor"><!-- class="f-pink" --><rect width="1" height="2"/></svg>"#;
        let masks = rasterise(icon, 2).unwrap();
        assert_eq!(slots(&masks), [Slot::Ink]);
        assert_eq!(masks[0].alpha, [255, 0, 255, 0]);
    }

    #[test]
    fn breaking_the_convention_is_an_error() {
        let err = |body: &str| rasterise(&svg(body), 16).unwrap_err();
        assert!(matches!(
            err(r#"<rect class="f-pink" width="1" height="1"/>"#),
            IconError::Class(c) if c == "f-pink"
        ));
        assert!(matches!(
            err(r#"<rect class="f-ink s-ink" width="1" height="1"/>"#),
            IconError::Class(_)
        ));
        assert!(matches!(
            err(r#"<rect class="f-ink" width="1" height="1"/><rect width="1" height="1"/>"#),
            IconError::Unslotted
        ));
        assert!(matches!(
            err(r#"<rect class="f-ink" width="1" height="1" fill-opacity="0.5"/>"#),
            IconError::Unsupported("opacity")
        ));
        assert!(matches!(
            err("<text>A</text>"),
            IconError::Unsupported("text")
        ));
        let wide = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 4 2"><rect class="f-ink" width="4" height="2"/></svg>"#;
        assert!(matches!(
            rasterise(wide, 4),
            Err(IconError::NotSquare(w, h)) if w == 4.0 && h == 2.0
        ));
    }

    /// Layers are bounded at parse, texture bytes at render: 200 alternating shapes at 256 px took 209 MB
    /// while rendering and 52 MB of textures before these limits.
    #[test]
    fn layers_and_raster_bytes_are_bounded() {
        let alternate = |n: usize| {
            svg(&(0..n)
                .map(|i| {
                    let slot = if i % 2 == 0 { "ink" } else { "accent" };
                    format!(
                        r#"<rect class="f-{slot}" x="{}" width="1" height="24"/>"#,
                        i % 24
                    )
                })
                .collect::<String>())
        };
        let at_limit = Shapes::parse(&alternate(LAYERS_MAX)).unwrap();
        assert_eq!(at_limit.layers(), LAYERS_MAX);
        assert_eq!(
            at_limit.render(256).unwrap().len(),
            LAYERS_MAX,
            "16 layers at 256 px is 4 MiB"
        );
        assert!(matches!(
            at_limit.render(257),
            Err(IconError::RasterTooLarge(16, 257))
        ));
        assert!(matches!(
            Shapes::parse(&alternate(200)),
            Err(IconError::Layers(200))
        ));
        let tape = Shapes::parse(own::TAPE.svg).unwrap();
        assert!(matches!(
            tape.render(ICON_PX_MAX),
            Err(IconError::RasterTooLarge(3, ICON_PX_MAX))
        ));
        let one = svg(r#"<rect class="f-ink" width="24" height="24"/>"#);
        let full = rasterise(&one, ICON_PX_MAX).unwrap();
        assert_eq!(full[0].alpha.len(), (ICON_PX_MAX * ICON_PX_MAX) as usize);
    }

    #[test]
    fn bad_input_is_an_error() {
        assert!(matches!(
            rasterise(own::TAPE.svg, 0),
            Err(IconError::Size(0))
        ));
        assert!(matches!(
            rasterise(own::TAPE.svg, ICON_PX_MAX + 1),
            Err(IconError::Size(_))
        ));
        assert!(matches!(rasterise("<svg", 16), Err(IconError::Xml(_))));
        let long = svg(&"<g/>".repeat(ELEMENTS_MAX));
        assert!(matches!(rasterise(&long, 16), Err(IconError::TooLarge)));
        let huge = " ".repeat(SVG_BYTES_MAX + 1);
        assert!(matches!(rasterise(&huge, 16), Err(IconError::TooLarge)));
    }
}
