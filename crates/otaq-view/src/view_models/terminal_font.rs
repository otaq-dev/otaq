//! The terminal font (d0117 point 5, T-0482): which installed families the Settings → Appearance dropdown lists, and
//! the font stack every terminal draws with: the chosen font (Geist Mono by default), then the installed Nerd Fonts
//! for the prompt icons the chosen font lacks, then `monospace`. Otaq ships no Nerd Font; the desktop reads the
//! installed families from the OS font folders once per start and hands them here as (family, monospaced) pairs.

use otaq_core::board::settings::{DEFAULT_TERMINAL_FONT_FAMILY, GlobalSettings};

use super::settings_page::Choice;

/// The installed font families the terminal font is chosen from.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InstalledFonts {
    /// Monospace families, sorted by name, Geist Mono left out (it is the default entry).
    pub monospace: Vec<String>,
    /// Nerd Font families for missing symbols, sorted by name: one per family, its Mono variant where installed
    /// (icons one cell wide), never a proportional ("Propo") one.
    pub nerd: Vec<String>,
}

impl InstalledFonts {
    /// Sorts the installed families: `faces` is every face's family and whether it is monospaced (several faces of
    /// one family are fine). Hidden system families (a leading `.`) are left out.
    pub fn from_faces<'a>(faces: impl IntoIterator<Item = (&'a str, bool)>) -> Self {
        let mut monospace: Vec<String> = Vec::new();
        let mut nerd: Vec<String> = Vec::new();
        for (family, monospaced) in faces {
            let family = family.trim();
            if family.is_empty() || family.starts_with('.') {
                continue;
            }
            if monospaced && !family.eq_ignore_ascii_case(DEFAULT_TERMINAL_FONT_FAMILY) {
                push_new(&mut monospace, family);
            }
            if nerd_kind(family).is_some_and(|kind| kind != NerdKind::Proportional) {
                push_new(&mut nerd, family);
            }
        }
        // A family's Mono variant replaces the plain one ("3270 Nerd Font Mono" over "3270 Nerd Font").
        let plain_with_mono: Vec<String> = nerd
            .iter()
            .filter(|f| nerd_kind(f) == Some(NerdKind::Plain))
            .filter(|f| {
                nerd.iter()
                    .any(|g| mono_of(g).is_some_and(|base| base == **f))
            })
            .cloned()
            .collect();
        nerd.retain(|f| !plain_with_mono.contains(f));
        let by_name = |a: &String, b: &String| a.to_lowercase().cmp(&b.to_lowercase());
        monospace.sort_by(by_name);
        nerd.sort_by(by_name);
        Self { monospace, nerd }
    }
}

