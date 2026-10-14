//! Settings → About (d0071, d0076; *15 · Settings → Row · About & open-source licenses*): the mark, the name and the
//! line under it, the version with the commit it was built from and **Copy version info**, Otaq's own license with
//! **View license**, the **Open-source licenses** and **Theme credits** rows, and the trademark line. The version,
//! the commit, the OS and the license text come from the desktop app; this only words them.
//!
//! No update controls yet: 0073 puts them here once updates are built.

use serde::Serialize;
use ts_rs::TS;

use otaq_core::naming::PRODUCT_NAME;

use crate::view_models::theme_credits;

/// The line under the name (0071).
pub const TAGLINE: &str = "The board where you run your crew of agents";

/// What the build and the machine say about this copy of the app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Build {
    /// `0.1.0`.
    pub version: String,
    /// The short commit it was built from; `unknown` outside a git checkout.
    pub commit: String,
    /// `macOS 26.0`, `Windows 11 (26100)`, `Ubuntu 26.04`.
    pub os: String,
    /// `arm64` or `x64`, as people call them (see [`arch_name`]).
    pub arch: String,
}

/// *Settings · About*.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "AboutView")]
pub struct View {
    pub product: String,
    pub tagline: String,
    /// "Version 0.1.0 · build", followed by the commit chip.
    pub version: String,
    pub commit: String,
    pub version_description: String,
    pub copy_label: String,
    pub copied_label: String,
    /// What Copy version info puts on the clipboard: "Otaq 0.1.0 (bed9c10) · macOS 26.0 · arm64".
    pub version_info: String,
    /// "© 2026 The Otaq Authors · MIT License".
    pub copyright: String,
    pub license_description: String,
    pub view_license_label: String,
    pub licenses_label: String,
    pub licenses_description: String,
    pub view_licenses_label: String,
    pub credits_label: String,
    pub credits_description: String,
    pub view_credits_label: String,
    pub trademark: String,
}

/// *View license · dialog*: Otaq's own MIT text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "OwnLicenseDialog")]
pub struct LicenseView {
    pub title: String,
    pub description: String,
    /// The text in paragraphs, each with its hard line breaks joined.
    pub paragraphs: Vec<String>,
    /// What Copy license puts on the clipboard: the file as it is.
    pub text: String,
    pub copy_label: String,
    pub copied_label: String,
    pub done_label: String,
}

/// `std::env::consts::ARCH` as people call it: `aarch64` → `arm64`, `x86_64` → `x64`.
pub fn arch_name(arch: &str) -> &str {
    match arch {
        "aarch64" => "arm64",
        "x86_64" => "x64",
        other => other,
    }
}

/// The license file's copyright holder line, as the About row shows it: "© 2026 The Otaq Authors · MIT License".
pub fn copyright(license: &str) -> String {
    let holder = license
        .lines()
        .find_map(|l| l.trim().strip_prefix("Copyright (c) "))
        .unwrap_or(PRODUCT_NAME);
    format!("© {holder} · MIT License")
}

/// The About section for this build. `rust_crates` and `npm_packages` are the notices file's counts.
pub fn view(build: &Build, license: &str, rust_crates: usize, npm_packages: usize) -> View {
    let name = PRODUCT_NAME;
    View {
        product: name.into(),
        tagline: TAGLINE.into(),
        version: format!("Version {} · build", build.version),
        commit: build.commit.clone(),
        version_description:
            "For bug reports: copies the version, the build commit, your OS and its architecture."
                .into(),
        copy_label: "Copy version info".into(),
        copied_label: "Copied".into(),
        version_info: format!(
            "{name} {} ({}) · {} · {}",
            build.version, build.commit, build.os, build.arch
        ),
        copyright: copyright(license),
        license_description: format!(
            "{name} itself is free and open source under the MIT License."
        ),
        view_license_label: "View license".into(),
        licenses_label: "Open-source licenses".into(),
        licenses_description: format!(
            "{name} ships with {rust_crates} open-source Rust crates, {npm_packages} npm packages, fonts, icons and syntax definitions."
        ),
        view_licenses_label: "View".into(),
        credits_label: "Theme credits".into(),
        credits_description: theme_credits::ROW_DESCRIPTION.into(),
        view_credits_label: "View credits".into(),
        trademark: format!(
            "Claude and Claude Code are trademarks of Anthropic. {name} is an independent project and isn’t affiliated with or endorsed by Anthropic."
        ),
    }
}

/// *View license · dialog* for the license file `text`.
pub fn license(text: &str) -> LicenseView {
    let paragraphs = text
        .split("\n\n")
        .map(|p| p.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|p| !p.is_empty())
        .collect();
    LicenseView {
        title: "MIT License".into(),
        description: format!(
            "{PRODUCT_NAME}’s own license. It applies to the app and its source code."
        ),
        paragraphs,
        text: text.to_owned(),
        copy_label: "Copy license".into(),
        copied_label: "Copied".into(),
        done_label: "Done".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LICENSE: &str = "MIT License\n\nCopyright (c) 2026 The Example Authors\n\nPermission is hereby granted, free of charge,\nto any person\n\nTHE SOFTWARE IS PROVIDED \"AS IS\"\n";

    fn build() -> Build {
        Build {
            version: "0.1.0".into(),
            commit: "bed9c10".into(),
            os: "macOS 26.0".into(),
            arch: arch_name("aarch64").into(),
        }
    }

    #[test]
    fn the_rows_as_the_frame_words_them() {
        let v = view(&build(), LICENSE, 364, 9);
        assert_eq!(v.product, PRODUCT_NAME);
        assert_eq!(v.version, "Version 0.1.0 · build");
        assert_eq!(v.commit, "bed9c10");
        assert_eq!(v.copyright, "© 2026 The Example Authors · MIT License");
        assert_eq!(
            v.licenses_description,
            format!(
                "{PRODUCT_NAME} ships with 364 open-source Rust crates, 9 npm packages, fonts, icons and syntax definitions."
            )
        );
        assert!(v.trademark.starts_with(&format!(
            "Claude and Claude Code are trademarks of Anthropic. {PRODUCT_NAME} is"
        )));
    }

    #[test]
    fn copy_version_info_names_version_commit_os_and_arch() {
        assert_eq!(
            view(&build(), LICENSE, 1, 1).version_info,
            format!("{PRODUCT_NAME} 0.1.0 (bed9c10) · macOS 26.0 · arm64")
        );
        assert_eq!(arch_name("x86_64"), "x64");
        assert_eq!(arch_name("riscv64"), "riscv64");
    }

    #[test]
    fn the_license_in_paragraphs_and_whole_for_the_clipboard() {
        let l = license(LICENSE);
        assert_eq!(l.paragraphs.len(), 4);
        assert_eq!(l.paragraphs[1], "Copyright (c) 2026 The Example Authors");
        assert_eq!(
            l.paragraphs[2],
            "Permission is hereby granted, free of charge, to any person"
        );
        assert_eq!(l.text, LICENSE);
    }
}
