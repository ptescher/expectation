#!/usr/bin/env bash
# Builds the GitHub Pages site into target/pages (or $1): the landing page, the guide
# (mdBook), the API reference (rustdoc) and the kettle example's generated manual.
set -euo pipefail
cd "$(dirname "$0")/.."
out="${1:-target/pages}"
rm -rf "${out:?}"
mkdir -p "$out"
out="$(cd "$out" && pwd)"

# The example's manual, from a real run: tutorials, topics, the test report and screenshots.
cargo run -q -p expectation_harness --example kettle -- "$out/example"

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
