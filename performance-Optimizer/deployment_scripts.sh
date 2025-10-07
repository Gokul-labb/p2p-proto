#!/bin/bash
# deployment_scripts.sh - Comprehensive deployment automation for P2P File Converter

set -euo pipefail

# Configuration
PROJECT_NAME="p2p-file-converter"
VERSION="${VERSION:-$(cargo metadata --no-deps --format-version 1 | jq -r '.packages[0].version')}"
BUILD_DIR="${BUILD_DIR:-./build}"
DIST_DIR="${DIST_DIR:-./dist}"
TARGET_PLATFORMS=(
    "x86_64-unknown-linux-gnu"
    "x86_64-pc-windows-msvc"
    "x86_64-apple-darwin"
    "aarch64-unknown-linux-gnu"
    "aarch64-apple-darwin"
)

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Logging functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Clean previous builds
clean_build() {
    log_info "Cleaning previous builds..."
    rm -rf "$BUILD_DIR" "$DIST_DIR"
    mkdir -p "$BUILD_DIR" "$DIST_DIR"
    cargo clean
}

# Install required tools
install_tools() {
    log_info "Installing required build tools..."

    # Install cross-compilation targets
    for target in "${TARGET_PLATFORMS[@]}"; do
        log_info "Adding target: $target"
        rustup target add "$target" || log_warning "Failed to add target $target"
    done

    # Install additional tools
    if ! command -v cargo-deb &> /dev/null; then
        log_info "Installing cargo-deb..."
        cargo install cargo-deb
    fi

    if ! command -v cargo-generate-rpm &> /dev/null; then
        log_info "Installing cargo-generate-rpm..."
        cargo install cargo-generate-rpm
    fi

    if ! command -v upx &> /dev/null; then
        log_warning "UPX not found - binary compression will be skipped"
    fi
}

# Build for single platform
build_platform() {
    local target="$1"
    local profile="${2:-production}"

    log_info "Building for $target with $profile profile..."

    local start_time=$(date +%s)

    # Build command
    local build_cmd="cargo build --profile $profile --target $target"

    # Platform-specific configurations
    case "$target" in
        *windows*)
            export CC_x86_64_pc_windows_msvc="cl.exe"
            export AR_x86_64_pc_windows_msvc="lib.exe"
            ;;
        *linux-gnu)
            if [[ "$target" == "aarch64"* ]]; then
                export CC_aarch64_unknown_linux_gnu="aarch64-linux-gnu-gcc"
                export AR_aarch64_unknown_linux_gnu="aarch64-linux-gnu-ar"
            fi
            ;;
    esac

    # Execute build
    if eval "$build_cmd"; then
        local end_time=$(date +%s)
        local build_time=$((end_time - start_time))

        # Find binary path
        local binary_name="$PROJECT_NAME"
        if [[ "$target" == *windows* ]]; then
            binary_name="$binary_name.exe"
        fi

        local binary_path="target/$target/$profile/$binary_name"

        if [[ -f "$binary_path" ]]; then
            local binary_size=$(stat -f%z "$binary_path" 2>/dev/null || stat -c%s "$binary_path")
            local size_mb=$(echo "scale=2; $binary_size / 1024 / 1024" | bc)

            log_success "Built $target in ${build_time}s (${size_mb}MB)"

            # Copy to build directory
            local output_name="$PROJECT_NAME-$VERSION-$target"
            if [[ "$target" == *windows* ]]; then
                output_name="$output_name.exe"
            fi

            cp "$binary_path" "$BUILD_DIR/$output_name"

            # Post-process binary
            post_process_binary "$BUILD_DIR/$output_name" "$target"

            return 0
        else
            log_error "Binary not found at $binary_path"
            return 1
        fi
    else
        log_error "Build failed for $target"
        return 1
    fi
}

# Post-process binary (strip, compress)
post_process_binary() {
    local binary_path="$1"
    local target="$2"

    log_info "Post-processing binary for $target..."

    # Strip symbols (if not Windows)
    if [[ "$target" != *windows* ]] && command -v strip &> /dev/null; then
        log_info "Stripping debug symbols..."
        strip "$binary_path" || log_warning "Failed to strip symbols"
    fi

    # Compress with UPX (if available)
    if command -v upx &> /dev/null; then
        log_info "Compressing with UPX..."
        local original_size=$(stat -f%z "$binary_path" 2>/dev/null || stat -c%s "$binary_path")

        if upx --best --lzma "$binary_path" 2>/dev/null; then
            local compressed_size=$(stat -f%z "$binary_path" 2>/dev/null || stat -c%s "$binary_path")
            local ratio=$(echo "scale=1; (1 - $compressed_size / $original_size) * 100" | bc)
            log_success "Compressed binary by ${ratio}%"
        else
            log_warning "UPX compression failed"
        fi
    fi
}

