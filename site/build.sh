#!/usr/bin/env bash
# Builds the GitHub Pages site into target/pages (or $1): the landing page, the guide
# (mdBook), the API reference (rustdoc), the kettle example's generated manual, and the
# multi-platform demo's manual, and the rate-watch example's two manuals.
set -euo pipefail
cd "$(dirname "$0")/.."
out="${1:-target/pages}"
rm -rf "${out:?}"
mkdir -p "$out"
out="$(cd "$out" && pwd)"

# The example's manual, from a real run: tutorials, topics, the test report and screenshots.
cargo run -q -p expectation_harness --example kettle -- "$out/example"

# The multi-platform demo's manual, from the results and screenshots its runs saved.
mkdir -p "$out/demo"
cp -R demos/kettle/docs/. "$out/demo/"
cargo run -q -p expectation_cli -- docs demos/kettle/expectations "$out/demo"

# The feature-docs example's manuals: all of it, and just the approved part.
cargo run -q -p expectation_cli -- docs examples/rate-watch "$out/rate-watch"
cargo run -q -p expectation_cli -- docs examples/rate-watch "$out/rate-watch-approved" --approved-only

# The guide.
mdbook build book --dest-dir "$out/guide"

# The API reference.
cargo doc -q --workspace --no-deps
cp -R target/doc "$out/api"

# The landing page, on the same theme as the manual.
cp site/index.html site/style.css "$out/"
cp "$out/example/site/assets/theme.css" "$out/theme.css"
touch "$out/.nojekyll"
echo "site: $out/index.html"
