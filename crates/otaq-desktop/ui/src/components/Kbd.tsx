// Kbd (*01 · Components → Primitives → Kbd*): a shortcut hint. App shortcuts come from the keymap (`KeyAction::keys`
// in Rust, so `⌘J` on macOS and `Ctrl+J` on Windows and Linux); other hints, like `esc` in a dialog, are plain text.

export function Kbd({ keys }: { keys: string }) {
  return <kbd className="kbd">{keys}</kbd>;
}
