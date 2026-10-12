// The board with no project at all (*02 → No projects yet*, *Dark · no projects*; T-0233): onboarding was skipped or
// every project was removed. Add project folder… opens the system picker, then Add project with the folder; Check
// tools opens onboarding on its tools step; a folder dropped anywhere on the window works too (the onboarding
// screen's drop). Its words are `NoProjectsView`'s.
import { useCallback } from "react";

import { openOverlay } from "../../app/overlays";
import { Button } from "../../components";
import type { OnboardingStart } from "../../generated/OnboardingStart";
import type { OnboardingTexts } from "../../generated/OnboardingTexts";
import { useServiceView } from "../../service";
import { pickFolder, request } from "../../tauri";
import { Icon } from "../../theme/Icon";

/** Shown only once Rust says there is no project (not while the first project is still loading). */
export function NoProjects() {
  const load = useCallback(
    async () => ({
      start: (await request("onboarding.start")) as OnboardingStart,
      texts: (await request("onboarding.texts")) as OnboardingTexts,
    }),
    [],
  );
  const view = useServiceView(load, ["projects_changed"]).view;
  if (!view?.start.show) return null;
  const words = view.texts.no_projects;
  return (
    <div className="board-no-projects" role="region" aria-label={words.title}>
      <span className="board-no-projects-icon">
        <Icon name="folder-git-2" size={22} />
      </span>
      <h2 className="board-empty-title">{words.title}</h2>
      <p className="board-empty-text board-no-projects-text">{words.text}</p>
      <div className="board-empty-buttons">
        <Button
          label={words.add}
          icon="folder-plus"
          onClick={() => {
            pickFolder(words.add).then(
              (folder) => {
                if (folder !== null) openOverlay("add_project", { folder });
              },
              () => {
                openOverlay("add_project");
              },
            );
          }}
        />
        <Button
          label={words.check}
          icon="list-checks"
          variant="secondary"
          onClick={() => {
            openOverlay("onboarding", { step: "tools" });
          }}
        />
      </div>
      <span className="board-empty-hint">{words.drop}</span>
    </div>
  );
}
