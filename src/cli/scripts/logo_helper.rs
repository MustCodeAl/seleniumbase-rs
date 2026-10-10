//! The `sbase` banner.
//!
//! Python SeleniumBase prints its logo above the command list. The same
//! banner is shown at the top of `sbase --help`, and [`print_logo`] writes it
//! for any other command line tool that wants it.
//!
//! # Examples
//!
//! ```
//! use seleniumbase_rs::cli::scripts::logo_helper::{logo, LOGO};
//!
//! assert_eq!(logo(false), LOGO);
//! assert!(logo(true).starts_with("\x1b["));
//! assert_eq!(LOGO.lines().count(), 5);
//! ```

use super::rich_helper::{format_highlight, use_color, Stream};

/// The banner: "SeleniumBase" in ASCII art, five lines of at most 78 columns.
///
/// It has no trailing newline.
pub const LOGO: &str = r" ____          _                _                    ____
/ ___|   ___  | |  ___   _ __  (_) _   _  _ __ ___  | __ )   __ _   ___   ___
\___ \  / _ \ | | / _ \ | '_ \ | || | | || '_ ` _ \ |  _ \  / _` | / __| / _ \
 ___) ||  __/ | ||  __/ | | | || || |_| || | | | | || |_) || (_| | \__ \|  __/
|____/  \___| |_| \___| |_| |_||_| \__,_||_| |_| |_||____/  \__,_| |___/ \___|";

/// The banner as a string, in bold green when `color` is set.
#[must_use]
pub fn logo(color: bool) -> String {
    format_highlight(LOGO, color)
}

/// Prints the banner to standard output, in colour when that is a terminal.
pub fn print_logo() {
    println!("{}", logo(use_color(Stream::Stdout)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_logo_fits_an_80_column_terminal() {
        for line in LOGO.lines() {
            assert!(line.chars().count() <= 78, "{line:?}");
        }
    }

    #[test]
    fn the_logo_is_five_lines_without_a_trailing_newline() {
        assert_eq!(LOGO.lines().count(), 5);
        assert!(!LOGO.ends_with('\n'));
    }

    #[test]
    fn the_plain_logo_is_the_constant() {
        assert_eq!(logo(false), LOGO);
    }

    #[test]
    fn the_coloured_logo_is_the_constant_between_style_codes() {
        let coloured = logo(true);
        assert!(coloured.starts_with("\x1b[1;32m"), "{coloured:?}");
        assert!(coloured.ends_with("\x1b[0m"), "{coloured:?}");
        assert!(coloured.contains(LOGO));
    }
}
