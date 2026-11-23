//! Color helper matching Python Selenium's `selenium.webdriver.support.color.Color`.

use crate::error::SeleniumBaseError;

/// Color representation matching Python Selenium's `selenium.webdriver.support.color.Color`.
///
/// Supports parsing from hex strings (`#rgb`, `#rrggbb`, `#rgba`, `#rrggbbaa`),
/// CSS functional notations (`rgb(r, g, b)`, `rgba(r, g, b, a)`), and standard CSS color names.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::Color;
///
/// let color = Color::from_string("#ff0000").unwrap();
/// assert_eq!(color.hex(), "#ff0000");
/// assert_eq!(color.rgb(), "rgb(255, 0, 0)");
/// assert_eq!(color.rgba(), "rgba(255, 0, 0, 1)");
///
/// let from_named = Color::from_string("blue").unwrap();
/// assert_eq!(from_named.hex(), "#0000ff");
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    red: u8,
    green: u8,
    blue: u8,
    alpha: f64,
}

impl Color {
    /// Creates a new Color with `red`, `green`, `blue` (0..=255) and `alpha` (0.0..=1.0).
    #[must_use]
    pub fn new(red: u8, green: u8, blue: u8, alpha: f64) -> Self {
        Self {
            red,
            green,
            blue,
            alpha: alpha.clamp(0.0, 1.0),
        }
    }

    /// Red component (0..=255).
    #[must_use]
    pub const fn red(&self) -> u8 {
        self.red
    }

    /// Green component (0..=255).
    #[must_use]
    pub const fn green(&self) -> u8 {
        self.green
    }

    /// Blue component (0..=255).
    #[must_use]
    pub const fn blue(&self) -> u8 {
        self.blue
    }

    /// Alpha component (0.0..=1.0).
    #[must_use]
    pub const fn alpha(&self) -> f64 {
        self.alpha
    }

