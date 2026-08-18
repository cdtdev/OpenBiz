import { StrictMode } from "react";
import { act, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { App } from "./App";

/** What a real `fetch` rejects with once its `AbortSignal` fires. */
function abortError() {
  return new DOMException("The operation was aborted.", "AbortError");
}

/**
 * A `fetch` whose outcome for `/healthz` the test decides. It honours the `AbortSignal` the way the
 * real one does — rejecting with an `AbortError` the moment the signal fires, whatever the outcome
 * would have been — because a stub that ignored the signal would let the component's abort handling
 * rot untested while the suite stayed green. It records the signals handed to the *health* probe so
 * the unmount path can be asserted on.
 *
 * `App` also renders `Vocabularies`, which reads `/api/graphs` on mount. That request always
 * succeeds here, and with an empty registry: these tests are about the health probe, and a registry
 * that could also fail would put a second `role="alert"` on the page and make every assertion below
 * ambiguous. `Vocabularies` has its own suite for its own states.
 */
function stubFetch(outcome: () => Promise<Response>) {
  const signals: AbortSignal[] = [];
  const fetch = vi.fn((input: RequestInfo | URL, init?: RequestInit) => {
    const url = String(input);
    const signal = init?.signal;
    if (url === "/healthz" && signal) {
      signals.push(signal);
    }
    return new Promise<Response>((resolve, reject) => {
      if (signal?.aborted) {
        reject(abortError());
        return;
      }
      signal?.addEventListener("abort", () => reject(abortError()));
      const settle =
        url === "/healthz"
          ? outcome()
          : Promise.resolve(
              new Response(JSON.stringify({ graphs: [] }), {
                status: 200,
                headers: { "content-type": "application/json" },
              }),
            );
      settle.then(resolve, reject);
    });
  });
  vi.stubGlobal("fetch", fetch);
  return { fetch, signals };
}

/** Every URL `fetch` was asked for, in call order. */
function requested(fetch: ReturnType<typeof vi.fn>): string[] {
  return fetch.mock.calls.map((call) => String(call[0]));
}

/** A `fetch` that never answers, leaving the component in its loading state. */
function neverSettles() {
  return stubFetch(() => new Promise<Response>(() => {}));
}

/** A promise this test resolves by hand, so no assertion depends on scheduler timing. */
function deferred<T>() {
  let settle!: (value: T) => void;
  const promise = new Promise<T>((resolve) => {
    settle = resolve;
  });
  return { promise, settle };
}

/** Drain the microtask queue and let React commit whatever it produced. */
async function flush() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

function healthResponse(status: number, body: unknown): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json" },
  });
}

describe("App", () => {
  it("names the product and its purpose regardless of what the server says", () => {
    neverSettles();
    render(<App />);

    expect(screen.getByRole("heading", { name: "OpenBiz" })).toBeTruthy();
    expect(
      screen.getByText(
        "Centralise, author, and govern your taxonomies, ontologies, and thesauri.",
      ),
    ).toBeTruthy();
  });

  it("probes /healthz once on mount, and says so while it waits", () => {
    const { fetch } = neverSettles();
    render(<App />);

    expect(requested(fetch).filter((url) => url === "/healthz")).toEqual(["/healthz"]);
    expect(screen.getByText("Checking server…")).toBeTruthy();
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("reports the server's status and version when the probe succeeds", async () => {
    stubFetch(async () => healthResponse(200, { status: "ok", version: "0.1.0" }));
    render(<App />);

    expect(await screen.findByText("Server ok — version 0.1.0")).toBeTruthy();
    expect(screen.queryByText("Checking server…")).toBeNull();
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("names the status code when the server answers but refuses", async () => {
    stubFetch(async () => healthResponse(503, { status: "unavailable", version: "0.1.0" }));
    render(<App />);

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toBe("Cannot reach server: server responded 503");
    expect(screen.queryByText("Checking server…")).toBeNull();
  });

  it("does not present a failed probe's body as if it were a healthy one", async () => {
    // A 503 carrying a well-formed Health body is the trap: parsing first and checking `ok` second
    // would report "Server unavailable" as though the server had answered normally.
    stubFetch(async () => healthResponse(503, { status: "unavailable", version: "0.1.0" }));
    render(<App />);

    await screen.findByRole("alert");
    expect(screen.queryByText(/^Server unavailable/)).toBeNull();
  });

  it("surfaces a transport failure in the user's terms", async () => {
    stubFetch(() => Promise.reject(new TypeError("Failed to fetch")));
    render(<App />);

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toBe("Cannot reach server: Failed to fetch");
  });

  it("still reports something when the failure is not an Error", async () => {
    stubFetch(() => Promise.reject("no reason given"));
    render(<App />);

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toBe("Cannot reach server: unknown error");
  });

  it("aborts the in-flight probe when it unmounts", async () => {
    const { signals } = neverSettles();
    const { unmount } = render(<App />);

    expect(signals[0]?.aborted).toBe(false);
    unmount();
    expect(signals[0]?.aborted).toBe(true);
    await flush();
  });

  it("stays silent when the probe is aborted, because an abort is not a failure", async () => {
    // We aborted it, so there is nothing to tell the user. Rendering "Cannot reach server" here
    // would accuse a perfectly healthy server of being unreachable.
    stubFetch(() => Promise.reject(abortError()));
    render(<App />);
    await flush();

    expect(screen.queryByRole("alert")).toBeNull();
    expect(screen.getByText("Checking server…")).toBeTruthy();
  });

  it("survives StrictMode's double mount without a spurious error", async () => {
    // StrictMode mounts, unmounts, and remounts the effect in development, so the first probe is
    // always aborted while the second is still in flight. That window is exactly where a
    // mishandled abort becomes an alert every developer sees on every page load.
    const second = deferred<Response>();
    let call = 0;
    const { fetch } = stubFetch(() => {
      call += 1;
      return call === 1 ? new Promise<Response>(() => {}) : second.promise;
    });

    render(
      <StrictMode>
        <App />
      </StrictMode>,
    );

    await waitFor(() => {
      expect(requested(fetch).filter((url) => url === "/healthz")).toHaveLength(2);
    });
    await flush();

    expect(screen.queryByRole("alert")).toBeNull();
    expect(screen.getByText("Checking server…")).toBeTruthy();

    second.settle(healthResponse(200, { status: "ok", version: "0.1.0" }));

    expect(await screen.findByText("Server ok — version 0.1.0")).toBeTruthy();
    expect(screen.queryByRole("alert")).toBeNull();
  });
});
