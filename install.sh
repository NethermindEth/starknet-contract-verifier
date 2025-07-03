#!/bin/bash

# Starknet Contract Verifier Installation Script
# Automatically detects platform and downloads the latest release

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# GitHub repository
REPO="NethermindEth/starknet-contract-verifier"
BINARY_NAME="starknet-contract-verifier"

# Function to print colored output
print_status() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Detect platform
detect_platform() {
    local os
    local arch
    
    os=$(uname -s | tr '[:upper:]' '[:lower:]')
    arch=$(uname -m)
    
    case "$os" in
        linux*)
            case "$arch" in
                x86_64|amd64)
                    echo "linux-x86_64"
                    ;;
                aarch64|arm64)
                    echo "linux-aarch64"
                    ;;
                *)
                    print_error "Unsupported architecture: $arch"
                    exit 1
                    ;;
            esac
            ;;
        darwin*)
            case "$arch" in
                x86_64)
                    echo "macos-x86_64"
                    ;;
                arm64)
                    echo "macos-aarch64"
                    ;;
                *)
                    print_error "Unsupported architecture: $arch"
                    exit 1
                    ;;
            esac
            ;;
        *)
            print_error "Unsupported operating system: $os"
            print_error "Please download manually from: https://github.com/$REPO/releases"
            exit 1
            ;;
    esac
}

# Get latest release version
get_latest_version() {
    curl -s "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/'
}

# Download and install
install_binary() {
    local platform="$1"
    local version="$2"
    local filename="${BINARY_NAME}-${platform}.tar.gz"
    local download_url="https://github.com/$REPO/releases/download/$version/$filename"
    local temp_dir
    
    temp_dir=$(mktemp -d)
    
    print_status "Downloading $filename..."
    if curl -L -o "$temp_dir/$filename" "$download_url"; then
        print_status "Download successful"
    else
        print_error "Failed to download $filename"
        exit 1
    fi
    
    print_status "Extracting binary..."
    tar -xzf "$temp_dir/$filename" -C "$temp_dir"
    
    # Check if /usr/local/bin is writable, otherwise suggest alternative
    local install_dir="/usr/local/bin"
    if [[ ! -w "$install_dir" ]] && [[ ! -w "$(dirname "$install_dir")" ]]; then
        print_warning "/usr/local/bin is not writable"
        print_status "Installing to ~/.local/bin instead..."
        install_dir="$HOME/.local/bin"
        mkdir -p "$install_dir"
        
        # Add to PATH if not already there
        if [[ ":$PATH:" != *":$install_dir:"* ]]; then
            print_warning "Please add ~/.local/bin to your PATH:"
            echo "echo 'export PATH=\"\$HOME/.local/bin:\$PATH\"' >> ~/.bashrc"
            echo "source ~/.bashrc"
        fi
    fi
    
    if [[ -w "$install_dir" ]] || [[ -w "$(dirname "$install_dir")" ]]; then
        if mv "$temp_dir/$BINARY_NAME" "$install_dir/"; then
            chmod +x "$install_dir/$BINARY_NAME"
            print_status "Successfully installed $BINARY_NAME to $install_dir"
        else
            print_error "Failed to install binary to $install_dir"
            print_status "Trying with sudo..."
            if sudo mv "$temp_dir/$BINARY_NAME" "$install_dir/" && sudo chmod +x "$install_dir/$BINARY_NAME"; then
                print_status "Successfully installed $BINARY_NAME to $install_dir"
            else
                print_error "Failed to install binary even with sudo"
                exit 1
            fi
        fi
    else
        print_error "Cannot write to $install_dir"
        exit 1
    fi
    
    # Cleanup
    rm -rf "$temp_dir"
}

# Verify installation
verify_installation() {
    if command -v "$BINARY_NAME" >/dev/null 2>&1; then
        local version
        version=$("$BINARY_NAME" --version 2>/dev/null || echo "unknown")
        print_status "Installation verified! Version: $version"
        print_status "You can now run: $BINARY_NAME --help"
    else
        print_warning "Binary installed but not found in PATH"
        print_status "You may need to restart your shell or add the installation directory to PATH"
    fi
}

# Main installation flow
main() {
    print_status "Starting Starknet Contract Verifier installation..."
    
    # Check dependencies
    if ! command -v curl >/dev/null 2>&1; then
        print_error "curl is required but not installed"
        exit 1
    fi
    
    if ! command -v tar >/dev/null 2>&1; then
        print_error "tar is required but not installed"
        exit 1
    fi
    
    # Detect platform
    local platform
    platform=$(detect_platform)
    print_status "Detected platform: $platform"
    
    # Get latest version
    local version
    version=$(get_latest_version)
    if [[ -z "$version" ]]; then
        print_error "Failed to get latest version"
        exit 1
    fi
    print_status "Latest version: $version"
    
    # Install
    install_binary "$platform" "$version"
    
    # Verify
    verify_installation
    
    print_status "Installation complete!"
}

# Run main function
main "$@" 