    /// Parses a color string (hex, rgb, rgba, or CSS color name).
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Parse`] if the string cannot be parsed as a color.
    pub fn from_string(value: &str) -> Result<Self, SeleniumBaseError> {
        let trimmed = value.trim().to_ascii_lowercase();

        if let Some(named) = Self::from_named_color(&trimmed) {
            return Ok(named);
        }

        if trimmed.starts_with('#') {
            return Self::from_hex(&trimmed);
        }

        if trimmed.starts_with("rgb(") || trimmed.starts_with("rgba(") {
            return Self::from_rgb_or_rgba(&trimmed);
        }

        Err(SeleniumBaseError::parse(
            value,
            "hex string (#rgb, #rrggbb), rgb/rgba string, or CSS color name",
        ))
    }

    /// Formats as lower-case 6-digit hex string `#rrggbb`.
    #[must_use]
    pub fn hex(&self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.red, self.green, self.blue)
    }

    /// Formats as `rgb(r, g, b)`.
    #[must_use]
    pub fn rgb(&self) -> String {
        format!("rgb({}, {}, {})", self.red, self.green, self.blue)
    }

    /// Formats as `rgba(r, g, b, a)`.
    #[must_use]
    pub fn rgba(&self) -> String {
        let alpha_str = if (self.alpha - 1.0).abs() < f64::EPSILON {
            "1".to_string()
        } else if self.alpha.abs() < f64::EPSILON {
            "0".to_string()
        } else {
            format!("{:.2}", self.alpha)
                .trim_end_matches('0')
                .trim_end_matches('.')
                .to_owned()
        };
        format!(
            "rgba({}, {}, {}, {})",
            self.red, self.green, self.blue, alpha_str
        )
    }

    fn from_hex(hex: &str) -> Result<Self, SeleniumBaseError> {
        let s = hex.trim_start_matches('#');
        match s.len() {
            3 => {
                let r = u8::from_str_radix(&s[0..1].repeat(2), 16)
                    .map_err(|e| SeleniumBaseError::parse(hex, e.to_string()))?;
                let g = u8::from_str_radix(&s[1..2].repeat(2), 16)
                    .map_err(|e| SeleniumBaseError::parse(hex, e.to_string()))?;
                let b = u8::from_str_radix(&s[2..3].repeat(2), 16)
                    .map_err(|e| SeleniumBaseError::parse(hex, e.to_string()))?;
                Ok(Self::new(r, g, b, 1.0))
            }
            4 => {
                let r = u8::from_str_radix(&s[0..1].repeat(2), 16)
                    .map_err(|e| SeleniumBaseError::parse(hex, e.to_string()))?;
                let g = u8::from_str_radix(&s[1..2].repeat(2), 16)
                    .map_err(|e| SeleniumBaseError::parse(hex, e.to_string()))?;
                let b = u8::from_str_radix(&s[2..3].repeat(2), 16)
                    .map_err(|e| SeleniumBaseError::parse(hex, e.to_string()))?;
                let a = u8::from_str_radix(&s[3..4].repeat(2), 16)
                    .map_err(|e| SeleniumBaseError::parse(hex, e.to_string()))?;
                Ok(Self::new(r, g, b, f64::from(a) / 255.0))
            }
            6 => {
                let r = u8::from_str_radix(&s[0..2], 16)
                    .map_err(|e| SeleniumBaseError::parse(hex, e.to_string()))?;
                let g = u8::from_str_radix(&s[2..4], 16)
                    .map_err(|e| SeleniumBaseError::parse(hex, e.to_string()))?;
                let b = u8::from_str_radix(&s[4..6], 16)
                    .map_err(|e| SeleniumBaseError::parse(hex, e.to_string()))?;
                Ok(Self::new(r, g, b, 1.0))
            }
            8 => {
                let r = u8::from_str_radix(&s[0..2], 16)
                    .map_err(|e| SeleniumBaseError::parse(hex, e.to_string()))?;
                let g = u8::from_str_radix(&s[2..4], 16)
                    .map_err(|e| SeleniumBaseError::parse(hex, e.to_string()))?;
                let b = u8::from_str_radix(&s[4..6], 16)
                    .map_err(|e| SeleniumBaseError::parse(hex, e.to_string()))?;
                let a = u8::from_str_radix(&s[6..8], 16)
                    .map_err(|e| SeleniumBaseError::parse(hex, e.to_string()))?;
                Ok(Self::new(r, g, b, f64::from(a) / 255.0))
            }
            _ => Err(SeleniumBaseError::parse(hex, "3, 4, 6, or 8 hex digits")),
        }
    }

    fn from_rgb_or_rgba(s: &str) -> Result<Self, SeleniumBaseError> {
        let inside = s
            .trim_start_matches("rgba(")
            .trim_start_matches("rgb(")
            .trim_end_matches(')')
            .trim();
        let parts: Vec<&str> = inside.split(',').map(str::trim).collect();
        if parts.len() == 3 {
            let r: u8 = parts[0].parse().map_err(|e: std::num::ParseIntError| {
                SeleniumBaseError::parse(parts[0], e.to_string())
            })?;
            let g: u8 = parts[1].parse().map_err(|e: std::num::ParseIntError| {
                SeleniumBaseError::parse(parts[1], e.to_string())
            })?;
            let b: u8 = parts[2].parse().map_err(|e: std::num::ParseIntError| {
                SeleniumBaseError::parse(parts[2], e.to_string())
            })?;
            Ok(Self::new(r, g, b, 1.0))
        } else if parts.len() == 4 {
            let r: u8 = parts[0].parse().map_err(|e: std::num::ParseIntError| {
                SeleniumBaseError::parse(parts[0], e.to_string())
            })?;
            let g: u8 = parts[1].parse().map_err(|e: std::num::ParseIntError| {
                SeleniumBaseError::parse(parts[1], e.to_string())
            })?;
            let b: u8 = parts[2].parse().map_err(|e: std::num::ParseIntError| {
                SeleniumBaseError::parse(parts[2], e.to_string())
            })?;
            let a: f64 = parts[3].parse().map_err(|e: std::num::ParseFloatError| {
                SeleniumBaseError::parse(parts[3], e.to_string())
            })?;
            Ok(Self::new(r, g, b, a))
        } else {
            Err(SeleniumBaseError::parse(
                s,
                "rgb(r, g, b) or rgba(r, g, b, a)",
            ))
        }
    }

    fn from_named_color(name: &str) -> Option<Self> {
        Some(match name {
            "transparent" => Self::new(0, 0, 0, 0.0),
            "black" => Self::new(0, 0, 0, 1.0),
            "white" => Self::new(255, 255, 255, 1.0),
            "red" => Self::new(255, 0, 0, 1.0),
            "green" => Self::new(0, 128, 0, 1.0),
            "blue" => Self::new(0, 0, 255, 1.0),
            "yellow" => Self::new(255, 255, 0, 1.0),
            "cyan" | "aqua" => Self::new(0, 255, 255, 1.0),
            "magenta" | "fuchsia" => Self::new(255, 0, 255, 1.0),
            "gray" | "grey" => Self::new(128, 128, 128, 1.0),
            "silver" => Self::new(192, 192, 192, 1.0),
            "maroon" => Self::new(128, 0, 0, 1.0),
            "olive" => Self::new(128, 128, 0, 1.0),
            "lime" => Self::new(0, 255, 0, 1.0),
            "purple" => Self::new(128, 0, 128, 1.0),
            "teal" => Self::new(0, 128, 128, 1.0),
            "navy" => Self::new(0, 0, 128, 1.0),
            "orange" => Self::new(255, 165, 0, 1.0),
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hex_and_formats() {
        let c = Color::from_string("#f00").unwrap();
        assert_eq!(c.hex(), "#ff0000");
        assert_eq!(c.rgb(), "rgb(255, 0, 0)");
        assert_eq!(c.rgba(), "rgba(255, 0, 0, 1)");

        let c = Color::from_string("#00ff00").unwrap();
        assert_eq!(c.hex(), "#00ff00");
        assert_eq!(c.rgb(), "rgb(0, 255, 0)");
    }

    #[test]
    fn parses_rgb_rgba() {
        let c = Color::from_string("rgb(255, 255, 255)").unwrap();
        assert_eq!(c.hex(), "#ffffff");

        let c = Color::from_string("rgba(0, 0, 255, 0.5)").unwrap();
        assert_eq!(c.hex(), "#0000ff");
        assert_eq!(c.rgba(), "rgba(0, 0, 255, 0.5)");
    }

    #[test]
    fn parses_named_colors() {
        assert_eq!(Color::from_string("red").unwrap().hex(), "#ff0000");
        assert_eq!(Color::from_string("blue").unwrap().hex(), "#0000ff");
        assert_eq!(Color::from_string("transparent").unwrap().alpha(), 0.0);
    }
}
