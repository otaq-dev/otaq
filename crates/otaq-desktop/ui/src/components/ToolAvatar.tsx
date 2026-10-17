// The tool avatar (*01 · Components · Agent-first shell & agent tools (0086 · V) → Provider Mark (tool avatar)*;
// SHL-08, d0087): Otaq's own 16 px circle with the tool's two letters, on the tool's theme colours (`--tool-cc` and
// `--tool-cc-fg`). Never a logo, never a status colour. Hovering it shows the tool, and the model when one is
// stored. Whether avatars are drawn at all is Rust's (`tool_avatar::shown`): hidden while only one tool is connected.
import type { ToolAvatarView } from "../generated/ToolAvatarView";
import { Tooltip } from "./Tooltip";
import "./ToolAvatar.css";

export interface ToolAvatarProps {
  avatar: ToolAvatarView;
}

export function ToolAvatar({ avatar }: ToolAvatarProps) {
  return (
    <Tooltip text={avatar.hover}>
      <span
        className="tool-avatar"
        role="img"
        aria-label={avatar.hover}
        data-tool={avatar.tool}
        style={{ background: `var(--${avatar.background})`, color: `var(--${avatar.foreground})` }}
      >
        {avatar.letters}
      </span>
    </Tooltip>
  );
}
