FROM mcr.microsoft.com/devcontainers/rust:1-bullseye

# Install mingw-w64 for cross-compiling to Windows
RUN apt-get update && apt-get install -y \
    gcc-mingw-w64-x86-64 \
    && rm -rf /var/lib/apt/lists/*

# Add the Windows cross-compilation target for the vscode user
USER vscode
RUN rustup target add x86_64-pc-windows-gnu
