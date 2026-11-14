# Build image for the M0 spikes: Linux native and Windows cross-compile.
FROM rust:1.96-bookworm

# clang and lld compile and link the C and assembly parts of `ring` for the MSVC target.
RUN apt-get update \
    && apt-get install -y --no-install-recommends clang lld llvm \
    && rm -rf /var/lib/apt/lists/*

RUN rustup component add rustfmt clippy \
    && rustup target add x86_64-pc-windows-msvc \
    && cargo install --locked cargo-xwin

# cargo-xwin downloads the Microsoft CRT and Windows SDK on first use.
# Building with XWIN_ACCEPT_LICENSE=1 means you accept Microsoft's licence:
# https://go.microsoft.com/fwlink/?LinkId=2086102
ARG XWIN_ACCEPT_LICENSE
ENV XWIN_ACCEPT_LICENSE=${XWIN_ACCEPT_LICENSE}
