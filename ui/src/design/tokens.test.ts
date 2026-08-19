import { readdirSync, readFileSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { hexContrast, parseHex, relativeLuminance, THRESHOLD } from "./contrast";
import {
  pairings,
  readThemes,
  resolve,
  stripComments,
  UnreadableTokens,
  type Declarations,
} from "./tokens";

const HERE = dirname(fileURLToPath(import.meta.url));
const SRC = join(HERE, "..");
const TOKENS = join(HERE, "tokens.css");

/**
 * The token file, read off disk.
 *
 * Not `import "./tokens.css?raw"`, which is the obvious way and returns the empty string: Vitest
 * stubs CSS imports out, so every colour assertion below would have run over no tokens at all.
 * That is exactly the vacuous green this suite exists to prevent, and it is worth one dev-only
 * dependency (`@types/node`) to read the bytes that ship.
 */
const css = readFileSync(TOKENS, "utf8");
const themes = readThemes(css);

/** Every `.css` file under `src`, so a second stylesheet cannot quietly start its own palette. */
function stylesheets(dir: string): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) return stylesheets(path);
    return entry.isFile() && entry.name.endsWith(".css") ? [path] : [];
  });
}

/** The application's stylesheets — everything the tokens are meant to be consumed by. */
function rules(): [string, string][] {
  return stylesheets(SRC)
    .filter((path) => path !== TOKENS)
    .map((path) => [relative(SRC, path), readFileSync(path, "utf8")]);
}

/** Both themes, so every rule below is asserted twice rather than on whichever one is default. */
const BOTH: [string, Declarations][] = [
  ["light", themes.light],
  ["dark", themes.dark],
];

/** The token names, in the order the file declares them. */
function named(tokens: Declarations, prefix: string): string[] {
  return [...tokens.keys()].filter((name) => name.startsWith(prefix));
}

/**
 * Each item paired with the one before it.
 *
 * Every scale rule below is about neighbours, and index arithmetic under
 * `noUncheckedIndexedAccess` buries that in `?.` noise where an off-by-one hides well.
 */
function neighbours<T>(items: readonly T[]): [T, T][] {
  return items.slice(1).map((item, at) => [items[at] as T, item]);
}

/** A `rem` length as a number of rems. Anything else is a failure, not a zero. */
function rems(value: string): number {
  const match = /^(-?[\d.]+)rem$/.exec(value);
  expect(match, `${value} is not a rem length`).not.toBeNull();
  return Number(match?.[1]);
}

/**
 * Colour, checked as arithmetic over the shipped stylesheet.
 *
 * This is the item's actual claim. "Verified contrast" is worth nothing if the verification is a
 * screenshot someone looked at once, and worth a great deal if a palette tweak that breaks AA
 * cannot reach `main`.
 */
describe.each(BOTH)("%s theme colour", (_theme, tokens) => {
  const pairs = pairings(tokens);

  it("pairs every foreground with the background its name names", () => {
    // Guards against the whole suite passing vacuously: a reader that found nothing, or a rename
    // that quietly took every token out of the naming convention, both land here.
    expect(pairs.length).toBeGreaterThanOrEqual(16);
    for (const pair of pairs) {
      expect(tokens.has(pair.background), `${pair.foreground} names a missing background`).toBe(
        true,
      );
    }
  });

  it("meets WCAG 2.2 AA for every one of them", () => {
    const failures: string[] = [];
    for (const pair of pairs) {
      const required = pair.kind === "color" ? THRESHOLD.text : THRESHOLD.nonText;
      const ratio = hexContrast(resolve(tokens, pair.foreground), resolve(tokens, pair.background));
      if (ratio < required) {
        failures.push(
          `${pair.foreground} on ${pair.background}: ${ratio.toFixed(2)}:1, needs ${required}:1`,
        );
      }
    }
    // Collected rather than asserted one at a time, so a palette change that breaks four pairings
    // reports four and not the first.
    expect(failures).toEqual([]);
  });

  it("gives every background at least one legible foreground", () => {
    const backgrounds = named(tokens, "--color-").filter((name) => !name.includes("-on-"));
    expect(backgrounds.length).toBeGreaterThanOrEqual(5);
    for (const background of backgrounds) {
      const readable = pairs.some(
        (pair) => pair.background === background && pair.kind === "color",
      );
      expect(readable, `${background} is a surface nothing is legible on`).toBe(true);
    }
  });

  it("keeps literal colours in the palette layer and nowhere else", () => {
    for (const [name, value] of tokens) {
      if (name.startsWith("--palette-")) {
        expect(() => parseHex(value), `${name} is a palette entry and must be hex`).not.toThrow();
      } else {
        expect(value, `${name} writes a literal colour instead of naming a palette entry`).not.toMatch(
          /#[0-9a-fA-F]{3}/,
        );
      }
    }
  });
});

