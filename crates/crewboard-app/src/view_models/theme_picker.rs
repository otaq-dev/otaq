//! The theme picker shared by onboarding and Settings → Appearance (UI-11, d0069; *14 · First run & onboarding →
//! 2 · Theme*, *15 · Settings → Global · Usage, notifications, data, appearance*): the mode switch, then a Dark
//! theme and a Light theme section of cards. Every click is a new [`ThemeChoice`], applied and saved at once.

use gpui::SharedString;

use crate::theme::{Appearance, Mode, ThemeChoice, ThemeId};

/// The mode switch's options, in the frames' order.
pub const MODES: [(Mode, &str); 3] = [
    (Mode::Light, "Light"),
    (Mode::Dark, "Dark"),
    (Mode::System, "System"),
];

/// The option of the mode switch that is on.
pub fn mode_index(mode: Mode) -> usize {
    MODES.iter().position(|(m, _)| *m == mode).unwrap_or(2)
}

/// One theme card: the preview is drawn from `id`'s own colours.
#[derive(Debug, Clone, PartialEq)]
pub struct ThemeCard {
    pub id: ThemeId,
    pub name: SharedString,
    pub tagline: &'static str,
    pub selected: bool,
}

/// The Dark theme or the Light theme section.
#[derive(Debug, Clone, PartialEq)]
pub struct PickerSection {
    pub kind: Appearance,
    pub label: &'static str,
    pub description: &'static str,
    /// "In use · OS is light", in System mode on the section the OS appearance selects.
    pub in_use: Option<&'static str>,
    /// Rows of five cards.
    pub rows: Vec<Vec<ThemeCard>>,
}

/// Cards per row in both frames.
pub const PER_ROW: usize = 5;

/// The one-line tagline under a theme's name, as in the frames.
pub fn tagline(id: ThemeId) -> &'static str {
    match id {
        ThemeId::CrewboardDark => "Default · graphite",
        ThemeId::CatppuccinFrappe => "Muted, soft pastels",
        ThemeId::CatppuccinMacchiato => "Medium-contrast pastels",
        ThemeId::CatppuccinMocha => "Deep, cozy pastels",
        ThemeId::TokyoNight => "Neon city after dark",
        ThemeId::Dracula => "Bold, high contrast",
        ThemeId::Nord => "Arctic, calm blues",
        ThemeId::GruvboxDark => "Retro, warm earth",
        ThemeId::RosePine => "Soho vibes, muted",
        ThemeId::OneDark => "Atom’s classic",
        ThemeId::CrewboardLight => "Default · clean paper",
        ThemeId::CatppuccinLatte => "Pastels in daylight",
        ThemeId::Daylight => "Familiar and crisp",
        ThemeId::SolarizedLight => "Low-glare cream",
        ThemeId::RosePineDawn => "Warm morning blush",
    }
}

/// Both sections for a choice, given the OS appearance (for the "In use" tag).
pub fn sections(choice: ThemeChoice, os: Appearance) -> [PickerSection; 2] {
    let selected = [ThemeId::dark(choice.dark), ThemeId::light(choice.light)];
    let section = |kind: Appearance, label, description, themes: &[ThemeId]| {
        let in_use = (choice.mode == Mode::System && os == kind).then_some(match kind {
            Appearance::Dark => "In use · OS is dark",
            Appearance::Light => "In use · OS is light",
        });
        let cards: Vec<ThemeCard> = themes
            .iter()
            .map(|&id| ThemeCard {
                id,
                name: id.name(),
                tagline: tagline(id),
                selected: selected.contains(&id),
            })
            .collect();
        PickerSection {
            kind,
            label,
            description,
            in_use,
            rows: cards.chunks(PER_ROW).map(<[ThemeCard]>::to_vec).collect(),
        }
    };
    [
        section(
            Appearance::Dark,
            "Dark theme",
            "Used in Dark mode, and in System mode when the OS is dark. Recolors the board, panels and the project terminal.",
            &ThemeId::DARK,
        ),
        section(
            Appearance::Light,
            "Light theme",
            "Used in Light mode, and in System mode when the OS is light.",
            &ThemeId::LIGHT,
        ),
    ]
}