# Build all platforms
build_all_platforms() {
    log_info "Building for all platforms..."

    local successful_builds=0
    local total_builds=${#TARGET_PLATFORMS[@]}

    for target in "${TARGET_PLATFORMS[@]}"; do
        if build_platform "$target" "production"; then
            ((successful_builds++))
        fi
    done

    log_info "Build completed: $successful_builds/$total_builds successful"

    if [[ $successful_builds -eq $total_builds ]]; then
        log_success "All platform builds successful!"
        return 0
    else
        log_warning "Some platform builds failed"
        return 1
    fi
}

# Run comprehensive tests
run_tests() {
    log_info "Running comprehensive test suite..."

    # Unit tests
    log_info "Running unit tests..."
    cargo test --lib --bins

    # Integration tests
    log_info "Running integration tests..."
    cargo test --test '*'

    # Documentation tests
    log_info "Running documentation tests..."
    cargo test --doc

    # Benchmarks (quick run)
    log_info "Running benchmarks..."
    cargo bench -- --quick

    log_success "All tests completed successfully"
}

# Generate packages
generate_packages() {
    log_info "Generating distribution packages..."

    # Create source tarball
    log_info "Creating source tarball..."
    tar -czf "$DIST_DIR/$PROJECT_NAME-$VERSION-src.tar.gz" \
        --exclude=target \
        --exclude=.git \
        --exclude="$BUILD_DIR" \
        --exclude="$DIST_DIR" \
        .

    # Create binary tarballs
    for target in "${TARGET_PLATFORMS[@]}"; do
        local binary_name="$PROJECT_NAME-$VERSION-$target"
        if [[ "$target" == *windows* ]]; then
            binary_name="$binary_name.exe"
        fi

        if [[ -f "$BUILD_DIR/$binary_name" ]]; then
            log_info "Creating tarball for $target..."

            # Create temporary directory with proper structure
            local temp_dir=$(mktemp -d)
            local package_dir="$temp_dir/$PROJECT_NAME-$VERSION"
            mkdir -p "$package_dir"

            # Copy files
            cp "$BUILD_DIR/$binary_name" "$package_dir/p2p-converter$(if [[ "$target" == *windows* ]]; then echo .exe; fi)"
            cp README.md LICENSE "$package_dir/"
            cp -r examples "$package_dir/" 2>/dev/null || true
            cp -r config "$package_dir/" 2>/dev/null || true

            # Create tarball
            (cd "$temp_dir" && tar -czf "$DIST_DIR/$PROJECT_NAME-$VERSION-$target.tar.gz" "$PROJECT_NAME-$VERSION")

            # Cleanup
            rm -rf "$temp_dir"

            log_success "Created package for $target"
        fi
    done

    # Generate Linux packages (Debian)
    if cargo deb --version &>/dev/null; then
        log_info "Generating Debian package..."
        cargo deb --output "$DIST_DIR/"
    fi

    # Generate RPM packages
    if cargo generate-rpm --version &>/dev/null; then
        log_info "Generating RPM package..."
        cargo generate-rpm -o "$DIST_DIR/"
    fi
}

# Generate checksums
generate_checksums() {
    log_info "Generating checksums..."

    cd "$DIST_DIR"

    # Generate SHA256 checksums
    if command -v sha256sum &> /dev/null; then
        sha256sum * > SHA256SUMS
    elif command -v shasum &> /dev/null; then
        shasum -a 256 * > SHA256SUMS
    fi

    # Generate MD5 checksums
    if command -v md5sum &> /dev/null; then
        md5sum * > MD5SUMS
    elif command -v md5 &> /dev/null; then
        md5 * > MD5SUMS
    fi

    cd - > /dev/null

    log_success "Checksums generated"
}

# Generate release notes
generate_release_notes() {
    log_info "Generating release notes..."

    cat > "$DIST_DIR/RELEASE_NOTES.md" << EOF
# P2P File Converter v${VERSION}

## Release Information

- **Version**: ${VERSION}
- **Release Date**: $(date +"%Y-%m-%d")
- **Build Date**: $(date +"%Y-%m-%d %H:%M:%S %Z")

## Supported Platforms

EOF

    for target in "${TARGET_PLATFORMS[@]}"; do
        local binary_name="$PROJECT_NAME-$VERSION-$target"
        if [[ "$target" == *windows* ]]; then
            binary_name="$binary_name.exe"
        fi

        if [[ -f "$BUILD_DIR/$binary_name" ]]; then
            local size=$(stat -f%z "$BUILD_DIR/$binary_name" 2>/dev/null || stat -c%s "$BUILD_DIR/$binary_name")
            local size_mb=$(echo "scale=2; $size / 1024 / 1024" | bc)
            echo "- **$target**: ${size_mb}MB" >> "$DIST_DIR/RELEASE_NOTES.md"
        fi
    done

    cat >> "$DIST_DIR/RELEASE_NOTES.md" << EOF

## Installation

### From Binary Releases

Download the appropriate binary for your platform and add it to your PATH:

\`\`\`bash
# Linux/macOS
curl -L "https://github.com/user/p2p-file-converter/releases/download/v${VERSION}/p2p-file-converter-${VERSION}-x86_64-unknown-linux-gnu.tar.gz" | tar -xz
sudo mv p2p-file-converter-${VERSION}/p2p-converter /usr/local/bin/

# Windows (PowerShell)
Invoke-WebRequest -Uri "https://github.com/user/p2p-file-converter/releases/download/v${VERSION}/p2p-file-converter-${VERSION}-x86_64-pc-windows-msvc.tar.gz" -OutFile "p2p-converter.tar.gz"
\`\`\`

### From Package Managers

\`\`\`bash
# Debian/Ubuntu
wget https://github.com/user/p2p-file-converter/releases/download/v${VERSION}/p2p-file-converter_${VERSION}_amd64.deb
sudo dpkg -i p2p-file-converter_${VERSION}_amd64.deb

# Red Hat/CentOS/Fedora
wget https://github.com/user/p2p-file-converter/releases/download/v${VERSION}/p2p-file-converter-${VERSION}-1.x86_64.rpm
sudo rpm -i p2p-file-converter-${VERSION}-1.x86_64.rpm
\`\`\`

### From Source

\`\`\`bash
cargo install p2p-file-converter
\`\`\`

## Usage

\`\`\`bash
# Start receiver
p2p-converter

# Send file
p2p-converter --target /ip4/peer/tcp/8080/p2p/ID --file document.txt --format pdf
\`\`\`

## Changes in This Release

See [CHANGELOG.md](CHANGELOG.md) for detailed changes.

## Support

- **Documentation**: https://docs.rs/p2p-file-converter
- **Issues**: https://github.com/user/p2p-file-converter/issues
- **Discussions**: https://github.com/user/p2p-file-converter/discussions

EOF

    log_success "Release notes generated"
}

# Verify release integrity
verify_release() {
    log_info "Verifying release integrity..."

    local errors=0

    # Check all expected files exist
    for target in "${TARGET_PLATFORMS[@]}"; do
        local binary_name="$PROJECT_NAME-$VERSION-$target"
        if [[ "$target" == *windows* ]]; then
            binary_name="$binary_name.exe"
        fi

        if [[ ! -f "$BUILD_DIR/$binary_name" ]]; then
            log_error "Missing binary: $binary_name"
            ((errors++))
        fi

        local tarball="$DIST_DIR/$PROJECT_NAME-$VERSION-$target.tar.gz"
        if [[ ! -f "$tarball" ]]; then
            log_error "Missing tarball: $(basename "$tarball")"
            ((errors++))
        fi
    done

    # Check checksums exist
    if [[ ! -f "$DIST_DIR/SHA256SUMS" ]]; then
        log_error "Missing SHA256SUMS"
        ((errors++))
    fi

    # Test binary execution
    for target in "${TARGET_PLATFORMS[@]}"; do
        local binary_name="$PROJECT_NAME-$VERSION-$target"
        if [[ "$target" == *windows* ]]; then
            binary_name="$binary_name.exe"
        fi

        local binary_path="$BUILD_DIR/$binary_name"
        if [[ -f "$binary_path" ]] && [[ "$target" != *windows* ]]; then
            log_info "Testing binary: $target"
            if ! "$binary_path" --version >/dev/null 2>&1; then
                log_error "Binary test failed for $target"
                ((errors++))
            fi
        fi
    done

    if [[ $errors -eq 0 ]]; then
        log_success "Release verification passed"
        return 0
    else
        log_error "Release verification failed with $errors errors"
        return 1
    fi
}

# Main deployment function
main() {
    local command="${1:-build}"

    case "$command" in
        "clean")
            clean_build
            ;;
        "tools")
            install_tools
            ;;
        "build")
            clean_build
            install_tools
            build_all_platforms
            ;;
        "test")
            run_tests
            ;;
        "package")
            generate_packages
            generate_checksums
            generate_release_notes
            ;;
        "verify")
            verify_release
            ;;
        "release")
            clean_build
            install_tools
            run_tests
            build_all_platforms
            generate_packages
            generate_checksums
            generate_release_notes
            verify_release

            log_success "🎉 Release build completed successfully!"
            log_info "📦 Artifacts available in: $DIST_DIR"
            log_info "🔍 Binary builds in: $BUILD_DIR"
            ;;
        *)
            echo "Usage: $0 {clean|tools|build|test|package|verify|release}"
            echo ""
            echo "Commands:"
            echo "  clean     - Clean previous builds"
            echo "  tools     - Install required build tools"
            echo "  build     - Build for all platforms"
            echo "  test      - Run comprehensive tests"
            echo "  package   - Generate distribution packages"
            echo "  verify    - Verify release integrity"
            echo "  release   - Full release build (all steps)"
            exit 1
            ;;
    esac
}

# Handle script interruption
trap 'log_error "Script interrupted"; exit 1' INT TERM

# Run main function
main "$@"