describe("the dark theme", () => {
  it("re-points every `--color-` role rather than inheriting one by accident", () => {
    // A role left at its light value is the failure mode of a layered theme. For text it cannot
    // survive: no colour clears 4.5:1 against both a near-white and a near-black canvas, so the
    // contrast check would already have caught it. This is the assertion for the case arithmetic
    // cannot reach — a background silently unchanged between themes.
    //
    // Borders and focus rings are deliberately *not* in scope. 3:1 against both canvases is
    // achievable, and the status borders below do share one value across the two themes on
    // purpose. Demanding they differ would mean inventing a second near-identical swatch for a
    // rule nobody could state.
    const roles = [...themes.light.keys()].filter((name) => name.startsWith("--color-"));
    expect(roles.length).toBeGreaterThanOrEqual(14);
    const inherited = roles.filter((name) => themes.dark.get(name) === themes.light.get(name));
    expect(inherited).toEqual([]);
  });

  it("keeps the palette itself fixed — only the roles move", () => {
    for (const name of named(themes.light, "--palette-")) {
      expect(themes.dark.get(name), `${name} changes between themes`).toBe(themes.light.get(name));
    }
  });

  it("declares `color-scheme` so form controls and scrollbars follow the page", () => {
    expect(css).toMatch(/:root\s*\{[^}]*color-scheme:\s*light/);
    expect(css).toMatch(/prefers-color-scheme:\s*dark\)\s*\{\s*:root\s*\{[^}]*color-scheme:\s*dark/);
  });
});

describe("the type scale", () => {
  const steps = named(themes.light, "--text-").map((name) => ({
    name,
    size: rems(themes.light.get(name) ?? ""),
  }));

  it("has the steps the interface needs", () => {
    expect(steps.length).toBeGreaterThanOrEqual(7);
  });

  it("is declared in ascending order, so the file reads as the scale it is", () => {
    for (const [previous, step] of neighbours(steps)) {
      expect(
        step.size,
        `${step.name} is declared after ${previous.name} but is not larger`,
      ).toBeGreaterThan(previous.size);
    }
  });

  it("lands every step on a whole pixel at a 16px root", () => {
    // Half-pixel type is the difference between crisp and slightly smeared, and it is the kind of
    // thing that is invisible in review and obvious on a screen all day.
    for (const step of steps) {
      expect(step.size * 16, `${step.name} is ${step.size * 16}px`).toBe(
        Math.round(step.size * 16),
      );
    }
  });

  it("keeps every step within a ratio that reads as one scale", () => {
    for (const [previous, step] of neighbours(steps)) {
      const ratio = step.size / previous.size;
      const where = `${previous.name} → ${step.name} jumps ${ratio.toFixed(3)}×`;
      expect(ratio, where).toBeGreaterThanOrEqual(1.1);
      expect(ratio, where).toBeLessThanOrEqual(1.35);
    }
  });

  it("caps prose at a measure a reader can return from", () => {
    expect(themes.light.get("--measure")).toMatch(/^\d+ch$/);
  });
});

describe("the spacing scale", () => {
  const steps = named(themes.light, "--space-").map((name) => ({
    name,
    size: rems(themes.light.get(name) ?? ""),
  }));

  it("is declared in ascending order and never repeats a value", () => {
    expect(steps.length).toBeGreaterThanOrEqual(8);
    expect(steps[0]?.size).toBeGreaterThan(0);
    for (const [previous, step] of neighbours(steps)) {
      expect(step.size, `${step.name} is not larger than ${previous.name}`).toBeGreaterThan(
        previous.size,
      );
    }
  });

  it("sits on a 4px grid, which is what makes unrelated components line up", () => {
    for (const step of steps) {
      const px = step.size * 16;
      expect(px % 4, `${step.name} is ${px}px, off the grid`).toBe(0);
    }
  });
});

