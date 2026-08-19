/**
 * WCAG 2.2 contrast, computed rather than asserted.
 *
 * `CLAUDE.md` §3 lists WCAG 2.2 AA as a conformance target and the Phase 3 design-system item asks
 * for "colour with verified contrast". Verified means *arithmetic over the values that ship*, not a
 * designer's recollection of what a checker said once. Everything here is the formula from WCAG 2.2
 * §Relative luminance and §Contrast ratio, applied to the same bytes the browser loads.
 *
 * Deliberately limited to sRGB hex. The token file writes hex and nothing else, and a parser that
 * silently accepted `oklch()` by approximating it would report a contrast we had not actually
 * computed — which is exactly the failure this module exists to prevent. An unparseable colour is
 * an error, never a default.
 */

/** A colour we could not read. Thrown rather than defaulted: see the module note. */
export class UnreadableColour extends Error {
  constructor(value: string) {
    super(`not an sRGB hex colour: ${JSON.stringify(value)}`);
    this.name = "UnreadableColour";
  }
}

/** The three sRGB channels, 0–255. */
export interface Rgb {
  r: number;
  g: number;
  b: number;
}

/**
 * Parse `#rgb`, `#rrggbb`, or `#rrggbbaa`.
 *
 * The alpha form is parsed and its alpha **rejected**: a translucent foreground has no single
 * contrast ratio — it depends on whatever happens to be behind it — so accepting one would let a
 * token pass a check that means nothing. Opaque `#rrggbbff` is fine and is treated as `#rrggbb`.
 */
export function parseHex(value: string): Rgb {
  const digits = /^#([0-9a-fA-F]{3}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})$/.exec(value.trim())?.[1];
  if (digits === undefined) {
    throw new UnreadableColour(value);
  }

  // `#rgb` is shorthand for `#rrggbb`. Expanding it here leaves one code path for everything after.
  const full = digits.length === 3 ? [...digits].map((digit) => digit + digit).join("") : digits;
  const octet = (index: number) => Number.parseInt(full.slice(index * 2, index * 2 + 2), 16);

  if (full.length === 8 && octet(3) !== 0xff) {
    throw new Error(
      `translucent colours have no fixed contrast ratio, so ${value} cannot be verified`,
    );
  }
  return { r: octet(0), g: octet(1), b: octet(2) };
}

/** One channel, linearised out of sRGB's transfer function. WCAG 2.2, §Relative luminance. */
function linearise(channel: number): number {
  const c = channel / 255;
  return c <= 0.04045 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4);
}

/** Relative luminance, 0 for black and 1 for white. */
export function relativeLuminance(colour: Rgb): number {
  return (
    0.2126 * linearise(colour.r) + 0.7152 * linearise(colour.g) + 0.0722 * linearise(colour.b)
  );
}

/**
 * The contrast ratio between two colours, from 1 (identical) to 21 (black on white).
 *
 * Symmetric by construction — the lighter of the two goes on top whichever order they arrive in —
 * because a caller that had to know which was the background would eventually get it backwards and
 * read a ratio below 1.
 */
export function contrastRatio(a: Rgb, b: Rgb): number {
  const one = relativeLuminance(a);
  const other = relativeLuminance(b);
  return (Math.max(one, other) + 0.05) / (Math.min(one, other) + 0.05);
}

/** Contrast between two hex strings, for callers holding token values rather than channels. */
export function hexContrast(a: string, b: string): number {
  return contrastRatio(parseHex(a), parseHex(b));
}

/**
 * The thresholds this design system holds itself to, named so a failure message can say which rule
 * was broken rather than only which number was missed.
 */
export const THRESHOLD = {
  /** SC 1.4.3, normal-size text. */
  text: 4.5,
  /** SC 1.4.3, text at 18.66px bold or 24px regular and above. */
  largeText: 3,
  /** SC 1.4.11, user-interface components and graphical objects — borders, focus rings. */
  nonText: 3,
} as const;
