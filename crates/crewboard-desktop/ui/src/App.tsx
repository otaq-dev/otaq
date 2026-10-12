import { Frame } from "./app/Frame";
import { type Registries, RegistriesContext } from "./app/registry";
import { useServiceConnection } from "./service";

// The page: the frame with the screens' registries. Until the banners screen (T-0231) fills the banner area, it says
// why the service couldn't start instead of staying blank (Q40).
export function App({ registries }: { registries: Registries }) {
  const connection = useServiceConnection();
  return (
    <RegistriesContext value={registries}>
      <Frame bannerFallback={connection?.state === "failed" && <p role="alert">{connection.reason}</p>} />
    </RegistriesContext>
  );
}