describe("the palette", () => {
  /** `--palette-<family>-<step>` grouped by family, in declared order. */
  interface Swatch {
    name: string;
    step: number;
    hex: string;
  }
  const families = new Map<string, Swatch[]>();
  for (const name of named(themes.light, "--palette-")) {
    const match = /^--palette-([a-z]+)-(\d+)$/.exec(name);
    const [, family, step] = match ?? [];
    expect(family, `${name} is not --palette-<family>-<step>`).toBeTypeOf("string");
    if (family === undefined || step === undefined) continue;
    const swatches = families.get(family) ?? [];
    swatches.push({ name, step: Number(step), hex: themes.light.get(name) ?? "" });
    families.set(family, swatches);
  }

  it("names a step that gets darker as the number gets larger, in every family", () => {
    // The ramp's numbering is the only thing telling a reader what `-400` means. A swatch nudged
    // past its neighbour makes every later choice a guess, and nothing else would report it.
    for (const [family, steps] of families) {
      const ordered = [...steps].sort((a, b) => a.step - b.step);
      expect(ordered.map((entry) => entry.name), `${family} is declared out of order`).toEqual(
        steps.map((entry) => entry.name),
      );
      for (const [lighter, darker] of neighbours(ordered)) {
        expect(
          relativeLuminance(parseHex(darker.hex)),
          `${darker.name} is not darker than ${lighter.name}`,
        ).toBeLessThan(relativeLuminance(parseHex(lighter.hex)));
      }
    }
  });

  it("carries no swatch that no role names", () => {
    // `CLAUDE.md` §4: a value nothing invokes is not groundwork, it is an entry in UNTESTED.md.
    // A palette is where that rots invisibly — nobody deletes a colour.
    const referenced = new Set<string>();
    for (const tokens of [themes.light, themes.dark]) {
      for (const [name, value] of tokens) {
        if (name.startsWith("--palette-")) continue;
        for (const [, swatch] of value.matchAll(/var\(\s*(--palette-[\w-]+)\s*\)/g)) {
          if (swatch !== undefined) referenced.add(swatch);
        }
      }
    }
    const orphans = named(themes.light, "--palette-").filter((name) => !referenced.has(name));
    expect(orphans).toEqual([]);
  });
});

/**
 * Block comments *and* `//` line comments removed. Naive about `//` inside a string literal, which
 * is safe for the one entry point this is used on and would not be for source generally.
 */
function uncommented(source: string): string {
  return stripComments(source).replace(/^\s*\/\/.*$/gm, "");
}

