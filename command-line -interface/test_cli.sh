#!/bin/bash
# Test script to demonstrate CLI validation

echo "=== P2P File Converter CLI Tests ==="
echo

echo "1. Testing help output:"
cargo run -- --help
echo

echo "2. Testing receiver mode (should work):"
cargo run -- --output ./test_output
echo

echo "3. Testing invalid multiaddr (should fail):"
cargo run -- --target "invalid-address" --file Cargo.toml || echo "✅ Correctly rejected invalid multiaddr"
echo

echo "4. Testing missing file (should fail):"
cargo run -- --target "/ip4/127.0.0.1/tcp/8080" --file "nonexistent.txt" || echo "✅ Correctly rejected missing file"
echo

echo "5. Testing partial arguments (should fail):"
cargo run -- --target "/ip4/127.0.0.1/tcp/8080" || echo "✅ Correctly required both target and file"
echo

echo "6. Testing valid sender arguments (should work):"
# Create a test file
echo "test content" > test_file.txt
cargo run -- --target "/ip4/127.0.0.1/tcp/8080/p2p/12D3KooWBmwkafWE2fqfzS96VoTZgpGp6aJsF4SJ6eAR5AHXCXAZ" --file test_file.txt
# Clean up
rm -f test_file.txt
echo

echo "=== Tests Complete ==="
