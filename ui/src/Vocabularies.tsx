import { useState } from "react";
import { useProbe } from "./useProbe";

/** How a named graph is used. Mirrors `openbiz_api::GraphKind`. */
type GraphKind = "vocabulary" | "system" | "inferred" | "candidate";

/** One entry in the graph registry. Mirrors `openbiz_api::GraphSummary`. */
interface GraphSummary {
  iri: string;
  kind: GraphKind;
}

/** The registry. Mirrors `openbiz_api::GraphList`. */
interface GraphList {
  graphs: GraphSummary[];
}

/** One serialisation the server can write. Mirrors `openbiz_api::ExportFormat`. */
interface ExportFormat {
  token: string;
  label: string;
  mediaType: string;
  fileExtension: string;
  recordsGraphNames: boolean;
}

/** Every serialisation the server can write. Mirrors `openbiz_api::ExportFormats`. */
interface ExportFormats {
  formats: ExportFormat[];
}

/**
 * Where a graph can be downloaded from.
 *
 * A plain URL, so the thing the interface does is the thing a script can do. Exporting from the
 * incumbents means a wizard or a job to come back for, which is why their exports cannot be put in
 * a runbook or diffed in CI.
 */
function exportUrl(iri: string, token: string): string {
  return `/api/export?graph=${encodeURIComponent(iri)}&format=${encodeURIComponent(token)}`;
}

/** The formats that keep the graph name, listed for a warning that names the alternatives. */
function lossless(formats: ExportFormat[]): string {
  return formats
    .filter((format) => format.recordsGraphNames)
    .map((format) => format.label)
    .join(" or ");
}

/**
 * The vocabularies this OpenBiz holds, and a way to get one out of it.
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
 * The format list is **read from the server** rather than written here. The UI and the server ship
 * in one binary (`CLAUDE.md` §1.2), so an interface offering a format the serialiser does not have
 * would not be caught by a type check or a deployment — only by a user picking it. The same
 * response carries `recordsGraphNames`, so the warning below is derived from the constant the
 * writer actually branches on rather than from a second copy of the same knowledge.
 *
 * A kind outside the three is not handled, and that is a decision rather than an oversight: the UI
 * and the server ship in one binary, so they are always the same build, and the server refuses a
 * registry kind it does not recognise before it ever serialises one.
 */
export function Vocabularies() {
  const registry = useProbe<GraphList>("/api/graphs");
  const formats = useProbe<ExportFormats>("/api/export/formats");
  const [chosen, setChosen] = useState<string | null>(null);

  const available = formats.state === "ok" ? formats.data.formats : [];
  // The server orders the list most-readable first, so its first entry is the default a caller
  // gets from `/api/export` with no `?format=`. Following it keeps the two agreeing.
  const selected = available.find((format) => format.token === chosen) ?? available[0];

  if (registry.state === "loading") {
    return (
      <section aria-labelledby="vocabularies-heading">
        <h2 id="vocabularies-heading">Vocabularies</h2>
        <p>Loading vocabularies…</p>
      </section>
    );
  }

  if (registry.state === "error") {
    return (
      <section aria-labelledby="vocabularies-heading">
        <h2 id="vocabularies-heading">Vocabularies</h2>
        <p role="alert">Cannot list vocabularies: {registry.message}</p>
      </section>
    );
  }

  const vocabularies = registry.data.graphs.filter((graph) => graph.kind === "vocabulary");
  const internal = registry.data.graphs.length - vocabularies.length;

  return (
    <section aria-labelledby="vocabularies-heading">
      <h2 id="vocabularies-heading">Vocabularies</h2>
      {vocabularies.length === 0 ? (
        <p>
          No vocabularies yet. Before creating one, OpenBiz will look for an existing vocabulary
          that already serves — reuse outranks creation.
        </p>
      ) : (
        <>
          {selected ? (
            <p>
              <label htmlFor="export-format">Download format</label>{" "}
              <select
                id="export-format"
                value={selected.token}
                onChange={(event) => setChosen(event.target.value)}
              >
                {available.map((format) => (
                  <option key={format.token} value={format.token}>
                    {format.label}
                  </option>
                ))}
              </select>
            </p>
          ) : (
            formats.state === "error" && (
              <p role="alert">Cannot offer downloads: {formats.message}</p>
            )
          )}
          {selected && !selected.recordsGraphNames && (
            <p>
              {selected.label} cannot record which graph a statement belongs to, so the file will
              not say which vocabulary it came from. Download as {lossless(available)} to keep that.
            </p>
          )}
          <ul>
            {vocabularies.map((graph) => (
              <li key={graph.iri}>
                {graph.iri}
                {selected && (
                  <>
                    {" "}
                    <a
                      href={exportUrl(graph.iri, selected.token)}
                      aria-label={`Download ${graph.iri} as ${selected.label}`}
                    >
                      Download
                    </a>
                  </>
                )}
              </li>
            ))}
          </ul>
        </>
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
