//! Guards the single-binary promise in `CLAUDE.md` §1.
//!
//! The server embeds `ui/dist` at compile time. That directory is a build artefact and is
//! gitignored, so a fresh clone has no UI until `npm run build` has run in `ui/`. Two things
//! follow, and they pull in opposite directions:
//!
//! * `cargo test` must work on a machine with no Node toolchain, or the Rust suite becomes
//!   hostage to the frontend build.
//! * A **release** binary with a stub UI inside it would be a silent violation of the charter —
//!   the binary would start, serve a page, and be wrong.
//!
//! So: in a debug profile we synthesise a clearly-marked placeholder and warn loudly; in a release
//! profile we refuse to build at all. The placeholder is recorded by a sentinel file so a stale
//! placeholder left over from a debug build cannot slip into a later release build.

use std::io::Write as _;
use std::path::{Path, PathBuf};

/// Marks a `ui/dist` that this script synthesised rather than Vite producing.
const SENTINEL: &str = ".openbiz-placeholder";

fn main() {
    // rust-embed re-runs on changes under the folder, but only once the folder exists. Watching it
    // here also catches the transition from "absent" to "built".
    println!("cargo::rerun-if-changed=../../ui/dist");
    println!("cargo::rustc-check-cfg=cfg(openbiz_placeholder_ui)");

    let dist = dist_dir();
    let index = dist.join("index.html");
    let sentinel = dist.join(SENTINEL);

    let release = std::env::var("PROFILE").as_deref() == Ok("release");
    let real_ui = index.is_file() && !sentinel.exists();

    if real_ui {
        return;
    }

    if release {
        panic!(
            "the UI is not built, so this release binary would ship without one.\n\
             Run `npm ci && npm run build` in `ui/` before building for release.\n\
             (looked for {})",
            index.display()
        );
    }

    write_placeholder(&dist, &index, &sentinel);
    println!("cargo::rustc-cfg=openbiz_placeholder_ui");
    println!(
        "cargo::warning=ui/dist was not built; embedding a placeholder page. \
         Run `npm ci && npm run build` in ui/ for the real interface. \
         Release builds refuse to use the placeholder."
    );
}

fn dist_dir() -> PathBuf {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR");
    Path::new(&manifest).join("../../ui/dist")
}

/// Write a minimal stand-in page. It carries `<div id="root">` so it exercises the same serving
/// path as the real Vite output, and says plainly that it is not the product.
fn write_placeholder(dist: &Path, index: &Path, sentinel: &Path) {
    std::fs::create_dir_all(dist).expect("create ui/dist for the placeholder UI");

    let mut file = std::fs::File::create(index).expect("write the placeholder ui/dist/index.html");
    file.write_all(
        br#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <title>OpenBiz - UI not built</title>
  </head>
  <body>
    <div id="root">
      <h1>OpenBiz</h1>
      <p>
        This is a placeholder. The React interface was not built when this binary was compiled.
        Run <code>npm ci &amp;&amp; npm run build</code> in <code>ui/</code> and rebuild.
      </p>
    </div>
  </body>
</html>
"#,
    )
    .expect("write the placeholder ui/dist/index.html");

    std::fs::write(
        sentinel,
        "Written by crates/openbiz-server/build.rs because ui/dist was missing.\n\
         Delete this directory and run `npm run build` in ui/ to get the real interface.\n",
    )
    .expect("write the placeholder sentinel");
}
