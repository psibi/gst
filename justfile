# List all recipes
list:
    just --list

# Dev server with live reload (http://localhost:8080)
serve:
    trunk serve

# Production build into dist/
build:
    trunk build --release
