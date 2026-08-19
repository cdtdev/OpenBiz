import { describe, expect, it } from "vitest";
import {
  contrastRatio,
  hexContrast,
  parseHex,
  relativeLuminance,
  THRESHOLD,
  UnreadableColour,
} from "./contrast";

/**
 * The maths, checked against values WCAG states rather than against itself.
 *
 * Every colour assertion in `tokens.test.ts` is only as good as this file: a luminance formula with
 * a transposed coefficient would still produce plausible numbers, still order colours roughly
 * correctly, and still let a design system claim AA it does not meet.
 */
describe("relative luminance", () => {
  it("is 0 for black and 1 for white, the two values WCAG fixes by definition", () => {
    expect(relativeLuminance(parseHex("#000000"))).toBe(0);
    expect(relativeLuminance(parseHex("#ffffff"))).toBeCloseTo(1, 10);
  });

  it("weights green far above red and red above blue", () => {
    const red = relativeLuminance(parseHex("#ff0000"));
    const green = relativeLuminance(parseHex("#00ff00"));
    const blue = relativeLuminance(parseHex("#0000ff"));

    // The coefficients themselves — 0.2126 / 0.7152 / 0.0722 — since a primary at full intensity
    // linearises to exactly 1 and so returns its own weight.
    expect(red).toBeCloseTo(0.2126, 10);
    expect(green).toBeCloseTo(0.7152, 10);
    expect(blue).toBeCloseTo(0.0722, 10);
  });

  it("uses the linear segment below the sRGB knee, not the power curve", () => {
    // 10/255 ≈ 0.0392 is under 0.04045, so all three channels take the `c / 12.92` branch.
    const straight = (10 / 255 / 12.92) * (0.2126 + 0.7152 + 0.0722);
    expect(relativeLuminance(parseHex("#0a0a0a"))).toBeCloseTo(straight, 12);
  });
});

describe("contrast ratio", () => {
  it("is 21:1 for black on white — the maximum the formula can produce", () => {
    expect(hexContrast("#000000", "#ffffff")).toBeCloseTo(21, 10);
  });

  it("is 1:1 for a colour against itself", () => {
    expect(hexContrast("#4050c4", "#4050c4")).toBeCloseTo(1, 10);
  });

  it("does not depend on which colour is called the background", () => {
    expect(hexContrast("#ffffff", "#767676")).toBeCloseTo(hexContrast("#767676", "#ffffff"), 12);
  });

  it("agrees with the worked example WCAG uses for its own threshold", () => {
    // #767676 on white is the canonical "just passes 4.5:1" grey quoted throughout the WCAG
    // understanding documents. One step lighter must fail, or the check has no edge.
    expect(hexContrast("#767676", "#ffffff")).toBeGreaterThanOrEqual(THRESHOLD.text);
    expect(hexContrast("#777777", "#ffffff")).toBeLessThan(THRESHOLD.text);
  });

  it("takes channels, not only hex, so a caller can compute without a round trip", () => {
    expect(contrastRatio({ r: 0, g: 0, b: 0 }, { r: 255, g: 255, b: 255 })).toBeCloseTo(21, 10);
  });
});

describe("reading a colour", () => {
  it("expands the three-digit form the way CSS does", () => {
    expect(parseHex("#abc")).toEqual(parseHex("#aabbcc"));
  });

  it("accepts an opaque eight-digit colour as its six-digit equivalent", () => {
    expect(parseHex("#4050c4ff")).toEqual(parseHex("#4050c4"));
  });

  it("is case-insensitive and tolerates surrounding whitespace", () => {
    expect(parseHex("  #4050C4 ")).toEqual(parseHex("#4050c4"));
  });

  it.each(["", "#", "4050c4", "#4050c", "#gggggg", "rgb(0 0 0)", "oklch(0.5 0.1 250)"])(
    "refuses %o rather than guessing at it",
    (value) => {
      expect(() => parseHex(value)).toThrow(UnreadableColour);
    },
  );

  it("refuses a translucent colour, which has no fixed contrast ratio at all", () => {
    // The trap this closes: `#00000080` on white "passes" if the alpha is dropped, and the text it
    // describes is in fact half as dark as the number claims.
    expect(() => parseHex("#00000080")).toThrow(/translucent/);
  });
});
