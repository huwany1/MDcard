FROM rust:1.92-bookworm@sha256:e90e846de4124376164ddfbaab4b0774c7bdeef5e738866295e5a90a34a307a2
RUN apt-get update && apt-get install -y --no-install-recommends \
    gcc-mingw-w64-x86-64 g++-mingw-w64-x86-64 \
    libxkbcommon-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
    libegl1-mesa-dev binutils dpkg-dev xz-utils python3 \
    && rm -rf /var/lib/apt/lists/*
RUN rustup target add x86_64-pc-windows-gnu
ENV CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc
WORKDIR /work
