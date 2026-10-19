//! Files dropped on the window (d0099, T-0388). The door forwards every drop here; the page decides where it landed:
//! over a terminal that takes typing it pastes [`paste_text`] into it, over a terminal that doesn't it does nothing,
//! anywhere else Add project opens with the first path (*02 → No projects yet*). Otaq does nothing else with the
//! files: no copy, no link, no launch flag, nothing written anywhere.

use std::path::PathBuf;

use crate::App;
use crate::events::PageEvent;

/// The system the window runs on: the drop point's units and the way a pasted path is quoted differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Mac,
    Windows,
    Linux,
}

impl Os {
    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::Mac
        } else if cfg!(windows) {
            Self::Windows
        } else {
            Self::Linux
        }
    }
}

/// Files dropped at `position` on the window: the page hears them as `files_dropped`.
pub fn dropped(app: &App, paths: &[PathBuf], position: (f64, f64), scale: f64) {
    if paths.is_empty() {
        return;
    }
    let os = Os::current();
    let paths: Vec<String> = paths.iter().map(|p| p.display().to_string()).collect();
    let (x, y) = drop_point(position, scale, os);
    app.events.publish(&PageEvent::FilesDropped {
        paste: paste_text(&paths, os),
        paths,
        x,
        y,
    });
}

/// The drop point in the page's CSS pixels. Tauri 2.12 types it as physical pixels relative to the webview, but wry
/// 0.57 fills it in each system's own units: on macOS the view's points (`draggingLocation`, flipped), on Linux GTK's
/// widget coordinates, both already CSS pixels; only on Windows client pixels (`ScreenToClient`), divided by the scale.
pub fn drop_point(position: (f64, f64), scale: f64, os: Os) -> (f64, f64) {
    match os {
        Os::Windows if scale > 0.0 => (position.0 / scale, position.1 / scale),
        _ => position,
    }
}

/// What a terminal receives: every path quoted, one space between them and one after the last, so the user can keep
/// typing.
pub fn paste_text(paths: &[String], os: Os) -> String {
    paths
        .iter()
        .map(|p| format!("{} ", quote_path(p, os)))
        .collect()
}

/// One path as Terminal.app pastes it: a backslash before every character that isn't safe in a shell word. Letters
/// and digits of any script, other non-ASCII characters (emoji) and `_ - . / , : @ + %` stay as they are; `=` stays
/// unless it comes first.
// TODO(Q219): Windows and Linux are unchecked (d0021). Windows Terminal wraps a path with spaces in double quotes
// (a backslash is its path separator); GNOME Terminal wraps every path in single quotes. Built as Q219 option 1.
pub fn quote_path(path: &str, os: Os) -> String {
    match os {
        Os::Windows => {
            if path.contains([' ', '\t']) {
                format!("\"{path}\"")
            } else {
                path.to_owned()
            }
        }
        Os::Mac | Os::Linux => {
            let mut quoted = String::with_capacity(path.len());
            for (i, c) in path.chars().enumerate() {
                let safe = !c.is_ascii()
                    || c.is_ascii_alphanumeric()
                    || "_-./,:@+%".contains(c)
                    || (c == '=' && i > 0);
                if !safe {
                    quoted.push('\\');
                }
                quoted.push(c);
            }
            quoted
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mac(path: &str) -> String {
        quote_path(path, Os::Mac)
    }

    #[test]
    fn a_plain_path_stays_as_it_is() {
        assert_eq!(
            mac("/Users/me/Downloads/shot_2026-10-19.v2,final:a@b+c%d.png"),
            "/Users/me/Downloads/shot_2026-10-19.v2,final:a@b+c%d.png"
        );
    }

    #[test]
    fn spaces_quotes_and_shell_characters_get_a_backslash() {
        assert_eq!(mac("/tmp/Screen Shot.png"), r"/tmp/Screen\ Shot.png");
        assert_eq!(mac("/tmp/it's \"x\".pdf"), r#"/tmp/it\'s\ \"x\".pdf"#);
        assert_eq!(mac("/tmp/$HOME `x` !1"), r"/tmp/\$HOME\ \`x\`\ \!1");
        assert_eq!(
            mac("/tmp/a&b;c|d<e>f(g)h[i]j{k}l*m?n#o~p^q"),
            r"/tmp/a\&b\;c\|d\<e\>f\(g\)h\[i\]j\{k\}l\*m\?n\#o\~p\^q"
        );
    }

    #[test]
    fn an_equals_sign_is_escaped_only_first() {
        assert_eq!(mac("=x"), r"\=x");
        assert_eq!(mac("/tmp/a=b"), "/tmp/a=b");
    }

    #[test]
    fn a_name_that_looks_escaped_is_escaped_again() {
        assert_eq!(mac(r"/tmp/a\b"), r"/tmp/a\\b");
        assert_eq!(mac(r"/tmp/a\ b"), r"/tmp/a\\\ b");
    }

    #[test]
    fn unicode_letters_and_emoji_stay() {
        assert_eq!(
            mac("/Users/me/Belgeler/Çalışma ğüşö é 📄.pdf"),
            r"/Users/me/Belgeler/Çalışma\ ğüşö\ é\ 📄.pdf"
        );
    }

    #[test]
    fn several_paths_and_a_folder_are_one_paste_ending_in_a_space() {
        let paths = [
            "/tmp/one.png".to_owned(),
            "/tmp/two words.txt".to_owned(),
            "/Users/me/My Folder".to_owned(),
        ];
        assert_eq!(
            paste_text(&paths, Os::Mac),
            r"/tmp/one.png /tmp/two\ words.txt /Users/me/My\ Folder "
        );
        assert_eq!(paste_text(&paths[..1], Os::Mac), "/tmp/one.png ");
    }

    #[test]
    fn windows_wraps_a_path_with_spaces_in_double_quotes() {
        let paths = [r"C:\Users\me\a b.png".to_owned(), r"C:\x.txt".to_owned()];
        assert_eq!(
            paste_text(&paths, Os::Windows),
            r#""C:\Users\me\a b.png" C:\x.txt "#
        );
    }

    #[test]
    fn the_drop_point_is_in_css_pixels() {
        assert_eq!(drop_point((300.0, 120.0), 2.0, Os::Mac), (300.0, 120.0));
        assert_eq!(drop_point((300.0, 120.0), 2.0, Os::Linux), (300.0, 120.0));
        assert_eq!(drop_point((300.0, 120.0), 2.0, Os::Windows), (150.0, 60.0));
        assert_eq!(drop_point((300.0, 120.0), 1.5, Os::Windows), (200.0, 80.0));
        assert_eq!(drop_point((300.0, 120.0), 0.0, Os::Windows), (300.0, 120.0));
    }
}
