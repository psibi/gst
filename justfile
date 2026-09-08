# List all recipes
list:
    just --list

# Dev server with live reload (http://localhost:8080)
serve:
    trunk serve

# Production build into dist/
build:
    trunk build --release

# Production build for GitHub Pages into dist/.
# Site is served from http://psibi.in/gst, so assets are emitted with the
# /gst/ base path.
publish:
    trunk build --release --public-url /gst/

# All CI checks: formatting, clippy on the host and wasm32 targets, tests.
ci:
    cargo fmt --all -- --check
    cargo clippy --all-targets -- --deny warnings
    cargo clippy --target wasm32-unknown-unknown --all-targets -- --deny warnings
    cargo test
