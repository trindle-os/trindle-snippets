//! Snippets's own sizes, from its style sheet (share/snippets.css, built in) with

use toolkit::common::sheet::{read_sheet, own_sheet, Layers};

pub const CSS: &str = include_str!("../../share/snippets.css");

const OWN_MOST: u64 = 256 * 1024;

#[derive(Clone, Debug, PartialEq)]
pub struct Sizes {
    pub window: (u32, u32),
    pub list_width: f32,
    pub margin: f32,
    pub between: f32,
    pub inside: f32,
    pub form_gap: f32,
    pub buttons_gap: f32,
    pub replacement_least: f32,
    pub replacement_size: f32,
    pub preview_letters: usize,
}

impl Sizes {

    pub fn read() -> Sizes {
        let own = read_sheet(&own_sheet(), OWN_MOST).unwrap_or_default();
        Sizes::from(&Layers::new(CSS, &own))
    }

    pub fn from(l: &Layers) -> Sizes {

        let n = |name: &str, min: f32, max: f32| {
            l.number(name, false, min, max).or_else(|| Layers::new(CSS, "").number(name, false, min, max)).unwrap_or(min)
        };
        Sizes {
            window: (n("snippets-window-width", 480.0, 4000.0) as u32, n("snippets-window-height", 360.0, 4000.0) as u32),
            list_width: n("snippets-list-width", 200.0, 900.0),
            margin: n("snippets-margin", 0.0, 64.0),
            between: n("snippets-between", 4.0, 64.0),
            inside: n("snippets-inside", 4.0, 64.0),
            form_gap: n("snippets-form-gap", 2.0, 32.0),
            buttons_gap: n("snippets-buttons-gap", 4.0, 32.0),
            replacement_least: n("snippets-replacement-least", 60.0, 1000.0),
            replacement_size: n("snippets-replacement-size", 8.0, 48.0),
            preview_letters: n("snippets-preview-letters", 20.0, 400.0) as usize,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_size_comes_from_the_sheet_and_stays_within_its_limits() {
        let own = Sizes::from(&Layers::new(CSS, ""));
        assert_eq!(own.list_width, 340.0);
        assert_eq!(own.window, (1000, 680));
        let wild = Sizes::from(&Layers::new(CSS, ":root { --snippets-list-width: 99999; --snippets-margin: -5; --snippets-inside: words; }"));
        assert_eq!(wild.list_width, 900.0);
        assert_eq!(wild.margin, 0.0);
        assert_eq!(wild.inside, own.inside);
        for d in toolkit::common::sheet::declarations(CSS) {
            let moved = ["7", "700"].iter().any(|v| Sizes::from(&Layers::new(CSS, &format!(":root {{ --{}: {v}; }}", d.name))) != own);
            assert!(moved, "{} is in the sheet but changing it changes nothing", d.name);
        }
    }

    #[test]
    fn the_sheet_sets_no_colour_and_every_pair_painted_with_it_is_readable() {
        for d in toolkit::common::sheet::declarations(CSS) {
            assert!(d.name.starts_with("snippets-"), "{} is not Snippets's own", d.name);
        }
        let layers = Layers::new(CSS, "");
        for light in [false, true] {
            let short: Vec<_> = trindle_toolkit::theme::painted(&layers, light, "#3584e4").into_iter().filter(|p| p.short()).collect();
            assert!(short.is_empty(), "light={light}: {:?}", short.iter().map(|p| (p.ink, p.on.clone())).collect::<Vec<_>>());
        }
    }
}
