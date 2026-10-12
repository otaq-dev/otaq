import { useEffect } from "react";

import { Frame } from "./app/Frame";
import { KeyLog } from "./app/KeyLog";
import { installKeys } from "./app/keyboard";
import { type Registries, RegistriesContext } from "./app/registry";
import { useServiceConnection } from "./service";

// The page: the frame with the screens' registries, and the key dispatcher that runs their shortcut handlers. Until
// the banners screen (T-0231) fills the banner area, it says why the service couldn't start instead of staying blank
// (Q40).
export function App({ registries }: { registries: Registries }) {
  const connection = useServiceConnection();
  useEffect(() => installKeys(registries), [registries]);
  return (
    <RegistriesContext value={registries}>
      <Frame bannerFallback={connection?.state === "failed" && <p role="alert">{connection.reason}</p>} />
      <KeyLog />
    </RegistriesContext>
  );
}
