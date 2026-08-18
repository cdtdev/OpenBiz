import { act, fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { Vocabularies } from "./Vocabularies";

/** What a real `fetch` rejects with once its `AbortSignal` fires. */
function abortError() {
  return new DOMException("The operation was aborted.", "AbortError");
}

/**
 * A `fetch` whose outcome the test decides, honouring the `AbortSignal` the way the real one does.
 * A stub that ignored the signal would leave the abort path untested while the suite stayed green.
 *
 * `outcome` receives the URL, because the component reads two endpoints — the registry and the
 * list of serialisations it may offer. A stub that answered both with the same body would let a
 * component that fetched the wrong URL pass.
 */
function stubFetch(outcome: (url: string) => Promise<Response>) {
  const signals: AbortSignal[] = [];
  const fetch = vi.fn((input: RequestInfo | URL, init?: RequestInit) => {
    const url = String(input);
    const signal = init?.signal;
    if (signal) {
      signals.push(signal);
    }
    return new Promise<Response>((resolve, reject) => {
      if (signal?.aborted) {
        reject(abortError());
        return;
      }
      signal?.addEventListener("abort", () => reject(abortError()));
      outcome(url).then(resolve, reject);
    });
  });
  vi.stubGlobal("fetch", fetch);
  return { fetch, signals };
}

/** A `fetch` that never answers, leaving the component in its loading state. */
function neverSettles() {
  return stubFetch(() => new Promise<Response>(() => {}));
}

/** A 200 carrying `body` as JSON. */
function json(body: unknown): Response {
  return new Response(JSON.stringify(body), {
    status: 200,
    headers: { "content-type": "application/json" },
  });
}

/**
 * The serialisations the real server advertises, in the order it advertises them.
 *
 * Copied from `openbiz_store::RdfSyntax` rather than invented, so a test that renders a format
 * chooser renders the one a user would actually see. The server-side test
 * `the_advertised_formats_are_the_ones_the_store_has` is what keeps that list true.
 */
const FORMATS = [
  { token: "turtle", label: "Turtle", mediaType: "text/turtle", fileExtension: "ttl", recordsGraphNames: false },
  { token: "ntriples", label: "N-Triples", mediaType: "application/n-triples", fileExtension: "nt", recordsGraphNames: false },
  { token: "nquads", label: "N-Quads", mediaType: "application/n-quads", fileExtension: "nq", recordsGraphNames: true },
  { token: "trig", label: "TriG", mediaType: "application/trig", fileExtension: "trig", recordsGraphNames: true },
  { token: "rdfxml", label: "RDF/XML", mediaType: "application/rdf+xml", fileExtension: "rdf", recordsGraphNames: false },
  { token: "jsonld", label: "JSON-LD", mediaType: "application/ld+json", fileExtension: "jsonld", recordsGraphNames: true },
];

/** Serve a registry exactly as `GET /api/graphs` would, alongside the advertised formats. */
function registry(graphs: Array<{ iri: string; kind: string }>) {
  return stubFetch(async (url) =>
    json(url === "/api/export/formats" ? { formats: FORMATS } : { graphs }),
  );
}

/** Drain the microtask queue and let React commit whatever it produced. */
async function flush() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

const SYSTEM_GRAPH = { iri: "urn:openbiz:graph:system", kind: "system" };

describe("Vocabularies", () => {
  it("is a titled region even before the server answers", () => {
    neverSettles();
    render(<Vocabularies />);

    expect(screen.getByRole("heading", { name: "Vocabularies" })).toBeTruthy();
    expect(screen.getByText("Loading vocabularies…")).toBeTruthy();
    expect(screen.queryByRole("list")).toBeNull();
  });

  it("reads the registry and the offered formats once each, on mount", () => {
    const { fetch } = neverSettles();
    render(<Vocabularies />);

    expect(fetch.mock.calls.map((call) => String(call[0]))).toEqual([
      "/api/graphs",
      "/api/export/formats",
    ]);
  });

  it("lists each vocabulary by its IRI", async () => {
    registry([
      { iri: "http://example.org/v/animals", kind: "vocabulary" },
      { iri: "http://example.org/v/plants", kind: "vocabulary" },
      SYSTEM_GRAPH,
    ]);
    render(<Vocabularies />);

    const items = await screen.findAllByRole("listitem");
    expect(items.map((item) => item.textContent)).toEqual([
      "http://example.org/v/animals Download",
      "http://example.org/v/plants Download",
    ]);
    expect(screen.queryByRole("alert")).toBeNull();
  });

  /**
   * The differentiator, as a test. VocBench shows the triplestore's own support graphs alongside
   * the user's content, and a subject-matter expert is then asked which graph to author into.
   * The API returns the whole registry on purpose; keeping our bookkeeping out of the user's list
   * is this component's job, and if it stops doing it nothing else will notice.
   */
  it("never presents OpenBiz's own graphs as the user's vocabularies", async () => {
    registry([
      { iri: "http://example.org/v/animals", kind: "vocabulary" },
      SYSTEM_GRAPH,
      { iri: "urn:openbiz:graph:inferred:http://example.org/v/animals", kind: "inferred" },
    ]);
    render(<Vocabularies />);

    const items = await screen.findAllByRole("listitem");
    expect(items.map((item) => item.textContent)).toEqual([
      "http://example.org/v/animals Download",
    ]);
    expect(screen.queryByText(/urn:openbiz:/)).toBeNull();
  });

  /**
   * Held back, not hidden. A governance tool that silently drops rows from a list invites the
   * question of what else it has dropped, so the count is stated even though the IRIs are not.
   */
  it("says how many graphs it is holding back, and counts them correctly", async () => {
    registry([
      { iri: "http://example.org/v/animals", kind: "vocabulary" },
      SYSTEM_GRAPH,
      { iri: "urn:openbiz:graph:inferred:http://example.org/v/animals", kind: "inferred" },
    ]);
    render(<Vocabularies />);

    expect(
      await screen.findByText(
        "2 further graphs are held for OpenBiz’s own use and are not shown here.",
      ),
    ).toBeTruthy();
  });

  /**
   * A proposed change is staged in a graph of its own until somebody approves it. It is in the
   * registry — an operator asking what the store holds is entitled to the whole answer — and it
   * must not be in front of a taxonomist as a vocabulary, because the statements in it are not
   * part of anybody's vocabulary yet. That is what "not yet approved" has to mean in the UI.
   */
  it("does not present a pending change's staging graph as a vocabulary", async () => {
    registry([
      { iri: "http://example.org/v/animals", kind: "vocabulary" },
      SYSTEM_GRAPH,
      { iri: "urn:openbiz:graph:candidate:1", kind: "candidate" },
    ]);
    render(<Vocabularies />);

    const items = await screen.findAllByRole("listitem");
    expect(items.map((item) => item.textContent)).toEqual([
      "http://example.org/v/animals Download",
    ]);
    expect(
      await screen.findByText(
        "2 further graphs are held for OpenBiz’s own use and are not shown here.",
      ),
    ).toBeTruthy();
  });

  it("says it in the singular when there is one", async () => {
    registry([{ iri: "http://example.org/v/animals", kind: "vocabulary" }, SYSTEM_GRAPH]);
    render(<Vocabularies />);

    expect(
      await screen.findByText(
        "1 further graph is held for OpenBiz’s own use and is not shown here.",
      ),
    ).toBeTruthy();
  });

  it("says nothing about held-back graphs when there are none", async () => {
    registry([{ iri: "http://example.org/v/animals", kind: "vocabulary" }]);
    render(<Vocabularies />);

    await screen.findAllByRole("listitem");
    expect(screen.queryByText(/held for OpenBiz/)).toBeNull();
  });

  /**
   * A fresh store holds the system graph and nothing else, so this is what every new deployment
   * sees first. It must not be an empty list — and per `CLAUDE.md` §1.7 it is the right moment to
   * say that reuse comes before creation, rather than offering a "New vocabulary" button that
   * makes the tenth overlapping vocabulary the cheapest thing on the page.
   */
  it("explains the empty state rather than rendering an empty list", async () => {
    registry([SYSTEM_GRAPH]);
    render(<Vocabularies />);

    expect(
      await screen.findByText(
        "No vocabularies yet. Before creating one, OpenBiz will look for an existing vocabulary that already serves — reuse outranks creation.",
      ),
    ).toBeTruthy();
    expect(screen.queryByRole("list")).toBeNull();
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("reports a registry that could not be read, without pretending it is empty", async () => {
    stubFetch(
      async () =>
        new Response(JSON.stringify({ message: "the graph registry could not be read" }), {
          status: 500,
          headers: { "content-type": "application/json" },
        }),
    );
    render(<Vocabularies />);

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toBe("Cannot list vocabularies: server responded 500");
    expect(screen.queryByText(/No vocabularies yet/)).toBeNull();
    expect(screen.queryByRole("list")).toBeNull();
  });

  it("surfaces a transport failure in the user's terms", async () => {
    stubFetch(() => Promise.reject(new TypeError("Failed to fetch")));
    render(<Vocabularies />);

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toBe("Cannot list vocabularies: Failed to fetch");
  });

  /**
   * The download is a link to a plain URL, not a button that opens a wizard. That is the point:
   * whatever the interface can do here, `curl` and a scheduled job can do identically. The
   * accessible name carries the vocabulary and the format, because a page of links all reading
   * "Download" is unusable to anyone navigating by link list.
   */
  it("offers each vocabulary as a download at a plain URL", async () => {
    registry([
      { iri: "http://example.org/v/animals", kind: "vocabulary" },
      { iri: "http://example.org/v/plants", kind: "vocabulary" },
    ]);
    render(<Vocabularies />);

    const link = await screen.findByRole("link", {
      name: "Download http://example.org/v/animals as Turtle",
    });
    expect(link.getAttribute("href")).toBe(
      "/api/export?graph=http%3A%2F%2Fexample.org%2Fv%2Fanimals&format=turtle",
    );
    expect(screen.getAllByRole("link")).toHaveLength(2);
  });

  /** An IRI with a query, a hash, or a space must survive into the URL as one parameter. */
  it("escapes the IRI into the download URL rather than splicing it in", async () => {
    registry([{ iri: "http://example.org/v/a b?x=1#frag", kind: "vocabulary" }]);
    render(<Vocabularies />);

    const link = await screen.findByRole("link");
    expect(link.getAttribute("href")).toBe(
      "/api/export?graph=http%3A%2F%2Fexample.org%2Fv%2Fa%20b%3Fx%3D1%23frag&format=turtle",
    );
  });

  /**
   * The chooser renders what the *server* said it can write. Hard-coding the list here is the
   * failure this replaces: the UI and the server ship in one binary, so a divergence would never
   * be caught by a build — only by a user picking a format and being refused.
   */
  it("offers exactly the formats the server advertises, in the order it advertised them", async () => {
    registry([{ iri: "http://example.org/v/animals", kind: "vocabulary" }]);
    render(<Vocabularies />);

    const chooser = (await screen.findByLabelText("Download format")) as HTMLSelectElement;
    expect(Array.from(chooser.options).map((option) => option.value)).toEqual(
      FORMATS.map((format) => format.token),
    );
    expect(chooser.value).toBe("turtle");
  });

  it("rewrites every download link when a different format is chosen", async () => {
    registry([
      { iri: "http://example.org/v/animals", kind: "vocabulary" },
      { iri: "http://example.org/v/plants", kind: "vocabulary" },
    ]);
    render(<Vocabularies />);

    const chooser = await screen.findByLabelText("Download format");
    fireEvent.change(chooser, { target: { value: "nquads" } });

    expect(
      screen.getAllByRole("link").map((link) => link.getAttribute("href")),
    ).toEqual([
      "/api/export?graph=http%3A%2F%2Fexample.org%2Fv%2Fanimals&format=nquads",
      "/api/export?graph=http%3A%2F%2Fexample.org%2Fv%2Fplants&format=nquads",
    ]);
  });

  /**
   * The honesty the incumbents skip. Turtle cannot record which graph a statement belongs to, so a
   * Turtle export cannot say which vocabulary it is — and a user finds that out from a re-import
   * that lands in the wrong place. The warning is derived from `recordsGraphNames`, which the
   * server reads from the same constant its serialiser branches on, so it cannot say one thing
   * while the writer does another.
   */
  it("warns when the chosen format cannot say which vocabulary the file came from", async () => {
    registry([{ iri: "http://example.org/v/animals", kind: "vocabulary" }]);
    render(<Vocabularies />);

    expect(
      await screen.findByText(
        "Turtle cannot record which graph a statement belongs to, so the file will not say which vocabulary it came from. Download as N-Quads or TriG or JSON-LD to keep that.",
      ),
    ).toBeTruthy();

    fireEvent.change(screen.getByLabelText("Download format"), {
      target: { value: "nquads" },
    });
    expect(screen.queryByText(/cannot record which graph/)).toBeNull();
  });

  /**
   * `CLAUDE.md` §4.4 requires anything user-facing to be keyboard-navigable. jsdom cannot prove a
   * tab order, so what is asserted is the thing that makes one: native controls with real labels,
   * rather than a `div` with a click handler. The remaining gap is recorded in `UNTESTED.md`.
   */
  it("uses native controls, so it is reachable without a mouse", async () => {
    registry([{ iri: "http://example.org/v/animals", kind: "vocabulary" }]);
    render(<Vocabularies />);

    const chooser = await screen.findByLabelText("Download format");
    expect(chooser.tagName).toBe("SELECT");

    const link = screen.getByRole("link");
    expect(link.tagName).toBe("A");
    expect(link.getAttribute("href")).toBeTruthy();

    chooser.focus();
    expect(document.activeElement).toBe(chooser);
    (link as HTMLElement).focus();
    expect(document.activeElement).toBe(link);
  });

  /**
   * The registry and the format list fail independently. A server that can list vocabularies but
   * not describe its serialisations must still show the vocabularies — losing the list because the
   * download menu is unavailable would be a much larger failure than the one that happened.
   */
  it("still lists the vocabularies when the formats cannot be read", async () => {
    stubFetch(async (url) =>
      url === "/api/export/formats"
        ? new Response("{}", { status: 500, headers: { "content-type": "application/json" } })
        : json({ graphs: [{ iri: "http://example.org/v/animals", kind: "vocabulary" }] }),
    );
    render(<Vocabularies />);

    const items = await screen.findAllByRole("listitem");
    expect(items.map((item) => item.textContent)).toEqual(["http://example.org/v/animals"]);
    expect(screen.queryByRole("link")).toBeNull();
    expect((await screen.findByRole("alert")).textContent).toBe(
      "Cannot offer downloads: server responded 500",
    );
  });

  it("aborts the in-flight read when it unmounts", async () => {
    const { signals } = neverSettles();
    const { unmount } = render(<Vocabularies />);

    expect(signals[0]?.aborted).toBe(false);
    unmount();
    expect(signals[0]?.aborted).toBe(true);
    await flush();
  });

  it("stays silent when the read is aborted, because an abort is not a failure", async () => {
    stubFetch(() => Promise.reject(abortError()));
    render(<Vocabularies />);
    await flush();

    expect(screen.queryByRole("alert")).toBeNull();
    expect(screen.getByText("Loading vocabularies…")).toBeTruthy();
  });
});
