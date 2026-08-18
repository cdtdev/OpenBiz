import { Vocabularies } from "./Vocabularies";
import { useProbe } from "./useProbe";

/** Health report from `GET /healthz`. Mirrors `openbiz_api::Health`. */
interface Health {
  status: string;
  version: string;
}

export function App() {
  const probe = useProbe<Health>("/healthz");

  return (
    <main>
      <h1>OpenBiz</h1>
      <p>Centralise, author, and govern your taxonomies, ontologies, and thesauri.</p>
      {probe.state === "loading" && <p>Checking server…</p>}
      {probe.state === "ok" && <p>Server {probe.data.status} — version {probe.data.version}</p>}
      {probe.state === "error" && <p role="alert">Cannot reach server: {probe.message}</p>}
      <Vocabularies />
    </main>
  );
}
