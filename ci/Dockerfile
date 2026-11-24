# Build image: lint, test, Linux native and Windows cross-compile, packaging.
# Keep in step with rust-toolchain.toml. Every tool is pinned: the image builds the same
# way next month. Raise a version on purpose, and read what changed.
FROM rust:1.96.0-bookworm

# clang and lld compile and link the C and assembly parts of `ring` for the MSVC target.
RUN apt-get update \
    && apt-get install -y --no-install-recommends clang lld llvm zip \
    && rm -rf /var/lib/apt/lists/*

RUN rustup component add rustfmt clippy \
    && rustup target add x86_64-pc-windows-msvc \
    && cargo install --locked cargo-xwin@0.24.1 cargo-nextest@0.9.148 cargo-deny@0.20.2 \
        cargo-deb@3.8.0 cargo-generate-rpm@0.21.1

# cargo-xwin downloads the Microsoft CRT and Windows SDK on first use.
# Building with XWIN_ACCEPT_LICENSE=1 means you accept Microsoft's licence:
# https://go.microsoft.com/fwlink/?LinkId=2086102
ARG XWIN_ACCEPT_LICENSE
ENV XWIN_ACCEPT_LICENSE=${XWIN_ACCEPT_LICENSE}
