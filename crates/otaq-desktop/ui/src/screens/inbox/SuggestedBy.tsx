// The line a lead's suggestion adds to the usual close (d0044) and delete (d0060) confirmations (LEAD-08; *10 → Row ·
// Epic lead items (0083) → Close… · the usual confirmation*): "Suggested by Wishlist lead: “…”", violet with the agent
// icon. The text is Rust's (`inbox.rs` `suggested_line`), carried in the overlay's `suggestion`.
import { Icon } from "../../theme/Icon";

export function SuggestedBy({ line }: { line: string }) {
  return (
    <p className="inbox-suggested-by">
      <Icon name="bot" size={13} />
      <span>{line}</span>
    </p>
  );
}
