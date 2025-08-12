#!/bin/bash

echo "🚀 Testing Solana Vanity Address Generator with Prefix and Suffix Support"
echo "=================================================================="

# Build the project
echo "🔨 Building project..."
cargo build --release

echo ""
echo "📋 Test Examples:"
echo ""

# Test 1: Prefix only
echo "🧪 Test 1: Prefix only (starting with 'ABC')"
echo "Command: cargo run --release -- --prefix ABC --threads 8"
echo "Expected: Find address starting with 'ABC'"
echo ""

# Test 2: Suffix only
echo "🧪 Test 2: Suffix only (ending with 'pump')"
echo "Command: cargo run --release -- --suffix pump --threads 8"
echo "Expected: Find address ending with 'pump'"
echo ""

# Test 3: Both prefix and suffix
echo "🧪 Test 3: Both prefix and suffix (starting with 'ABC', ending with 'pump')"
echo "Command: cargo run --release -- --prefix ABC --suffix pump --threads 8"
echo "Expected: Find address starting with 'ABC' and ending with 'pump'"
echo ""

# Test 4: Case insensitive with mixed case
echo "🧪 Test 4: Case insensitive with mixed case pattern"
echo "Command: cargo run --release -- --prefix AbC --suffix PuMp --case-mode mixed --threads 8"
echo "Expected: Find address with exact case matching for mixed patterns"
echo ""

# Test 5: High performance with optimal threading
echo "🧪 Test 5: High performance with optimal threading"
echo "Command: cargo run --release -- --prefix ABC --suffix pump --threads 0 --chunk-size 50000"
echo "Expected: Auto-detect optimal thread count and use large chunk size for performance"
echo ""

# Test 6: Limited attempts for testing
echo "🧪 Test 6: Limited attempts for testing (1000 attempts)"
echo "Command: cargo run --release -- --prefix ABC --suffix pump --max-attempts 1000 --threads 4"
echo "Expected: Stop after 1000 attempts (useful for testing)"
echo ""

# Test 7: Custom output filename
echo "🧪 Test 7: Custom output filename"
echo "Command: cargo run --release -- --prefix ABC --suffix pump --output my_address.json --threads 8"
echo "Expected: Save found address to my_address.json instead of data.json"
echo ""

# Test 8: JSON output with all features
echo "🧪 Test 8: JSON output with all features"
echo "Command: cargo run --release -- --prefix ABC --suffix pump --case-mode mixed --output vanity_result.json --threads 8"
echo "Expected: Save comprehensive data to vanity_result.json with case analysis"
echo ""

# Test 9: Append to existing file
echo "🧪 Test 9: Append to existing file"
echo "Command: cargo run --release -- --prefix XYZ --suffix moon --output addresses.json --threads 8"
echo "Expected: Add new address to existing addresses.json file (creates array of addresses)"
echo ""

# Test 10: Clear output file and start fresh
echo "🧪 Test 10: Clear output file and start fresh"
echo "Command: cargo run --release -- --prefix ABC --suffix pump --clear-output --output fresh_start.json --threads 8"
echo "Expected: Clear existing file and start with empty addresses array"
echo ""

echo "💡 Performance Tips:"
echo "   - Use --threads 0 for auto-optimization"
echo "   - Increase --chunk-size for better thread distribution"
echo "   - Use --case-mode mixed for exact case matching"
echo "   - Combine prefix + suffix for more specific searches"
echo ""

echo "💾 JSON Output Features:"
echo "   - Automatically appends new addresses to existing file (creates array)"
echo "   - Use --output filename.json to specify custom output file"
echo "   - Use --clear-output to reset file before starting new search"
echo "   - Each found address includes timestamp, parameters, and search stats"
echo "   - JSON format is human-readable and machine-parseable"
echo ""

echo "📁 File Structure:"
echo "   - Creates 'vanity_addresses' array in JSON file"
echo "   - Each address entry contains: address, private_key, found_at, search_parameters, search_stats"
echo "   - File grows as more addresses are found"
echo "   - Use --clear-output to start fresh"
echo ""

echo "📚 Display Features:"
echo "   - Automatically shows current addresses in output file at startup"
echo "   - Displays count and list of previously found addresses"
echo "   - Useful for tracking progress across multiple search sessions"
echo "   - Shows timestamp of when each address was found"
echo ""

echo "🔍 To run any test, copy the command and execute it in your terminal."
echo "⚠️  Note: Some patterns may take a very long time to find. Use --max-attempts for testing."
