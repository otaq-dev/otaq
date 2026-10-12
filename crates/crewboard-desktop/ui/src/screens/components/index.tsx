// Area 01 · Components (T-0217): the confirm dialog every screen opens by id (`openOverlay("confirm", …)`), and
// the developer-only design gallery, opened over the app when it starts with `CREWBOARD_GALLERY=1`. The components
// themselves are in `ui/src/components/`.
import { openOverlay } from "../../app/overlays";
import type { OverlayProps, Screen } from "../../app/registry";
import { ConfirmDialog } from "../../components";
import { request } from "../../tauri";
import { Gallery } from "./Gallery";

function ConfirmOverlay({ params, close }: OverlayProps<"confirm">) {
  return (
    <ConfirmDialog
      title={params.title}
      body={params.body}
      confirmLabel={params.confirmLabel}
      destructive={params.destructive}
      onConfirm={() => {
        close();
        void params.onConfirm();
      }}
      onCancel={close}
    />
  );
}

/** Opens the gallery when the app was started with `CREWBOARD_GALLERY=1` (`gallery.wanted`). */
export async function openGalleryIfWanted(ask: typeof request = request): Promise<boolean> {
  const wanted = await ask("gallery.wanted").then(
    (answer) => answer === true,
    () => false,
  );
  if (wanted) openOverlay("gallery");
  return wanted;
}

export const components: Screen = (app) => {
  app.overlays.register("confirm", { Component: ConfirmOverlay });
  app.overlays.register("gallery", { Component: Gallery });
  void openGalleryIfWanted();
};
