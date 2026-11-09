//! How click-hint labels look: the user's style, where labels go, and drawing
//! them with smooth edges into a premultiplied BGRA pixel buffer.

use serde::{Deserialize, Serialize};

pub const SIZE_RANGE: std::ops::RangeInclusive<u32> = 10..=28;
pub const RADIUS_RANGE: std::ops::RangeInclusive<u32> = 0..=14;
pub const OPACITY_RANGE: std::ops::RangeInclusive<u32> = 40..=100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HintPosition {
    /// At the item's left edge, which hides the least of its text.
    #[default]
    Left,
    Center,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct HintStyle {
    pub background: String,
    pub text: String,
    /// Colour of the letters typed so far.
    pub typed: String,
    /// Letter size in points.
    pub size: u32,
    /// Corner radius.
    pub radius: u32,
    /// Background opacity in percent.
    pub opacity: u32,
    pub position: HintPosition,
    pub shadow: bool,
}

impl Default for HintStyle {
    fn default() -> Self {
        Self {
            background: "#171B24".into(),
            text: "#FFFFFF".into(),
            typed: "#7AA7FF".into(),
            size: 14,
            radius: 7,
            opacity: 94,
            position: HintPosition::Left,
            shadow: true,
        }
    }
}

/// Red, green and blue from `#RRGGBB`.
pub fn parse_color(text: &str) -> Option<[u8; 3]> {
    let digits = text.strip_prefix('#')?;
    if digits.len() != 6 || !digits.is_ascii() {
        return None;
    }
    let channel = |at: usize| u8::from_str_radix(&digits[at..at + 2], 16).ok();
    Some([channel(0)?, channel(2)?, channel(4)?])
}

impl HintStyle {
    pub(crate) fn check(&self, errors: &mut Vec<String>) {
        for (name, value) in [
            ("background", &self.background),
            ("text", &self.text),
            ("typed", &self.typed),
        ] {
            if parse_color(value).is_none() {
                errors.push(format!("click hints: {name} must look like #66CCFF"));
            }
        }
        for (name, value, range) in [
            ("size", self.size, SIZE_RANGE),
            ("radius", self.radius, RADIUS_RANGE),
            ("opacity", self.opacity, OPACITY_RANGE),
        ] {
            if !range.contains(&value) {
                errors.push(format!(
                    "click hints: {name} must be {} to {}",
                    range.start(),
                    range.end()
                ));
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LabelBox {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl LabelBox {
    fn overlaps(&self, other: &LabelBox, gap: i32) -> bool {
        self.x < other.x + other.w + gap
            && other.x < self.x + self.w + gap
            && self.y < other.y + other.h + gap
            && other.y < self.y + self.h + gap
    }
}

const MAX_NUDGES: usize = 60;

/// Moves labels just enough that they don't cover each other, and keeps them
/// on screen. Earlier labels stay where they are.
pub fn place_labels(boxes: &[LabelBox], width: i32, height: i32, gap: i32) -> Vec<LabelBox> {
    let mut placed: Vec<LabelBox> = Vec::with_capacity(boxes.len());
    for wanted in boxes {
        let mut label = *wanted;
        label.x = label.x.clamp(0, (width - label.w).max(0));
        label.y = label.y.clamp(0, (height - label.h).max(0));
        for _ in 0..MAX_NUDGES {
            let Some(hit) = placed.iter().find(|p| label.overlaps(p, gap)) else {
                break;
            };
            label.x = hit.x + hit.w + gap;
            if label.x + label.w > width {
                label.x = wanted.x.clamp(0, (width - label.w).max(0));
                label.y = (hit.y + hit.h + gap).min((height - label.h).max(0));
            }
        }
        placed.push(label);
    }
    placed
}

/// A premultiplied BGRA pixel buffer, top row first.
pub struct Canvas<'a> {
    pixels: &'a mut [u32],
    width: i32,
    height: i32,
}

/// Distance from a point to the edge of a rounded rectangle; negative inside.
fn rounded_distance(px: f32, py: f32, b: LabelBox, radius: f32) -> f32 {
    let half_w = b.w as f32 / 2.0;
    let half_h = b.h as f32 / 2.0;
    let radius = radius.min(half_w).min(half_h);
    let qx = (px - (b.x as f32 + half_w)).abs() - half_w + radius;
    let qy = (py - (b.y as f32 + half_h)).abs() - half_h + radius;
    let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
    outside + qx.max(qy).min(0.0) - radius
}

impl<'a> Canvas<'a> {
    pub fn new(pixels: &'a mut [u32], width: i32, height: i32) -> Self {
        debug_assert_eq!(pixels.len(), (width * height) as usize);
        Self {
            pixels,
            width,
            height,
        }
    }

    pub fn clear(&mut self) {
        self.pixels.fill(0);
    }

    fn blend(&mut self, x: i32, y: i32, [r, g, b]: [u8; 3], alpha: f32) {
        if x < 0 || y < 0 || x >= self.width || y >= self.height || alpha <= 0.0 {
            return;
        }
        let pixel = &mut self.pixels[(y * self.width + x) as usize];
        let channel = |source: u8, shift: u32, dest: u32| -> u32 {
            let under = ((dest >> shift) & 0xFF) as f32;
            ((source as f32 * alpha + under * (1.0 - alpha)).round() as u32).min(255) << shift
        };
        let dest = *pixel;
        *pixel = channel(b, 0, dest)
            | channel(g, 8, dest)
            | channel(r, 16, dest)
            | channel(255, 24, dest);
    }

    /// Runs `paint` with each pixel in `b` expanded by `margin`.
    fn each_pixel(&mut self, b: LabelBox, margin: i32, mut paint: impl FnMut(&mut Self, i32, i32)) {
        for y in (b.y - margin).max(0)..(b.y + b.h + margin).min(self.height) {
            for x in (b.x - margin).max(0)..(b.x + b.w + margin).min(self.width) {
                paint(self, x, y);
            }
        }
    }

    pub fn fill_rounded(&mut self, b: LabelBox, radius: f32, color: [u8; 3], alpha: f32) {
        self.each_pixel(b, 1, |canvas, x, y| {
            let distance = rounded_distance(x as f32 + 0.5, y as f32 + 0.5, b, radius);
            canvas.blend(x, y, color, (0.5 - distance).clamp(0.0, 1.0) * alpha);
        });
    }

    /// A one-pixel line just inside the edge.
    pub fn outline_rounded(&mut self, b: LabelBox, radius: f32, color: [u8; 3], alpha: f32) {
        self.each_pixel(b, 1, |canvas, x, y| {
            let distance = rounded_distance(x as f32 + 0.5, y as f32 + 0.5, b, radius);
            let coverage = (0.5 - distance).clamp(0.0, 1.0) - (-0.5 - distance).clamp(0.0, 1.0);
            canvas.blend(x, y, color, coverage * alpha);
        });
    }

    /// A soft dark glow around a rounded rectangle.
    pub fn shadow(&mut self, b: LabelBox, radius: f32, blur: f32, alpha: f32) {
        self.each_pixel(b, blur.ceil() as i32, |canvas, x, y| {
            let distance = rounded_distance(x as f32 + 0.5, y as f32 + 0.5, b, radius);
            let fade = (1.0 - distance.max(0.0) / blur).clamp(0.0, 1.0);
            canvas.blend(x, y, [0, 0, 0], fade * fade * alpha);
        });
    }

    /// Draws a coverage mask at (`left`, `top`). Columns before `split` use
    /// `first`, the rest use `second`.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_mask(
        &mut self,
        left: i32,
        top: i32,
        width: i32,
        mask: &[u8],
        split: i32,
        first: [u8; 3],
        second: [u8; 3],
    ) {
        for (i, &coverage) in mask.iter().enumerate() {
            let i = i as i32;
            let (column, row) = (i % width, i / width);
            let color = if column < split { first } else { second };
            self.blend(left + column, top + row, color, coverage as f32 / 255.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn boxed(x: i32, y: i32) -> LabelBox {
        LabelBox { x, y, w: 20, h: 16 }
    }

    #[test]
    fn colors_parse() {
        assert_eq!(parse_color("#102030"), Some([16, 32, 48]));
        assert_eq!(parse_color("102030"), None);
        assert_eq!(parse_color("#1234"), None);
        assert_eq!(parse_color("#ééé"), None);
    }

    #[test]
    fn the_default_style_is_valid() {
        let mut errors = Vec::new();
        HintStyle::default().check(&mut errors);
        assert!(errors.is_empty(), "{errors:?}");
    }

    #[test]
    fn bad_styles_are_reported() {
        let cases: [fn(&mut HintStyle); 4] = [
            |s| s.background = "red".into(),
            |s| s.size = 9,
            |s| s.radius = 15,
            |s| s.opacity = 39,
        ];
        for change in cases {
            let mut style = HintStyle::default();
            change(&mut style);
            let mut errors = Vec::new();
            style.check(&mut errors);
            assert_eq!(errors.len(), 1, "{style:?}");
        }
    }

    #[test]
    fn separate_labels_stay_where_they_are() {
        let boxes = [boxed(10, 10), boxed(100, 10)];
        assert_eq!(place_labels(&boxes, 800, 600, 2), boxes);
    }

    #[test]
    fn overlapping_labels_are_moved_apart() {
        let placed = place_labels(&[boxed(10, 10), boxed(15, 12)], 800, 600, 2);
        assert_eq!(placed[0], boxed(10, 10));
        assert!(!placed[1].overlaps(&placed[0], 2));
    }

    #[test]
    fn many_labels_at_one_spot_do_not_overlap() {
        let placed = place_labels(&[boxed(50, 50); 12], 800, 600, 2);
        for (i, a) in placed.iter().enumerate() {
            assert!(placed[i + 1..].iter().all(|b| !a.overlaps(b, 2)));
        }
    }

    #[test]
    fn labels_stay_on_screen() {
        let placed = place_labels(&[boxed(795, 590), boxed(-30, -30)], 800, 600, 2);
        assert_eq!((placed[0].x, placed[0].y), (780, 584));
        assert_eq!((placed[1].x, placed[1].y), (0, 0));
    }

    #[test]
    fn a_full_row_wraps_to_the_next_line() {
        let placed = place_labels(&[boxed(760, 10), boxed(770, 12)], 800, 600, 2);
        assert!(placed[1].y > placed[0].y);
        assert!(placed[1].x + placed[1].w <= 800);
    }

    const WHITE: [u8; 3] = [255, 255, 255];

    fn alpha(pixels: &[u32], width: i32, x: i32, y: i32) -> u32 {
        pixels[(y * width + x) as usize] >> 24
    }

    #[test]
    fn a_pill_is_solid_inside_and_clear_outside() {
        let mut pixels = vec![0u32; 40 * 40];
        let b = LabelBox {
            x: 10,
            y: 10,
            w: 20,
            h: 20,
        };
        Canvas::new(&mut pixels, 40, 40).fill_rounded(b, 6.0, WHITE, 1.0);
        assert_eq!(alpha(&pixels, 40, 20, 20), 255);
        assert_eq!(alpha(&pixels, 40, 5, 5), 0);
    }

    #[test]
    fn rounded_corners_are_smooth_not_jagged() {
        let mut pixels = vec![0u32; 40 * 40];
        let b = LabelBox {
            x: 10,
            y: 10,
            w: 20,
            h: 20,
        };
        Canvas::new(&mut pixels, 40, 40).fill_rounded(b, 6.0, WHITE, 1.0);
        let partial = pixels
            .iter()
            .filter(|p| (1..255).contains(&(**p >> 24)))
            .count();
        assert!(partial > 8, "{partial}");
        assert_eq!(alpha(&pixels, 40, 10, 10), 0);
    }

    #[test]
    fn colors_are_premultiplied() {
        let mut pixels = vec![0u32; 4];
        let mut canvas = Canvas::new(&mut pixels, 2, 2);
        canvas.blend(0, 0, [200, 100, 0], 0.5);
        assert_eq!(pixels[0], 0x80_64_32_00);
    }

    #[test]
    fn opacity_makes_the_pill_translucent() {
        let mut pixels = vec![0u32; 40 * 40];
        let b = LabelBox {
            x: 5,
            y: 5,
            w: 30,
            h: 30,
        };
        Canvas::new(&mut pixels, 40, 40).fill_rounded(b, 4.0, WHITE, 0.5);
        assert_eq!(alpha(&pixels, 40, 20, 20), 128);
    }

    #[test]
    fn the_shadow_fades_with_distance() {
        let mut pixels = vec![0u32; 60 * 60];
        let b = LabelBox {
            x: 20,
            y: 20,
            w: 20,
            h: 20,
        };
        Canvas::new(&mut pixels, 60, 60).shadow(b, 4.0, 8.0, 0.8);
        let near = alpha(&pixels, 60, 18, 30);
        let far = alpha(&pixels, 60, 14, 30);
        assert!(near > far && far > 0, "{near} {far}");
    }

    #[test]
    fn drawing_past_the_edges_is_safe() {
        let mut pixels = vec![0u32; 100];
        let b = LabelBox {
            x: -5,
            y: 8,
            w: 30,
            h: 30,
        };
        let mut canvas = Canvas::new(&mut pixels, 10, 10);
        canvas.fill_rounded(b, 4.0, WHITE, 1.0);
        canvas.shadow(b, 4.0, 6.0, 0.5);
        canvas.draw_mask(8, 8, 4, &[255; 16], 2, WHITE, WHITE);
    }

    #[test]
    fn typed_letters_use_their_own_color() {
        let mut pixels = vec![0u32; 4];
        let red = [255, 0, 0];
        let blue = [0, 0, 255];
        Canvas::new(&mut pixels, 4, 1).draw_mask(0, 0, 4, &[255; 4], 2, red, blue);
        assert_eq!(pixels[1], 0xFF_FF_00_00);
        assert_eq!(pixels[2], 0xFF_00_00_FF);
    }
}
