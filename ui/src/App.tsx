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
    <main className="page">
      <header>
        <h1 className="page__title">OpenBiz</h1>
        <p className="page__lede">
          Centralise, author, and govern your taxonomies, ontologies, and thesauri.
        </p>
      </header>
      {probe.state === "loading" && <p className="status status--pending">Checking server…</p>}
      {probe.state === "ok" && (
        <p className="status status--ok">
          Server {probe.data.status} — version {probe.data.version}
        </p>
      )}
      {probe.state === "error" && (
        <p className="status status--error" role="alert">
          Cannot reach server: {probe.message}
        </p>
      )}
      <Vocabularies />
    </main>
  );
}
