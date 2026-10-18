#!/usr/bin/env bash
# Build Otaq from this checkout and install it the way users get it (decision 0072, macOS): Otaq.app in
# ~/Applications, and ~/.local/bin/otaq pointing into it. No admin rights; shell files are never edited.
#
#   scripts/install-local.sh                       release build → ~/Applications/Otaq.app + ~/.local/bin/otaq
#   scripts/install-local.sh --dest <folder>       <folder>/Otaq.app only, no link (tests, teammates, checks)
#   scripts/install-local.sh --bundle-id <id>      another bundle ID (default io.otaq.desktop), for a check bundle
#                                                  that asks macOS without using up the real ID's one question
#                                                  (never in a temporary folder such as /tmp: macOS refuses
#                                                  notifications for apps there)
#   scripts/install-local.sh --from <folder>       take otaq and otaq-app from <folder> instead of building (tests)
#
# Otaq.app holds both programs in Contents/MacOS (otaq-app is the bundle's main program, otaq the command and the
# background service), the agent tools' plugin folders in Contents/Resources, the tent icon and an Info.plist, and
# is signed ad-hoc (decision 0021). Before replacing an installed Otaq.app it stops the background service of the
# data folder in use (OTAQ_HOME, or ~/.otaq) with the old bundle's otaq.

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
app_name="Otaq.app"
default_id="io.otaq.desktop"

dest=""
bundle_id="$default_id"
from=""
while [ $# -gt 0 ]; do
    case "$1" in
        --dest) dest="${2:?--dest needs a folder}"; shift 2 ;;
        --bundle-id) bundle_id="${2:?--bundle-id needs an ID}"; shift 2 ;;
        --from) from="${2:?--from needs a folder}"; shift 2 ;;
        -h|--help) sed -n '2,17p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "install-local.sh: unknown option $1 (see --help)" >&2; exit 2 ;;
    esac
done

if [ "$(uname -s)" != "Darwin" ]; then
    echo "install-local.sh: builds Otaq.app, so it runs on macOS only" >&2
    exit 1
fi
link=1
if [ -n "$dest" ]; then
    link=0
else
    dest="$HOME/Applications"
fi

# The version every crate shares: [workspace.package] in Cargo.toml.
version="$(sed -n '/^\[workspace\.package\]/,/^\[/s/^version *= *"\(.*\)"/\1/p' "$root/Cargo.toml")"
if [ -z "$version" ]; then
    echo "install-local.sh: no version in Cargo.toml's [workspace.package]" >&2
    exit 1
fi

if [ -z "$from" ]; then
    echo "==> Building the release (the app with its page inside, then the otaq command)"
    (cd "$root/crates/otaq-desktop" && npm run tauri build)
    (cd "$root" && cargo build --release -p otaq-cli)
    from="${CARGO_TARGET_DIR:-$root/target}/release"
fi
for program in otaq otaq-app; do
    if [ ! -x "$from/$program" ]; then
        echo "install-local.sh: $from/$program is missing" >&2
        exit 1
    fi
done

mkdir -p "$dest"
dest="$(cd "$dest" && pwd -P)"
target="$dest/$app_name"
staging="$(mktemp -d "$dest/.otaq-install.XXXXXX")"
trap 'rm -rf "$staging"' EXIT
bundle="$staging/$app_name"

echo "==> Assembling $app_name ($bundle_id, version $version)"
mkdir -p "$bundle/Contents/MacOS" "$bundle/Contents/Resources"
cp "$from/otaq-app" "$from/otaq" "$bundle/Contents/MacOS/"
cp "$root/crates/otaq-desktop/icons/icon.icns" "$bundle/Contents/Resources/icon.icns"
# Every plugin folder the agent tools look for: plugin/ (Claude Code, and Codex's and OpenCode's parts inside it)
# and plugin-copilot/ beside it. cp -R keeps plugin-copilot/skills a relative link to ../plugin/skills.
cp -R "$root/plugin" "$root/plugin-copilot" "$bundle/Contents/Resources/"
find "$bundle/Contents/Resources" -name .DS_Store -delete

# LSMinimumSystemVersion: the value Tauri's bundler writes by default (bundle > macOS > minimumSystemVersion).
cat > "$bundle/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleDevelopmentRegion</key>
	<string>en</string>
	<key>CFBundleDisplayName</key>
	<string>Otaq</string>
	<key>CFBundleExecutable</key>
	<string>otaq-app</string>
	<key>CFBundleIconFile</key>
	<string>icon.icns</string>
	<key>CFBundleIdentifier</key>
	<string>$bundle_id</string>
	<key>CFBundleInfoDictionaryVersion</key>
	<string>6.0</string>
	<key>CFBundleName</key>
	<string>Otaq</string>
	<key>CFBundlePackageType</key>
	<string>APPL</string>
	<key>CFBundleShortVersionString</key>
	<string>$version</string>
	<key>CFBundleVersion</key>
	<string>$version</string>
	<key>LSMinimumSystemVersion</key>
	<string>10.13</string>
	<key>NSHighResolutionCapable</key>
	<true/>
</dict>
</plist>
PLIST
plutil -lint -s "$bundle/Contents/Info.plist"

echo "==> Signing ad-hoc"
# otaq first, under the bundle's ID (by default codesign would name it "otaq"), then the bundle, which seals it.
codesign --force --sign - --identifier "$bundle_id" "$bundle/Contents/MacOS/otaq"
codesign --force --sign - "$bundle"
codesign --verify --deep --strict "$bundle"

old_otaq="$target/Contents/MacOS/otaq"
if [ -x "$old_otaq" ]; then
    echo "==> Stopping the background service of ${OTAQ_HOME:-~/.otaq} before replacing $target"
    "$old_otaq" daemon stop || echo "    (the old otaq couldn't stop it; carrying on)"
fi
rm -rf "$target"
mv "$bundle" "$target"
echo "==> Installed $target"

if [ "$link" = 1 ]; then
    bin="$HOME/.local/bin"
    mkdir -p "$bin"
    ln -sfn "$target/Contents/MacOS/otaq" "$bin/otaq"
    echo "==> Linked $bin/otaq → $target/Contents/MacOS/otaq"
    case ":$PATH:" in
        *":$bin:"*) ;;
        *) echo "Note: $bin isn't on your PATH, so the otaq command isn't found yet. Add it in your shell's" \
                "startup file, for example: export PATH=\"\$HOME/.local/bin:\$PATH\"" ;;
    esac
fi
