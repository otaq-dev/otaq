// The key log (`CREWBOARD_KEY_LOG=1`): the last keys the page got and where each went, for checking the §21.3 map by
// hand (T-0215). Draws nothing unless the flag is set.
import "./keylog.css";
import { keyLog, keymap } from "./keyboard";
import { useStore } from "./store";

export function KeyLog() {
  const map = useStore(keymap);
  const lines = useStore(keyLog);
  if (map?.log !== true) return null;
  return (
    <ol className="key-log" role="log" aria-label="Key log">
      {lines.map((line, i) => (
        // Lines repeat (the same key twice); the position is the identity.
        <li key={i}>{line}</li>
      ))}
    </ol>
  );
}
