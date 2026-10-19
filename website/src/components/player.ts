// The How it works player in the browser: draws a beat of flow.ts into HowItWorks.astro's markup and runs the
// 3.2 s timer, Pause / Play and the step buttons, as the canvas's script does (WEB-05).

import { BEAT_MS, type Player, beatView, jump, tick, toggle } from "./flow";

function setIcon(svg: Element | null, name: string): void {
  svg?.querySelector("use")?.setAttribute("href", `#m-${name}`);
}

function setText(el: Element | null, text: string): void {
  const target = el?.querySelector("[data-text]") ?? el;
  if (target && target.textContent !== text) target.textContent = text;
}

export function startPlayer(): void {
  const root = document.querySelector<HTMLElement>("[data-flow]");
  const stage = root?.querySelector<HTMLElement>(".stage");
  const card = root?.querySelector<HTMLElement>("[data-card]");
  const ghost = root?.querySelector<HTMLElement>("[data-ghost]");
  const playButton = root?.querySelector<HTMLButtonElement>("[data-play]");
  if (!root || !stage || !card || !ghost || !playButton) return;
  const lanes = [...root.querySelectorAll<HTMLElement>("[data-lane]")];
  const steps = [...root.querySelectorAll<HTMLButtonElement>("[data-step]")];

  const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  let player: Player = { beat: 0, playing: !reduce };
  let timer: number | undefined;

  function draw(): void {
    if (!stage || !card || !ghost || !playButton) return;
    const v = beatView(player.beat);
    lanes.forEach((lane, i) => lane.classList.toggle("on", v.lanesOn[i] === true));

    card.style.left = v.card.left;
    card.classList.toggle("attn", v.card.attn);
    const av = card.querySelector("[data-card-av]");
    if (av) {
      av.className = `av ${v.card.avClass}`;
      av.textContent = v.card.av;
    }
    const pill = card.querySelector("[data-card-pill]");
    if (pill) {
      pill.className = `pill ${v.card.pillClass}`;
      setIcon(pill, v.card.pillIcon);
      pill.querySelector("svg")?.classList.toggle("spin", v.card.spin);
      setText(pill, v.card.pillLabel);
    }
    const meta = card.querySelector("[data-card-meta]");
    if (meta) {
      meta.classList.toggle("inv", v.card.metaInv);
      setIcon(meta, v.card.metaIcon);
      setText(meta, v.card.meta);
    }

    ghost.style.left = v.ghost.left;
    const ghostPill = ghost.querySelector("[data-ghost-pill]");
    if (ghostPill) {
      ghostPill.className = `pill ${v.ghost.pillClass}`;
      setIcon(ghostPill, v.ghost.icon);
      setText(ghostPill, v.ghost.label);
    }

    // The bubble is added when a beat has one (it pops in) and kept while the next beat has one too (it slides).
    let bubble = stage.querySelector<HTMLElement>("[data-bubble]");
    if (!v.bubble) {
      bubble?.remove();
    } else {
      if (!bubble) {
        bubble = document.createElement("div");
        bubble.className = "bub";
        bubble.dataset["bubble"] = "";
        const avatar = document.createElement("span");
        avatar.style.cssText = "width:20px;height:20px;font-size:8px";
        bubble.append(avatar, document.createElement("span"));
        stage.append(bubble);
      }
      bubble.style.left = v.bubble.left;
      const [avatar, line] = bubble.children;
      if (avatar) {
        avatar.className = `av ${v.bubble.avClass}`;
        avatar.textContent = v.bubble.initials;
      }
      if (line) {
        const who = document.createElement("b");
        who.textContent = v.bubble.who;
        line.replaceChildren(who, ` ${v.bubble.text}`);
      }
    }

    setText(root?.querySelector("[data-now-n]") ?? null, v.n);
    setText(root?.querySelector("[data-now-title]") ?? null, v.title);
    setText(root?.querySelector("[data-now-text]") ?? null, v.text);

    steps.forEach((step, i) => {
      const s = v.steps[i];
      step.className = ["stp", s?.cls, player.playing ? "" : "paused"].filter(Boolean).join(" ");
      step.setAttribute("aria-current", s?.current ? "step" : "false");
      const bar = step.querySelector(".bar");
      if (bar) bar.className = `bar ${v.bar}`;
    });

    setIcon(playButton, player.playing ? "pause" : "play");
    setText(playButton, player.playing ? "Pause" : "Play");
  }

  // Not drawn, so the drawing can't show it: the timer only runs while the section is on screen and the tab is
  // visible. Coming back starts the beat's 3.2 s (and its progress bar) afresh, so the bar and the timer stay together.
  let onScreen = false;
  const running = (): boolean => onScreen && !document.hidden;

  function restartBar(): void {
    const bar = steps[player.beat]?.querySelector<HTMLElement>(".bar");
    if (!bar) return;
    bar.style.animation = "none";
    bar.getBoundingClientRect(); // a reflow, so the cleared animation starts again
    bar.style.animation = "";
  }

  function restartTimer(): void {
    window.clearInterval(timer);
    timer = undefined;
    if (!running()) return;
    timer = window.setInterval(() => {
      if (!player.playing) return;
      player = tick(player);
      draw();
    }, BEAT_MS);
  }

  function resume(): void {
    if (running() && timer === undefined) {
      restartBar();
      restartTimer();
    } else if (!running()) {
      restartTimer();
    }
  }

  steps.forEach((step, i) => {
    step.addEventListener("click", () => {
      player = jump(player, i);
      draw();
      restartTimer();
    });
  });
  playButton.addEventListener("click", () => {
    player = toggle(player);
    draw();
    restartTimer();
  });

  new IntersectionObserver((entries) => {
    onScreen = entries.some((e) => e.isIntersecting);
    resume();
  }).observe(root);
  document.addEventListener("visibilitychange", resume);

  draw();
}
