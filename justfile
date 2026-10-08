# Library: nothing to run or ship, apps depend on the crates from git.

test:
    cargo test --workspace

lint:
    cargo clippy --workspace --all-targets -- -D warnings
    cargo fmt --all --check

# Local install: libraries have nothing to install
install:
    @echo "vhrd_egui is a library, nothing to install; apps depend on it (path or git)"

# Libraries are not deployed
deploy:
    @echo "vhrd_egui is a library, nothing to deploy; apps pick up main through their Cargo.toml"
