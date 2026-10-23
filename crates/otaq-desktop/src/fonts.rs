//! The installed fonts (d0117 point 5, T-0482): the families in the OS font folders, read with fontdb once per app
//! start, the first time the Settings page or a terminal asks, and kept. Which of them the Terminal font dropdown
//! lists and which fill in missing symbols is the view model's (`otaq_view::view_models::terminal_font`).

use std::path::PathBuf;
use std::sync::Arc;

use otaq_view::view_models::terminal_font::InstalledFonts;
use tokio::sync::OnceCell;

/// Where the fonts are read from, and the list once read.
pub struct Fonts {
    /// `None`: the OS font folders. Tests name a fixture folder (or none), never the real ones.
    folders: Option<Vec<PathBuf>>,
    list: OnceCell<Arc<InstalledFonts>>,
}

impl Fonts {
    /// The OS's font folders (macOS `/System/Library/Fonts`, `/Library/Fonts`, `~/Library/Fonts`; Windows'
    /// `Fonts`; fontconfig's folders on Linux).
    pub fn system() -> Self {
        Self {
            folders: None,
            list: OnceCell::new(),
        }
    }

    /// Only the fonts in `folders` (tests).
    #[cfg(test)]
    pub fn in_folders(folders: Vec<PathBuf>) -> Self {
        Self {
            folders: Some(folders),
            list: OnceCell::new(),
        }
    }

    /// The installed fonts, read the first time (off the async threads: a few hundred files are parsed).
    pub async fn get(&self) -> Arc<InstalledFonts> {
        self.list
            .get_or_init(|| async {
                let folders = self.folders.clone();
                let read = tokio::task::spawn_blocking(move || read(folders.as_deref()));
                Arc::new(read.await.unwrap_or_default())
            })
            .await
            .clone()
    }
}

/// Reads every face's family name and whether it is monospaced.
fn read(folders: Option<&[PathBuf]>) -> InstalledFonts {
    let mut db = fontdb::Database::new();
    match folders {
        None => db.load_system_fonts(),
        Some(folders) => {
            for folder in folders {
                db.load_fonts_dir(folder);
            }
        }
    }
    InstalledFonts::from_faces(db.faces().filter_map(|face| {
        // fontdb puts the English name first.
        let (family, _) = face.families.first()?;
        Some((family.as_str(), face.monospaced))
    }))
}

#[cfg(test)]
pub mod fixture {
    //! Tiny font files for tests: only the tables fontdb reads (`name` for the family, `post` for monospaced).

    use std::path::Path;

    /// Writes a font file for `family` into `folder`.
    pub fn write(folder: &Path, file: &str, family: &str, monospaced: bool) {
        std::fs::write(folder.join(file), font(family, monospaced)).unwrap();
    }

    fn font(family: &str, monospaced: bool) -> Vec<u8> {
        let ps_name: String = family.chars().filter(|c| !c.is_whitespace()).collect();
        let tables = [
            (*b"name", name_table(&[(1, family), (6, &ps_name)])),
            (*b"post", post_table(monospaced)),
        ];
        let mut out = Vec::new();
        out.extend(0x0001_0000u32.to_be_bytes());
        out.extend((tables.len() as u16).to_be_bytes());
        out.extend([0u8; 6]); // searchRange, entrySelector, rangeShift: unread
        let mut offset = 12 + 16 * tables.len();
        let mut data = Vec::new();
        for (tag, table) in &tables {
            out.extend(tag);
            out.extend(0u32.to_be_bytes()); // checksum: unread
            out.extend((offset as u32).to_be_bytes());
            out.extend((table.len() as u32).to_be_bytes());
            let padded = table.len().div_ceil(4) * 4;
            data.extend(table);
            data.resize(data.len() + padded - table.len(), 0);
            offset += padded;
        }
        out.extend(data);
        out
    }

    /// A format 0 `name` table with Windows Unicode English (US) names.
    fn name_table(names: &[(u16, &str)]) -> Vec<u8> {
        let mut strings = Vec::new();
        let mut records = Vec::new();
        for (id, text) in names {
            let utf16: Vec<u8> = text.encode_utf16().flat_map(u16::to_be_bytes).collect();
            for value in [
                3u16,
                1,
                0x0409,
                *id,
                utf16.len() as u16,
                strings.len() as u16,
            ] {
                records.extend(value.to_be_bytes());
            }
            strings.extend(utf16);
        }
        let mut out = Vec::new();
        for value in [0u16, names.len() as u16, (6 + records.len()) as u16] {
            out.extend(value.to_be_bytes());
        }
        out.extend(records);
        out.extend(strings);
        out
    }

    /// A version 3 `post` table: isFixedPitch is the u32 at offset 12.
    fn post_table(monospaced: bool) -> Vec<u8> {
        let mut out = vec![0u8; 32];
        out[..4].copy_from_slice(&0x0003_0000u32.to_be_bytes());
        out[12..16].copy_from_slice(&u32::from(monospaced).to_be_bytes());
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn the_list_comes_from_the_folder_once() {
        let folder = tempfile::tempdir().unwrap();
        fixture::write(folder.path(), "mono.ttf", "Fixture Mono", true);
        fixture::write(folder.path(), "sans.ttf", "Fixture Sans", false);
        fixture::write(folder.path(), "nerd.ttf", "Fixture Nerd Font Mono", true);
        std::fs::write(folder.path().join("broken.ttf"), b"not a font").unwrap();
        let fonts = Fonts::in_folders(vec![folder.path().to_owned()]);
        let list = fonts.get().await;
        assert_eq!(list.monospace, ["Fixture Mono", "Fixture Nerd Font Mono"]);
        assert_eq!(list.nerd, ["Fixture Nerd Font Mono"]);

        // Read once per start: a font installed later shows at the next start.
        fixture::write(folder.path(), "later.ttf", "Later Mono", true);
        assert_eq!(fonts.get().await.monospace, list.monospace);
    }
}