fn push_new(list: &mut Vec<String>, family: &str) {
    if !list.iter().any(|f| f == family) {
        list.push(family.to_owned());
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NerdKind {
    Plain,
    Mono,
    Proportional,
}

/// Whether `family` is a Nerd Font, and which variant: `… Nerd Font`, `… Nerd Font Mono`, `… Nerd Font Propo`, and
/// the short names `… NF`, `… NFM`, `… NFP` (MesloLGS NF, JetBrainsMono NFM).
fn nerd_kind(family: &str) -> Option<NerdKind> {
    if family.ends_with(" NF") {
        Some(NerdKind::Plain)
    } else if family.ends_with(" NFM") {
        Some(NerdKind::Mono)
    } else if family.ends_with(" NFP") {
        Some(NerdKind::Proportional)
    } else if !family.contains("Nerd Font") {
        None
    } else if family.ends_with(" Mono") {
        Some(NerdKind::Mono)
    } else if family.contains("Propo") {
        Some(NerdKind::Proportional)
    } else {
        Some(NerdKind::Plain)
    }
}

/// The plain family a Mono variant belongs to: "3270 Nerd Font Mono" → "3270 Nerd Font", "Hack NFM" → "Hack NF".
fn mono_of(family: &str) -> Option<String> {
    match nerd_kind(family)? {
        NerdKind::Mono => family
            .strip_suffix(" Mono")
            .map(str::to_owned)
            .or_else(|| family.strip_suffix('M').map(str::to_owned)),
        _ => None,
    }
}

/// The dropdown's first entry, which stores no font.
pub const DEFAULT_CHOICE_LABEL: &str = "Geist Mono (default)";

/// The Terminal font dropdown: "Geist Mono (default)" (id `""`), then every installed monospace family.
pub fn choices(fonts: &InstalledFonts) -> Vec<Choice> {
    std::iter::once(Choice {
        id: String::new(),
        label: DEFAULT_CHOICE_LABEL.to_owned(),
    })
    .chain(fonts.monospace.iter().map(|family| Choice {
        id: family.clone(),
        label: family.clone(),
    }))
    .collect()
}

/// The dropdown's value: the chosen family while it is installed, else `""` (Geist Mono).
pub fn chosen(settings: &GlobalSettings, fonts: &InstalledFonts) -> String {
    settings
        .terminal_font_family_in(&fonts.monospace)
        .unwrap_or_default()
        .to_owned()
}

/// The terminal's CSS font stack: the chosen font (Geist Mono when none is, or it is no longer installed), each
/// installed Nerd Font for the symbols it lacks, then `monospace`.
pub fn stack(settings: &GlobalSettings, fonts: &InstalledFonts) -> String {
    let first = settings
        .terminal_font_family_in(&fonts.monospace)
        .unwrap_or(DEFAULT_TERMINAL_FONT_FAMILY);
    std::iter::once(first)
        .chain(
            fonts
                .nerd
                .iter()
                .map(String::as_str)
                .filter(|f| *f != first),
        )
        .map(quoted)
        .chain(std::iter::once("monospace".to_owned()))
        .collect::<Vec<_>>()
        .join(", ")
}

/// A family name as a CSS string.
fn quoted(family: &str) -> String {
    format!("\"{}\"", family.replace('\\', "\\\\").replace('"', "\\\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fixture folder's faces: one monospace font, one proportional font, a Nerd Font in two variants.
    fn fixture() -> InstalledFonts {
        InstalledFonts::from_faces([
            ("Fixture Mono", true),
            ("Fixture Mono", true),
            ("Fixture Sans", false),
            ("Fixture Nerd Font", false),
            ("Fixture Nerd Font Mono", true),
            ("Fixture Nerd Font Propo", false),
            (".Hidden Mono", true),
            ("Geist Mono", true),
        ])
    }

    #[test]
    fn the_list_keeps_monospace_and_the_nerd_fonts_mono_variant() {
        let fonts = fixture();
        assert_eq!(fonts.monospace, ["Fixture Mono", "Fixture Nerd Font Mono"]);
        assert_eq!(fonts.nerd, ["Fixture Nerd Font Mono"]);
        // Short names, and a plain variant with no Mono one installed.
        let fonts = InstalledFonts::from_faces([
            ("MesloLGS NF", true),
            ("Hack NF", true),
            ("Hack NFM", true),
            ("Hack NFP", false),
            ("3270 Nerd Font", false),
        ]);
        assert_eq!(fonts.nerd, ["3270 Nerd Font", "Hack NFM", "MesloLGS NF"]);
    }

    #[test]
    fn the_dropdown_lists_geist_mono_first_then_the_monospace_fonts() {
        let fonts = fixture();
        let labels: Vec<_> = choices(&fonts)
            .into_iter()
            .map(|c| (c.id, c.label))
            .collect();
        assert_eq!(
            labels,
            [
                ("".to_owned(), "Geist Mono (default)".to_owned()),
                ("Fixture Mono".to_owned(), "Fixture Mono".to_owned()),
                (
                    "Fixture Nerd Font Mono".to_owned(),
                    "Fixture Nerd Font Mono".to_owned()
                ),
            ]
        );
        let mut settings = GlobalSettings::default();
        assert_eq!(chosen(&settings, &fonts), "");
        settings.terminal_font_family = Some("Fixture Mono".into());
        assert_eq!(chosen(&settings, &fonts), "Fixture Mono");
        settings.terminal_font_family = Some("Gone Mono".into());
        assert_eq!(
            chosen(&settings, &fonts),
            "",
            "uninstalled reads as Geist Mono"
        );
    }

    #[test]
    fn the_stack_puts_the_nerd_font_after_the_chosen_font() {
        let fonts = fixture();
        let mut settings = GlobalSettings::default();
        assert_eq!(
            stack(&settings, &fonts),
            r#""Geist Mono", "Fixture Nerd Font Mono", monospace"#
        );
        settings.terminal_font_family = Some("Fixture Mono".into());
        assert_eq!(
            stack(&settings, &fonts),
            r#""Fixture Mono", "Fixture Nerd Font Mono", monospace"#
        );
        // The Nerd Font chosen itself is not listed twice.
        settings.terminal_font_family = Some("Fixture Nerd Font Mono".into());
        assert_eq!(
            stack(&settings, &fonts),
            r#""Fixture Nerd Font Mono", monospace"#
        );
        // No Nerd Font installed: none is added; a font removed since falls back to Geist Mono.
        settings.terminal_font_family = Some("Gone Mono".into());
        assert_eq!(
            stack(&settings, &InstalledFonts::default()),
            r#""Geist Mono", monospace"#
        );
        assert_eq!(quoted(r#"Odd "Name"\"#), r#""Odd \"Name\"\\""#);
    }
}
