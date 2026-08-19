/**
 * Reading the design tokens back out of the stylesheet that actually ships.
 *
 * The alternative — declaring the tokens in TypeScript and generating the CSS — would let the test
 * verify a source of truth the browser never sees. Two artefacts drift, and the one that drifts
 * silently is always the generated one. So `tokens.css` is the source of truth, and this module is
 * a small reader over the same bytes Vite bundles, used by `tokens.test.ts` to prove things about
 * them. Nothing here runs in the browser; the browser has a CSS parser already.
 *
 * The reader is deliberately narrow. It understands custom-property declarations, `:root`, and the
 * one `@media (prefers-color-scheme: dark)` block the token file uses, and it refuses anything it
 * does not understand rather than skipping it — a parser that silently finds nothing turns every
 * assertion downstream into a test that passes because it checked no tokens at all.
 */

/** A theme's custom properties, as written — values may still contain `var()`. */
export type Declarations = ReadonlyMap<string, string>;

/** The two colour schemes the token file defines. */
export interface Themes {
  light: Declarations;
  dark: Declarations;
}

/** The token file is malformed, or is not the shape this reader was written against. */
export class UnreadableTokens extends Error {
  constructor(message: string) {
    super(message);
    this.name = "UnreadableTokens";
  }
}

/**
 * Drop `/* … *\/` comments so a commented-out declaration is never read as a live one.
 *
 * Exported because the "no literal colour outside the palette" check needs the same view of a
 * stylesheet: a hex quoted in a comment explaining the rule is not a colour the browser ever draws,
 * and a check that flagged it would train people to stop writing the explanation.
 */
