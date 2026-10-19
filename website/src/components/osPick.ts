// The visitor's OS (os.ts) picks the Download label and note, the hero command and the Install tab; the tabs then
// switch the Install card only (WEB-05).

import { OS, OS_KEYS, type OsKey, pickOs } from "./os";

function setAll(selector: string, text: string, root: ParentNode = document): void {
  root.querySelectorAll(selector).forEach((el) => {
    el.textContent = text;
  });
}

function showInstall(card: HTMLElement, key: OsKey): void {
  const os = OS[key];
  card.querySelectorAll<HTMLButtonElement>("[data-os-tab]").forEach((tab) => {
    const on = tab.dataset["osTab"] === key;
    tab.classList.toggle("on", on);
    tab.setAttribute("aria-selected", on ? "true" : "false");
  });
  setAll("[data-os-intro]", os.intro, card);
  setAll("[data-os-command]", os.command, card);
  setAll("[data-os-path]", os.path, card);
  setAll("[data-os-archive]", os.archive, card);
  setAll("[data-os-foot]", os.foot, card);
}

export function startOsPick(): void {
  let key: OsKey = "mac";
  try {
    key = pickOs(navigator.userAgent, navigator.platform);
  } catch {
    // Unknown: macOS, as drawn.
  }
  const os = OS[key];
  const card = document.querySelector<HTMLElement>("[data-install]");

  setAll("[data-os-download]", os.download);
  setAll("[data-os-note]", os.note);
  document.querySelectorAll(".oneliner [data-os-command]").forEach((el) => {
    el.textContent = os.command;
  });
  if (!card) return;
  showInstall(card, key);
  card.querySelectorAll<HTMLButtonElement>("[data-os-tab]").forEach((tab) => {
    tab.addEventListener("click", () => {
      const k = OS_KEYS.find((x) => x === tab.dataset["osTab"]);
      if (k) showInstall(card, k);
    });
  });
}
