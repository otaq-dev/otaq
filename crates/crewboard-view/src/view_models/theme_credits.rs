//! The Theme credits dialog (UI-11, d0069, d0054; *15 · Settings → Theme credits · dialog*): each source project
//! of the bundled themes with the themes taken from it, its copyright line, its license and its source, then the
//! MIT text. This is the notice those licenses ask Otaq to ship. The texts are copied from the frame.

use crewboard_core::naming;

/// One source project.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Credit {
    pub project: &'static str,
    /// The bundled themes taken from it, as the frame lists them.
    pub themes: &'static str,
    pub copyright: &'static str,
    pub license: &'static str,
    /// The project's own repository (not in the design; an implementation choice, see review-M13).
    pub source: &'static str,
}

pub const CREDITS: [Credit; 9] = [
    Credit {
        project: "Catppuccin",
        themes: "Catppuccin Frappé, Macchiato, Mocha, Latte",
        copyright: "Copyright (c) 2021 Catppuccin",
        license: "MIT",
        source: "https://github.com/catppuccin/catppuccin",
    },
    Credit {
        project: "Tokyo Night",
        themes: "Tokyo Night",
        copyright: "Copyright (c) 2018-present Enkia",
        license: "MIT",
        source: "https://github.com/enkia/tokyo-night-vscode-theme",
    },
    Credit {
        project: "Dracula",
        themes: "Dracula",
        copyright: "Copyright (c) 2023 Dracula Theme",
        license: "MIT",
        source: "https://github.com/dracula/dracula-theme",
    },
    Credit {
        project: "Nord",
        themes: "Nord",
        copyright: "Copyright (c) 2016-present Sven Greb",
        license: "MIT",
        source: "https://github.com/nordtheme/nord",
    },
    Credit {
        project: "Gruvbox",
        themes: "Gruvbox Dark",
        copyright: "Copyright (c) Pavel Pertsev (morhetz)",
        license: "MIT/X11",
        source: "https://github.com/morhetz/gruvbox",
    },
    Credit {
        project: "Rosé Pine",
        themes: "Rosé Pine, Rosé Pine Dawn",
        copyright: "Copyright (c) mvllow",
        license: "MIT",
        source: "https://github.com/rose-pine/rose-pine-theme",
    },
    Credit {
        project: "Atom One Dark",
        themes: "One Dark",
        copyright: "Copyright (c) 2011-2022 GitHub Inc.",
        license: "MIT",
        source: "https://github.com/atom/atom/tree/master/packages/one-dark-syntax",
    },
    Credit {
        project: "Primer Primitives",
        themes: "Daylight",
        copyright: "Copyright (c) 2018 GitHub Inc.",
        license: "MIT",
        source: "https://github.com/primer/primitives",
    },
    Credit {
        project: "Solarized",
        themes: "Solarized Light",
        copyright: "Copyright (c) 2011 Ethan Schoonover",
        license: "MIT",
        source: "https://github.com/altercation/solarized",
    },
];

/// The MIT License's text after the copyright line, in the frame's three paragraphs.
pub const MIT_TEXT: [&str; 3] = [
    "Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the \"Software\"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:",
    "The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.",
    "THE SOFTWARE IS PROVIDED \"AS IS\", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.",
];

/// The dialog's introduction.
pub fn description() -> String {
    let name = naming::PRODUCT_NAME;
    format!(
        "{name}’s bundled themes are based on these open-source palettes. Some colors were adapted to fit {name}, such as status tints and surface steps. {name} isn’t affiliated with or endorsed by these projects."
    )
}

/// The Settings row's description.
pub const ROW_DESCRIPTION: &str = "Bundled themes are based on open-source palettes, used under the MIT License. Copyright stays with their authors.";

/// What Copy notices puts on the clipboard: the introduction, every entry with its themes, copyright line,
/// license and source, then the MIT License text.
pub fn notices() -> String {
    let mut out = vec![description(), String::new()];
    for credit in CREDITS {
        out.push(format!("{} → {}", credit.project, credit.themes));
        out.push(credit.copyright.to_owned());
        out.push(format!("License: {} · {}", credit.license, credit.source));
        out.push(String::new());
    }
    out.push("MIT License".to_owned());
    out.push(String::new());
    out.push(MIT_TEXT.join("\n\n"));
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::ThemeId;

    #[test]
    fn nine_projects_as_in_the_frame() {
        let projects: Vec<&str> = CREDITS.iter().map(|c| c.project).collect();
        assert_eq!(
            projects,
            [
                "Catppuccin",
                "Tokyo Night",
                "Dracula",
                "Nord",
                "Gruvbox",
                "Rosé Pine",
                "Atom One Dark",
                "Primer Primitives",
                "Solarized"
            ]
        );
        assert_eq!(CREDITS[4].license, "MIT/X11");
        assert!(
            CREDITS
                .iter()
                .filter(|c| c.project != "Gruvbox")
                .all(|c| c.license == "MIT")
        );
        assert!(
            CREDITS
                .iter()
                .all(|c| c.source.starts_with("https://github.com/"))
        );
    }

    #[test]
    fn every_bundled_theme_but_crewboard_is_credited() {
        let listed: Vec<String> = CREDITS
            .iter()
            .flat_map(|c| c.themes.split(", "))
            .map(|t| match t {
                // The frame lists the Catppuccin flavours after the first one without the family name.
                "Macchiato" | "Mocha" | "Latte" => format!("Catppuccin {t}"),
                other => other.to_owned(),
            })
            .collect();
        for id in ThemeId::DARK.into_iter().chain(ThemeId::LIGHT) {
            let own = matches!(id, ThemeId::OtaqDark | ThemeId::OtaqLight);
            assert_eq!(listed.contains(&id.name().to_string()), !own, "{id:?}");
        }
    }

    #[test]
    fn copied_notices_carry_every_entry_and_the_mit_text() {
        let text = notices();
        for credit in CREDITS {
            assert!(text.contains(credit.copyright), "{}", credit.project);
            assert!(text.contains(credit.source), "{}", credit.project);
        }
        for paragraph in MIT_TEXT {
            assert!(text.contains(paragraph));
        }
        assert!(text.starts_with(&description()));
        assert!(description().contains("isn’t affiliated with or endorsed by these projects."));
    }
}
