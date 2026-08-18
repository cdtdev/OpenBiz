import { useEffect, useState } from "react";

/**
 * The three states any read from the API can be in.
 *
 * A discriminated union rather than the `{ data, loading, error }` triple, because that triple
 * admits states that cannot happen — loading *and* errored, data *and* errored — and every one of
 * them eventually gets rendered by somebody.
 */
export type Probe<T> =
  | { state: "loading" }
  | { state: "ok"; data: T }
  | { state: "error"; message: string };

/**
 * Read JSON from `url` once, on mount.
 *
 * Shared rather than written out at each call site. The interesting part is not the `fetch`, it is
 * the three things around it that are easy to get wrong and invisible when wrong:
 *
 * - a non-2xx response is a failure even when it carries a well-formed body, so the status is
 *   checked *before* the body is parsed;
 * - the request is aborted on unmount, so a component that has gone away cannot set state;
 * - an abort is silently ignored, because we caused it. Reporting it would accuse a healthy server
 *   of being unreachable — and under React's StrictMode the first probe is always aborted, so that
 *   mistake would show on every page load in development.
 */
export function useProbe<T>(url: string): Probe<T> {
  const [probe, setProbe] = useState<Probe<T>>({ state: "loading" });

  useEffect(() => {
    const controller = new AbortController();

    fetch(url, { signal: controller.signal })
      .then(async (response) => {
        if (!response.ok) {
          throw new Error(`server responded ${response.status}`);
        }
        return (await response.json()) as T;
      })
      .then((data) => setProbe({ state: "ok", data }))
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
  }, [url]);

  return probe;
}
