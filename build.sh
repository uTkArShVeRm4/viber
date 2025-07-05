#!/bin/bash
set -e

echo "Starting build process for WASM Audio Visualizer..."

# Install Rust if not present
if ! command -v rustc &> /dev/null; then
    echo "Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source ~/.cargo/env
fi

# Install wasm-pack if not present
if ! command -v wasm-pack &> /dev/null; then
    echo "Installing wasm-pack..."
    curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
fi

# Add wasm32 target
rustup target add wasm32-unknown-unknown

# Build the WASM package
echo "Building WASM package..."
wasm-pack build --target web --out-dir pkg

# Create build directory
mkdir -p build

# Copy web files
echo "Copying web files..."
cp www/* build/

# Copy WASM package
echo "Copying WASM package..."
cp -r pkg build/

echo "Build completed successfully!"
echo "Files ready in build/ directory"