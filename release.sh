#!/bin/bash
set -e

echo "Building Terra-Link in release mode..."
cargo build --release

echo "Stripping debugging symbols to minimize binary size..."
strip target/release/terra-link

echo "Release build complete! The optimized binary is located at target/release/terra-link"
