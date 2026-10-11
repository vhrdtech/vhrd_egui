//! The CPU painter: the same marks as plain egui shapes, for backends without wgpu (glow, wasm).

use egui::{Color32, CornerRadius, Rect, Shape, Vec2};

use crate::field::Scene;
use crate::settings::Mark;

/// Marks dimmer than this are not worth a shape.
const MIN_ALPHA: f32 = 1.0 / 255.0;
/// Marks smaller than this (radius, points) are not worth a shape.
const MIN_HALF: f32 = 0.05;

/// `scene` as egui shapes: the background fill, then one circle or square per visible mark.
pub(crate) fn shape(scene: &Scene) -> Shape {
    let (cols, rows) = scene.cells();
    let mut shapes = Vec::with_capacity(cols * rows + 1);
    if scene.bg_alpha > 0.0 {
        shapes.push(Shape::rect_filled(
            scene.rect,
            CornerRadius::ZERO,
            scene.colors.bg.gamma_multiply(scene.bg_alpha),
        ));
    }
    for row in 0..rows {
        for col in 0..cols {
            let mark = scene.sample(scene.cell_center(col, row));
            if mark.rgba[3] < MIN_ALPHA || mark.half < MIN_HALF {
                continue;
            }
            let [r, g, b, a] = mark.rgba.map(|c| (c * 255.0).round() as u8);
            let color = Color32::from_rgba_unmultiplied(r, g, b, a);
            shapes.push(match scene.settings.mark {
                Mark::Dots => Shape::circle_filled(mark.center, mark.half, color),
                Mark::Squares => Shape::rect_filled(
                    Rect::from_center_size(mark.center, Vec2::splat(mark.half * 2.0)),
                    CornerRadius::ZERO,
                    color,
                ),
            });
        }
    }
    Shape::Vec(shapes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::test_scene;

    fn count(shape: &Shape) -> usize {
        match shape {
            Shape::Vec(shapes) => shapes.len(),
            _ => 1,
        }
    }

    #[test]
    fn one_shape_per_mark_plus_the_fill() {
        let scene = test_scene();
        assert_eq!(count(&shape(&scene)), 19 * 14 + 1);
    }

    #[test]
    fn invisible_marks_are_skipped() {
        let mut scene = test_scene();
        scene.settings.opacity = 0.0;
        scene.bg_alpha = 0.0;
        assert_eq!(count(&shape(&scene)), 0);
    }
}
