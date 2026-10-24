FROM rust:1.92-bookworm
RUN apt-get update && apt-get install -y --no-install-recommends \
    gcc-mingw-w64-x86-64 g++-mingw-w64-x86-64 \
    libxkbcommon-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
    libegl1-mesa-dev libgl1-mesa-dri libglx-mesa0 libxrandr2 libxcursor1 libxi6 libxkbcommon-x11-0 mesa-vulkan-drivers libvulkan1 \
    xvfb xauth xdotool x11-utils imagemagick dbus-x11 openbox xclip zenity xdg-desktop-portal xdg-desktop-portal-gtk \
    && rm -rf /var/lib/apt/lists/*
RUN rustup target add x86_64-pc-windows-gnu && rustup component add rustfmt clippy
ENV CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc
WORKDIR /work
