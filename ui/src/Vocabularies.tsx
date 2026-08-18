import { useProbe } from "./useProbe";

/** How a named graph is used. Mirrors `openbiz_api::GraphKind`. */
type GraphKind = "vocabulary" | "system" | "inferred";

/** One entry in the graph registry. Mirrors `openbiz_api::GraphSummary`. */
interface GraphSummary {
  iri: string;
  kind: GraphKind;
}

/** The registry. Mirrors `openbiz_api::GraphList`. */
interface GraphList {
  graphs: GraphSummary[];
}

/**
 * The vocabularies this OpenBiz holds.
 *
 * `GET /api/graphs` returns the whole registry — the store's own account of what it contains,
 * including OpenBiz's bookkeeping. **This is the layer that decides what a taxonomist sees**, and
 * the answer is: their vocabularies, and nothing else. VocBench puts the triplestore's support
 * graphs in the same list as the user's content, and the result is that "which graph does this go
 * in?" becomes a question a subject-matter expert is asked and cannot answer.
 *
 * Filtering here rather than in the API keeps both halves honest: the endpoint never lies about
 * what the store holds, and the interface never presents our metadata as the user's work. The
 * graphs we keep back are *counted*, not hidden — a governance tool that quietly omits rows is
 * asking to be disbelieved about the rows it does show.
 *
 * A kind outside the three is not handled, and that is a decision rather than an oversight: the UI
 * and the server ship in one binary (`CLAUDE.md` §1.2), so they are always the same build, and the
 * server refuses a registry kind it does not recognise before it ever serialises one.
 */
export function Vocabularies() {
  const probe = useProbe<GraphList>("/api/graphs");

  if (probe.state === "loading") {
    return (
      <section aria-labelledby="vocabularies-heading">
        <h2 id="vocabularies-heading">Vocabularies</h2>
        <p>Loading vocabularies…</p>
      </section>
    );
  }

  if (probe.state === "error") {
    return (
      <section aria-labelledby="vocabularies-heading">
        <h2 id="vocabularies-heading">Vocabularies</h2>
        <p role="alert">Cannot list vocabularies: {probe.message}</p>
      </section>
    );
  }

  const vocabularies = probe.data.graphs.filter((graph) => graph.kind === "vocabulary");
  const internal = probe.data.graphs.length - vocabularies.length;

  return (
    <section aria-labelledby="vocabularies-heading">
      <h2 id="vocabularies-heading">Vocabularies</h2>
      {vocabularies.length === 0 ? (
        <p>
          No vocabularies yet. Before creating one, OpenBiz will look for an existing vocabulary
          that already serves — reuse outranks creation.
        </p>
      ) : (
        <ul>
          {vocabularies.map((graph) => (
            <li key={graph.iri}>{graph.iri}</li>
          ))}
        </ul>
      )}
      {internal > 0 && (
        <p>
          {internal} further {internal === 1 ? "graph is" : "graphs are"} held for OpenBiz&rsquo;s
          own use and {internal === 1 ? "is" : "are"} not shown here.
        </p>
      )}
    </section>
  );
}
