use super::FrontendEngine;

impl FrontendEngine {
    /// WCAG relative-luminance contrast ratio between two `#rrggbb` colors.
    pub fn contrast_ratio(fg: &str, bg: &str) -> Option<f64> {
        let l1 = Self::relative_luminance(fg)?;
        let l2 = Self::relative_luminance(bg)?;
        let (light, dark) = if l1 >= l2 { (l1, l2) } else { (l2, l1) };
        Some((light + 0.05) / (dark + 0.05))
    }

    fn relative_luminance(hex: &str) -> Option<f64> {
        let h = hex.trim().trim_start_matches('#');
        if h.len() != 6 {
            return None;
        }
        let chan = |s: &str| u8::from_str_radix(s, 16).ok().map(|v| v as f64 / 255.0);
        let r = chan(&h[0..2])?;
        let g = chan(&h[2..4])?;
        let b = chan(&h[4..6])?;
        let lin = |c: f64| {
            if c <= 0.03928 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        Some(0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b))
    }
}
