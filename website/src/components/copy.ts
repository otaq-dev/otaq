// The Copy buttons (CopyButton.astro): copy the command in the same row and show the check and "Copied" for 1.6 s,
// one button at a time, as the canvas's `copy`. A copy that fails changes nothing; the command stays selectable.

export const COPIED_MS = 1600;

function show(button: HTMLButtonElement, copied: boolean): void {
  button.querySelector("use")?.setAttribute("href", copied ? "#m-check" : "#m-copy");
  const label = button.querySelector("[data-copy-label]");
  if (label) label.textContent = copied ? "Copied" : "Copy";
}

export function startCopyButtons(): void {
  const buttons = [...document.querySelectorAll<HTMLButtonElement>("[data-copy]")];
  let shown: HTMLButtonElement | undefined;
  let timer: number | undefined;

  for (const button of buttons) {
    button.addEventListener("click", () => {
      const text = button.parentElement?.querySelector("[data-cmd]")?.textContent ?? "";
      navigator.clipboard.writeText(text).then(
        () => {
          if (shown) show(shown, false);
          shown = button;
          show(button, true);
          window.clearTimeout(timer);
          timer = window.setTimeout(() => {
            show(button, false);
            shown = undefined;
          }, COPIED_MS);
        },
        () => undefined,
      );
    });
  }
}
