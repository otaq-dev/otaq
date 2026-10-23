// Add project later (*14 → Adding a second project later*): from the rail's +, ⌘P → Add project, the empty
// board's "Add project folder…" or a folder dropped on the window. Tools were checked already, so it is one dialog:
// the folder, the key, the base branch; Esc or Cancel closes it. Adding selects the new project and closes the dialog
// onto its Flow setup page, which is the flow editor from Otaq default (FED-09, d0115; `FlowSetupPage.tsx`).
import type { OverlayProps } from "../../app/registry";
import { Button, Dialog } from "../../components";
import { AddButton, AddProjectFields, useAddProject } from "./AddProjectForm";
import { addedLater } from "./logic";

function AddProjectBody({ folder, close }: { folder: string | null; close: () => void }) {
  const state = useAddProject(false, folder, (key) => {
    addedLater(key);
    close();
  });
  const { view } = state;
  return (
    <Dialog
      title={view?.title ?? ""}
      width={480}
      onConfirm={state.add}
      onCancel={close}
      footer={
        <>
          <span className="onboarding-spacer" />
          {view && <Button label={view.cancel} variant="ghost" onClick={close} />}
          <AddButton state={state} />
        </>
      }
    >
      <AddProjectFields state={state} />
    </Dialog>
  );
}

export function AddProjectDialog({ params, close }: OverlayProps<"add_project">) {
  const folder = params?.folder ?? null;
  // Another folder (dropped while it is open) starts the form again.
  return <AddProjectBody key={folder ?? ""} folder={folder} close={close} />;
}
