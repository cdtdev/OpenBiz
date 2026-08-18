import { act, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { Vocabularies } from "./Vocabularies";

/** What a real `fetch` rejects with once its `AbortSignal` fires. */
function abortError() {
  return new DOMException("The operation was aborted.", "AbortError");
}

/**
 * A `fetch` whose outcome the test decides, honouring the `AbortSignal` the way the real one does.
 * A stub that ignored the signal would leave the abort path untested while the suite stayed green.
 */
function stubFetch(outcome: () => Promise<Response>) {
  const signals: AbortSignal[] = [];
  const fetch = vi.fn((_input: RequestInfo | URL, init?: RequestInit) => {
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
      outcome().then(resolve, reject);
    });
  });
  vi.stubGlobal("fetch", fetch);
  return { fetch, signals };
}

/** A `fetch` that never answers, leaving the component in its loading state. */
function neverSettles() {
  return stubFetch(() => new Promise<Response>(() => {}));
}

/** Serve a registry exactly as `GET /api/graphs` would. */
function registry(graphs: Array<{ iri: string; kind: string }>) {
  return stubFetch(
    async () =>
      new Response(JSON.stringify({ graphs }), {
        status: 200,
        headers: { "content-type": "application/json" },
      }),
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

  it("reads the registry once on mount", () => {
    const { fetch } = neverSettles();
    render(<Vocabularies />);

    expect(fetch).toHaveBeenCalledTimes(1);
    expect(fetch.mock.calls[0]?.[0]).toBe("/api/graphs");
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
      "http://example.org/v/animals",
      "http://example.org/v/plants",
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
    expect(items.map((item) => item.textContent)).toEqual(["http://example.org/v/animals"]);
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