describe("the system as a whole", () => {
  it("is the only place a literal colour appears", () => {
    // The rule that turns a token file into a design system. Without it the tokens are a
    // suggestion, and the first component in a hurry writes its own grey.
    // No `\b` after the digits: `#666666` ends on a word character, so a word-boundary anchor
    // matches the three-digit form and silently misses every six-digit one. That mistake was in
    // this line, and a mutation put it there deliberately to find out.
    const offenders = rules()
      .filter(([, source]) => /#[0-9a-fA-F]{3}/.test(stripComments(source)))
      .map(([path]) => path);
    expect(offenders).toEqual([]);
  });

  it("has a rule for every colour role, so none of them is decoration", () => {
    // The mechanical form of `CLAUDE.md` §4's production-caller clause. A role with no rule is a
    // colour nobody can see, and it is exactly what accumulates when a token file is written ahead
    // of the components that need it.
    const stylesheet = stripComments(rules().map(([, source]) => source).join("\n"));
    expect(rules().length).toBeGreaterThan(0);
    const unused = [...themes.light.keys()]
      .filter((name) => name.startsWith("--color-") || /^--(border|focus)-on-/.test(name))
      .filter((name) => !stylesheet.includes(`var(${name})`));
    expect(unused).toEqual([]);
  });

  it("is imported by the application, not merely present in the tree", () => {
    // `CLAUDE.md` §4: a token file nothing loads is an entry in UNTESTED.md, not a design system.
    // Comments are stripped first, or commenting the import out would leave this passing — which
    // is what a mutation proved before this line stripped anything.
    expect(uncommented(readFileSync(join(SRC, "main.tsx"), "utf8"))).toMatch(
      /import\s+["'].*app\.css["']/,
    );
    const app = rules().find(([path]) => path.endsWith("app.css"));
    expect(app, "src/app.css is not in the tree").toBeDefined();
    expect(stripComments(app?.[1] ?? "")).toMatch(/@import\s+["']\.\/design\/tokens\.css["']/);
  });
});

/**
 * The reader's own failure paths.
 *
 * It is the thing standing between a broken token file and a green suite, so the ways it is
 * *supposed* to refuse matter more than the way it succeeds.
 */
describe("reading the token file", () => {
  const minimal = `:root { --a: #000; }
@media (prefers-color-scheme: dark) { :root { --a: #fff; } }`;

  it("refuses a file with no dark block rather than reporting one theme twice", () => {
    expect(() => readThemes(":root { --a: #000; }")).toThrow(UnreadableTokens);
  });

  it("refuses a dark block that introduces a token light never declared", () => {
    expect(() =>
      readThemes(`:root { --a: #000; }
@media (prefers-color-scheme: dark) { :root { --a: #fff; --b: #111; } }`),
    ).toThrow(/only in dark/);
  });

  it("refuses the same token declared twice in one block", () => {
    expect(() =>
      readThemes(`:root { --a: #000; --a: #111; }
@media (prefers-color-scheme: dark) { :root { --a: #fff; } }`),
    ).toThrow(/twice/);
  });

  it("ignores a commented-out declaration instead of reading it as live", () => {
    const { light } = readThemes(`:root { --a: #000; /* --b: #999; */ }
@media (prefers-color-scheme: dark) { :root { --a: #fff; } }`);
    expect(light.has("--b")).toBe(false);
  });

  it("brace-matches, so the dark block's own braces do not end it early", () => {
    const { dark } = readThemes(minimal);
    expect(dark.get("--a")).toBe("#fff");
  });

  it("follows a var() chain to the literal underneath", () => {
    const { light } = readThemes(`:root { --p: #123456; --mid: var(--p); --top: var(--mid); }
@media (prefers-color-scheme: dark) { :root { --p: #000000; } }`);
    expect(resolve(light, "--top")).toBe("#123456");
  });

  it("refuses a var() fallback, which is a token that might not exist", () => {
    const { light } = readThemes(`:root { --a: var(--missing, #f00); }
@media (prefers-color-scheme: dark) { :root { --a: #fff; } }`);
    expect(() => resolve(light, "--a")).toThrow(/falls back/);
  });

  it("names the cycle rather than hanging on it", () => {
    const { light } = readThemes(`:root { --a: var(--b); --b: var(--a); }
@media (prefers-color-scheme: dark) { :root { --a: #fff; } }`);
    expect(() => resolve(light, "--a")).toThrow(/cycle/);
  });

  it("refuses a reference to a token nobody declared", () => {
    const { light } = readThemes(`:root { --a: var(--nope); }
@media (prefers-color-scheme: dark) { :root { --a: #fff; } }`);
    expect(() => resolve(light, "--a")).toThrow(/not declared/);
  });

  it("derives a pairing from the name alone, qualifier or not", () => {
    const { light } = readThemes(`:root {
      --color-surface: #fff;
      --color-on-surface: #000;
      --color-muted-on-surface: #555;
      --border-on-surface: #888;
      --focus-width: 2px;
    }
@media (prefers-color-scheme: dark) { :root { --color-surface: #000; } }`);
    expect(pairings(light)).toEqual([
      { foreground: "--color-on-surface", background: "--color-surface", kind: "color" },
      { foreground: "--color-muted-on-surface", background: "--color-surface", kind: "color" },
      { foreground: "--border-on-surface", background: "--color-surface", kind: "border" },
    ]);
  });
});
