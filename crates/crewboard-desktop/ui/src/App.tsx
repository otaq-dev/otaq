import { useServiceConnection } from "./service";

// The empty window. The app frame and its screens come with T-0212, the service banners with T-0231. Until then the
// window says why the service couldn't start instead of staying blank (Q40).
export function App() {
  const connection = useServiceConnection();
  return (
    <main className="app">{connection?.state === "failed" && <p role="alert">{connection.reason}</p>}</main>
  );
}