export function stripComments(css: string): string {
  return css.replace(/\/\*[\s\S]*?\*\//g, "");
}

/**
 * The `{ … }` block whose opening brace is at or after `from`, brace-matched.
 *
 * Returns the block's inner text and the index just past its closing brace. Brace matching rather
 * than a lazy regex because the dark theme lives inside an `@media` block, and `[^}]*` would stop
 * at the first inner `}`.
 */
function block(css: string, from: number): { body: string; end: number } {
  const open = css.indexOf("{", from);
  if (open === -1) {
    throw new UnreadableTokens("expected a `{` after a selector and found none");
  }
  let depth = 0;
  for (let at = open; at < css.length; at += 1) {
    if (css[at] === "{") depth += 1;
    else if (css[at] === "}") {
      depth -= 1;
      if (depth === 0) {
        return { body: css.slice(open + 1, at), end: at + 1 };
      }
    }
  }
  throw new UnreadableTokens("a `{` in the token file is never closed");
}

/**
 * A capture group the pattern guarantees, as a `string`.
 *
 * `noUncheckedIndexedAccess` types every capture as possibly absent, which for a group outside a
 * `?` or `|` it never is. Saying so once here beats a non-null assertion at each use, and it still
 * throws rather than coercing if a pattern is ever edited to make the group optional.
 */
function group(match: RegExpExecArray, at: number): string {
  const value = match[at];
  if (value === undefined) {
    throw new UnreadableTokens(`capture ${at} is missing from a match of ${JSON.stringify(match[0])}`);
  }
  return value;
}

/** Every `--name: value` declaration directly inside one block body, in source order. */
function declarations(body: string): Map<string, string> {
  const found = new Map<string, string>();
  // Custom-property values may contain `(`, `,` and spaces but not `;`, which is what ends them.
  for (const statement of body.split(";")) {
    const match = /^\s*(--[\w-]+)\s*:\s*([\s\S]+?)\s*$/.exec(statement);
    if (!match) {
      continue;
    }
    const name = group(match, 1);
    const value = group(match, 2);
    if (found.has(name)) {
      throw new UnreadableTokens(
        `${name} is declared twice in one block; the second wins silently and the first is a lie`,
      );
    }
    found.set(name, value);
  }
  return found;
}

/** The `:root` block at or after `from`, or `null` when there is none. */
function rootBlock(css: string, from: number): { body: string; end: number } | null {
  const at = css.indexOf(":root", from);
  return at === -1 ? null : block(css, at);
}

const DARK_QUERY = "@media (prefers-color-scheme: dark)";

/**
 * Read the light and dark token sets.
 *
 * Dark is *layered over* light rather than standing alone: the dark block overrides only what
 * differs, so a token added to light and forgotten in dark still resolves — to the light value,
 * which the contrast check will then catch. Making dark a full independent set instead would turn
 * that mistake into a missing token, and a missing token is easy to skip past.
 */
export function readThemes(css: string): Themes {
  const text = stripComments(css);

  const darkAt = text.indexOf(DARK_QUERY);
  if (darkAt === -1) {
    throw new UnreadableTokens(`the token file defines no ${DARK_QUERY} block`);
  }
  const darkMedia = block(text, darkAt);

  const lightRoot = rootBlock(text, 0);
  if (!lightRoot || lightRoot.end > darkAt) {
    throw new UnreadableTokens("the token file defines no `:root` block outside the dark query");
  }
  const darkRoot = rootBlock(darkMedia.body, 0);
  if (!darkRoot) {
    throw new UnreadableTokens(`${DARK_QUERY} contains no \`:root\` block`);
  }

  const light = declarations(lightRoot.body);
  const dark = new Map(light);
  for (const [name, value] of declarations(darkRoot.body)) {
    if (!light.has(name)) {
      throw new UnreadableTokens(
        `${name} exists only in dark; a token the light theme lacks cannot be used by any rule`,
      );
    }
    dark.set(name, value);
  }

  return { light, dark };
}

/** How deep a `var()` chain may go before we call it a mistake rather than an indirection. */
const MAX_DEPTH = 8;

/**
 * Follow `var(--x)` until a literal value is reached.
 *
 * A fallback — `var(--x, red)` — is **refused**, not honoured. A fallback is what a token file
 * writes when it is not sure the token exists, and "not sure the token exists" is the condition
 * this whole module exists to make impossible.
 */
export function resolve(tokens: Declarations, name: string): string {
  let value = tokens.get(name);
  if (value === undefined) {
    throw new UnreadableTokens(`${name} is used but never declared`);
  }

  const seen = [name];
  let current = name;
  for (let depth = 0; depth < MAX_DEPTH; depth += 1) {
    const reference = /^var\(\s*(--[\w-]+)\s*(,[\s\S]*)?\)$/.exec(value.trim());
    if (!reference) {
      return value.trim();
    }
    if (reference[2] !== undefined) {
      throw new UnreadableTokens(
        `${current} falls back to a literal; a token that might not exist is not a token`,
      );
    }
    const target = group(reference, 1);
    if (seen.includes(target)) {
      throw new UnreadableTokens(`${[...seen, target].join(" → ")} is a cycle`);
    }
    const next = tokens.get(target);
    if (next === undefined) {
      throw new UnreadableTokens(`${target}, referenced by ${current}, is not declared`);
    }
    seen.push(target);
    current = target;
    value = next;
  }
  throw new UnreadableTokens(`${seen.join(" → ")} is more than ${MAX_DEPTH} deep`);
}

/**
 * A foreground token and the surface its name says it sits on.
 *
 * The pairing is derived from the **name**, not from a list kept beside the tokens. A list is a
 * second place to update, so a surface added without a foreground, or a foreground pointing at a
 * surface nobody defined, would pass a check written against the list. Here the same mistake is
 * an unresolvable partner and the check fails.
 */
export interface Pairing {
  /** The foreground token, e.g. `--color-muted-on-surface`. */
  foreground: string;
  /** The background token its name names, e.g. `--color-surface`. */
  background: string;
  /** `color` reads as text; `border` and `focus` are non-text under SC 1.4.11. */
  kind: "color" | "border" | "focus";
}

/** `--<kind>-[<qualifier>-]on-<base>` — the one naming rule the colour system has. */
const PAIRED = /^--(color|border|focus)-(?:[\w-]+?-)?on-([\w-]+)$/;

/** Every token whose name claims a background, with the token that background must be. */
export function pairings(tokens: Declarations): Pairing[] {
  const found: Pairing[] = [];
  for (const name of tokens.keys()) {
    const match = PAIRED.exec(name);
    if (!match) {
      continue;
    }
    found.push({
      foreground: name,
      background: `--color-${group(match, 2)}`,
      kind: group(match, 1) as Pairing["kind"],
    });
  }
  return found;
}