/// The choice after a click on a card: that theme becomes the dark or the light one; the mode and the other
/// theme stay.
pub fn pick_theme(choice: ThemeChoice, id: ThemeId) -> ThemeChoice {
    choice.with_theme(id)
}

/// The choice after a click on the mode switch.
pub fn pick_mode(choice: ThemeChoice, index: usize) -> ThemeChoice {
    ThemeChoice {
        mode: MODES.get(index).map_or(choice.mode, |(mode, _)| *mode),
        ..choice
    }
}

#[cfg(test)]
mod tests {
    use crewboard_core::board::settings::{DarkTheme, LightTheme};

    use super::*;

    fn selected(section: &PickerSection) -> Vec<ThemeId> {
        section
            .rows
            .iter()
            .flatten()
            .filter(|c| c.selected)
            .map(|c| c.id)
            .collect()
    }

    #[test]
    fn two_sections_as_in_the_frames() {
        let [dark, light] = sections(ThemeChoice::default(), Appearance::Light);
        assert_eq!(dark.label, "Dark theme");
        assert_eq!(
            dark.rows.iter().map(Vec::len).collect::<Vec<_>>(),
            [5, 5],
            "two rows of five"
        );
        assert_eq!(light.rows.iter().map(Vec::len).collect::<Vec<_>>(), [5]);
        assert_eq!(dark.rows[0][4].name, "Tokyo Night");
        assert_eq!(dark.rows[0][4].tagline, "Neon city after dark");
        assert_eq!(light.rows[0][3].name, "Solarized Light");
        assert_eq!(selected(&dark), [ThemeId::CrewboardDark]);
        assert_eq!(selected(&light), [ThemeId::CrewboardLight]);
    }

    #[test]
    fn in_use_only_in_system_mode_on_the_os_side() {
        for (mode, os, dark_tag, light_tag) in [
            (
                Mode::System,
                Appearance::Light,
                None,
                Some("In use · OS is light"),
            ),
            (
                Mode::System,
                Appearance::Dark,
                Some("In use · OS is dark"),
                None,
            ),
            (Mode::Light, Appearance::Light, None, None),
            (Mode::Light, Appearance::Dark, None, None),
            (Mode::Dark, Appearance::Light, None, None),
            (Mode::Dark, Appearance::Dark, None, None),
        ] {
            let choice = ThemeChoice {
                mode,
                ..ThemeChoice::default()
            };
            let [dark, light] = sections(choice, os);
            assert_eq!(
                (dark.in_use, light.in_use),
                (dark_tag, light_tag),
                "{mode:?} {os:?}"
            );
        }
    }

    #[test]
    fn a_click_stores_one_side_and_keeps_the_other() {
        let choice = pick_theme(ThemeChoice::default(), ThemeId::TokyoNight);
        assert_eq!(choice.dark, DarkTheme::TokyoNight);
        assert_eq!(choice.light, LightTheme::Crewboard);
        let choice = pick_theme(choice, ThemeId::Daylight);
        assert_eq!(
            (choice.mode, choice.dark, choice.light),
            (Mode::System, DarkTheme::TokyoNight, LightTheme::Daylight)
        );
        let [dark, light] = sections(choice, Appearance::Dark);
        assert_eq!(selected(&dark), [ThemeId::TokyoNight]);
        assert_eq!(selected(&light), [ThemeId::Daylight]);

        let dark_mode = pick_mode(choice, mode_index(Mode::Dark));
        assert_eq!(dark_mode.mode, Mode::Dark);
        assert_eq!(
            (dark_mode.dark, dark_mode.light),
            (choice.dark, choice.light)
        );
        assert_eq!(MODES[mode_index(Mode::System)].1, "System");
    }
}
