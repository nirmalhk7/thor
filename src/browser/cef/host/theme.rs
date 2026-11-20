use ::cef::*;

pub(super) const BG: u32 = 0xff151a20;
pub(super) const INSET: u32 = 0xff101419;
pub(super) const TEXT: u32 = 0xffedf1f5;
pub(super) const MUTED: u32 = 0xffa6b0bd;
pub(super) const ACCENT: u32 = 0xff90b8f8;
pub(super) const CHROME: u32 = 0xff000000;
pub(super) const FIELD: u32 = 0xff393939;
pub(super) const DRAWER: u32 = 0xff262626;
pub(super) const SCRIM: u32 = 0x80262626;
pub(super) const TOOLBAR_HEIGHT: i32 = 35;
pub(super) const FIELD_HEIGHT: i32 = 22;
pub(super) const DRAWER_WIDTH: i32 = 281;
pub(super) const TRAFFIC_LIGHT_SPACE: i32 = 82;

pub(super) fn drawer_bounds(content: Rect) -> (Rect, Rect) {
    let drawer_width = content.width.clamp(0, DRAWER_WIDTH);
    let scrim_width = content.width.max(0) - drawer_width;
    (
        Rect {
            width: scrim_width,
            ..content
        },
        Rect {
            x: content.x + scrim_width,
            width: drawer_width,
            ..content
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drawer_and_scrim_cover_only_content_and_share_an_edge() {
        let content = Rect {
            x: 0,
            y: 35,
            width: 712,
            height: 380,
        };
        let (scrim, drawer) = drawer_bounds(content);
        assert_eq!(
            (scrim.x, scrim.y, scrim.width, scrim.height),
            (0, 35, 431, 380)
        );
        assert_eq!(
            (drawer.x, drawer.y, drawer.width, drawer.height),
            (431, 35, 281, 380)
        );
    }

    #[test]
    fn drawer_stays_within_the_content_when_space_is_limited() {
        let (scrim, drawer) = drawer_bounds(Rect {
            x: 12,
            y: 60,
            width: 180,
            height: 250,
        });
        assert_eq!(scrim.width, 0);
        assert_eq!(
            (drawer.x, drawer.y, drawer.width, drawer.height),
            (12, 60, 180, 250)
        );
    }
}

pub(super) fn apply(window: &Window) {
    for (id, color) in [
        (ColorId::COLOR_PRIMARY_BACKGROUND, BG),
        (ColorId::COLOR_WINDOW_BACKGROUND, BG),
        (ColorId::COLOR_TEXTFIELD_BACKGROUND, FIELD),
        (ColorId::COLOR_TEXTFIELD_FOREGROUND, TEXT),
        (ColorId::COLOR_TEXTFIELD_FOREGROUND_PLACEHOLDER, MUTED),
        (ColorId::COLOR_BUTTON_FOREGROUND, TEXT),
        (ColorId::COLOR_LABEL_FOREGROUND, TEXT),
        (ColorId::COLOR_FOCUSABLE_BORDER_FOCUSED, ACCENT),
    ] {
        window.set_theme_color(id.get_raw() as i32, color);
    }
    window.theme_changed();
}

#[derive(Clone, Copy)]
pub(super) enum Icon {
    Back,
    Forward,
    Reload,
    Home,
}

pub(super) fn icon(kind: Icon, color: u32) -> Option<Image> {
    let lines: &[(f32, f32, f32, f32)] = match kind {
        Icon::Back => &[(14., 9., 4., 9.), (4., 9., 9., 4.), (4., 9., 9., 14.)],
        Icon::Forward => &[(4., 9., 14., 9.), (14., 9., 9., 4.), (14., 9., 9., 14.)],
        Icon::Reload => &[
            (14., 7., 11., 4.),
            (11., 4., 6., 4.),
            (6., 4., 3., 8.),
            (3., 8., 4., 12.),
            (4., 12., 8., 15.),
            (8., 15., 12., 14.),
            (12., 14., 14., 11.),
            (14., 3., 14., 7.),
            (10., 7., 14., 7.),
        ],
        Icon::Home => &[
            (3., 9., 9., 3.),
            (9., 3., 15., 9.),
            (5., 8., 5., 15.),
            (5., 15., 13., 15.),
            (13., 15., 13., 8.),
        ],
    };
    let image = image_create()?;
    for scale in [1, 2] {
        let side = 18 * scale;
        let mut pixels = vec![0; side * side * 4];
        for y in 0..side {
            for x in 0..side {
                let (px, py) = (
                    (x as f32 + 0.5) / scale as f32,
                    (y as f32 + 0.5) / scale as f32,
                );
                let distance = lines
                    .iter()
                    .map(|&(x1, y1, x2, y2)| {
                        let (dx, dy) = (x2 - x1, y2 - y1);
                        let t =
                            (((px - x1) * dx + (py - y1) * dy) / (dx * dx + dy * dy)).clamp(0., 1.);
                        ((px - x1 - t * dx).powi(2) + (py - y1 - t * dy).powi(2)).sqrt()
                    })
                    .fold(f32::MAX, f32::min);
                let alpha = ((1.2 - distance) * scale as f32).clamp(0., 1.);
                let offset = (y * side + x) * 4;
                for (channel, shift) in [16, 8, 0].into_iter().enumerate() {
                    pixels[offset + channel] = (((color >> shift) & 255) as f32 * alpha) as u8;
                }
                pixels[offset + 3] = (255. * alpha) as u8;
            }
        }
        image.add_bitmap(
            scale as f32,
            side as i32,
            side as i32,
            ColorType::RGBA_8888,
            AlphaType::PREMULTIPLIED,
            Some(&pixels),
        );
    }
    Some(image)
}
