# Library crates (apps depend on them from git) plus the app template command ve_template.

# Bare `just` only lists the recipes
default:
    @just --list

test:
    cargo test --workspace

lint:
    cargo clippy --workspace --all-targets -- -D warnings
    cargo fmt --all --check

# Refresh ve_theme's copy of the vhrd_brand palette export (sibling checkout); build.rs makes the constants
brand:
    cp ../vhrd_brand/web/palette.json ve_theme/brand/palette.json

# Local install: the app template command (the crates themselves are depended on, not installed)
install:
    cargo install --path ve_template --locked

# Generate an app from the template: just new NAME [DIR]
new NAME DIR=".":
    cargo run -q -p ve_template -- new {{NAME}} --out {{DIR}}

# Libraries are not deployed
deploy:
    @echo "vhrd_egui is a library, nothing to deploy; apps pick up main through their Cargo.toml"
