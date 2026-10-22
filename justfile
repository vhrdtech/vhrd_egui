# Library crates (apps depend on them from git) plus the app template command ve_template.

# Bare `just` only lists the recipes
default:
    @just --list

test:
    cargo test --workspace

lint:
    cargo clippy --workspace --all-targets -- -D warnings
    cargo fmt --all --check
    just check-wasm

# The crates a wasm32 dashboard build pulls in (PLT-7, PLT-9, BACK-3): no std::time/fs/thread/process that
# panics on the web; each crate's clippy.toml backs this with a disallowed-methods list. ve_backdrop twice:
# the CPU painter alone (WebGL2 bundle) and with the shader (WebGPU bundle).
check-wasm:
    cargo clippy --target wasm32-unknown-unknown -p ve_dash -p ve_theme -p ve_basics -p ve_frame -p ve_settle --features ve_settle/frame,ve_settle/serde -- -D warnings
    cargo clippy --target wasm32-unknown-unknown -p ve_backdrop --no-default-features -- -D warnings
    cargo clippy --target wasm32-unknown-unknown -p ve_backdrop --features wgpu -- -D warnings

# Refresh ve_theme's copy of the vhrd_brand palette export (sibling checkout); build.rs makes the constants
brand:
    cp ../vhrd_brand/web/palette.json ve_theme/brand/palette.json

# Local install: the app template command (the crates themselves are depended on, not installed)
install:
    cargo build --profile deploy --locked -p ve_template
    # With tpm on PATH: tpm install (kept-releases stack, one copy on PATH); otherwise: cargo install
    if command -v tpm >/dev/null; then tpm install target/deploy/ve_template; else cargo install --locked --path ve_template; fi

# Generate an app from the template: just new NAME [DIR]
new NAME DIR=".":
    cargo run -q -p ve_template -- new {{NAME}} --out {{DIR}}

# Bring this PC up to main and install the template command (the libraries are not deployed: apps pick up main through their Cargo.toml)
deploy:
    test -z "$(git status --porcelain)" || { echo "local changes, commit or move them first:" >&2; git status --short >&2; exit 1; }
    git pull --ff-only
    just install
    ve_template --version | grep -qF "($(git rev-parse --short HEAD)"
    ve_template --version

# Regenerate ve_theme's emoji fallback font from the current Noto Emoji (OFL): Regular weight, only the code points of emoji-glyphs.txt (THM-14)
emoji-font:
    curl -fsSL -o /tmp/NotoEmoji-var.ttf 'https://github.com/google/fonts/raw/main/ofl/notoemoji/NotoEmoji%5Bwght%5D.ttf'
    uv run --with fonttools python -m fontTools.varLib.instancer /tmp/NotoEmoji-var.ttf wght=400 -o /tmp/NotoEmoji-regular.ttf
    uv run --with fonttools pyftsubset /tmp/NotoEmoji-regular.ttf --unicodes-file=ve_theme/fonts/emoji-glyphs.txt --output-file=ve_theme/fonts/NotoEmoji-subset.ttf --layout-features= --no-hinting --desubroutinize
