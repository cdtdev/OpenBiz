import { useEffect, useState } from "react";

/** Health report from `GET /healthz`. Mirrors `openbiz_api::Health`. */
interface Health {
  status: string;
  version: string;
}

type Probe =
  | { state: "loading" }
  | { state: "ok"; health: Health }
  | { state: "error"; message: string };

export function App() {
  const [probe, setProbe] = useState<Probe>({ state: "loading" });

  useEffect(() => {
    const controller = new AbortController();

    fetch("/healthz", { signal: controller.signal })
      .then(async (response) => {
        if (!response.ok) {
          throw new Error(`server responded ${response.status}`);
        }
        return (await response.json()) as Health;
      })
      .then((health) => setProbe({ state: "ok", health }))
      .catch((error: unknown) => {
        if (error instanceof DOMException && error.name === "AbortError") {
          return;
        }
        setProbe({
          state: "error",
          message: error instanceof Error ? error.message : "unknown error",
        });
      });

    return () => controller.abort();
  }, []);

  return (
    <main>
      <h1>OpenBiz</h1>
      <p>Centralise, author, and govern your taxonomies, ontologies, and thesauri.</p>
      {probe.state === "loading" && <p>Checking server…</p>}
      {probe.state === "ok" && <p>Server {probe.health.status} — version {probe.health.version}</p>}
      {probe.state === "error" && <p role="alert">Cannot reach server: {probe.message}</p>}
    </main>
  );
}
