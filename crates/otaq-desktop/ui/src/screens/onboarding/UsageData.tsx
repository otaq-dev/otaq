// "Help improve Otaq" (d0085): asked once, when first run ends (and at the next start for someone who was already
// past it). Nothing is sent unless the user presses Send anonymous data; No thanks, Esc and a click outside all answer
// no. Either answer is stored, and Settings → Data changes it later. The words are Rust's (`usage_data.rs`).
import type { OverlayProps } from "../../app/registry";
import { ConfirmDialog } from "../../components";
import type { UsageDataAsk } from "../../generated/UsageDataAsk";
import type { UsageDataDialog } from "../../generated/UsageDataDialog";
import { request } from "../../tauri";
import { Icon } from "../../theme/Icon";
import type { Ask } from "./logic";

/** Opens the dialog when the user hasn't answered yet. Returns whether it opened. */
export async function askUsageDataWhenNeeded(
  ask: Ask,
  open: (dialog: UsageDataDialog) => void,
): Promise<boolean> {
  const answer = (await ask("usage.dialog")) as UsageDataAsk;
  if (!answer.show) return false;
  open(answer.dialog);
  return true;
}

export function UsageDataOverlay({ params, close }: OverlayProps<"usage_data">) {
  const choose = (on: boolean) => {
    close();
    request("usage.choose", { on }).catch(() => undefined);
  };
  return (
    <ConfirmDialog
      title={params.title}
      width={460}
      body={
        <>
          <p className="dialog-text">{params.text}</p>
          <ul className="confirm-rows" aria-label={params.sent_heading}>
            {params.sent.map((line) => (
              <li key={line} className="confirm-row">
                <Icon name="check" size={13} />
                <span>{line}</span>
              </li>
            ))}
          </ul>
          <ul className="confirm-rows" aria-label={params.never_heading}>
            {params.never.map((line) => (
              <li key={line} className="confirm-row">
                <Icon name="ban" size={13} />
                <span>{line}</span>
              </li>
            ))}
          </ul>
          <p className="dialog-text">{params.foot}</p>
        </>
      }
      confirmLabel={params.accept_label}
      secondary={{
        label: params.decline_label,
        onClick: () => {
          choose(false);
        },
      }}
      onConfirm={() => {
        choose(true);
      }}
      onCancel={() => {
        choose(false);
      }}
    />
  );
}
