# Strawberry Web Server 🍓

A lightweight, fast image serving web server written in Rust that provides on-the-fly image resizing and optimization with intelligent caching.

## Features

- **Dynamic Image Resizing** - Resize images on-the-fly using URL parameters
- **Quality Optimization** - Adjust JPEG compression quality dynamically
- **Smart Caching** - Automatically caches processed images for faster subsequent requests
- **Multiple Format Support** - Handles PNG, JPEG, GIF, WebP, and BMP formats
- **Aspect Ratio Preservation** - Maintains aspect ratio when only width or height is specified
- **Security** - Path traversal protection built-in
- **Simple Setup** - Minimal configuration required

## Installation

### Prerequisites

- Rust (latest stable version)
- Cargo

### Building from Source

```bash
git clone https://github.com/MiranDaniel/strawberry
cd strawberry
cargo build --release
```

The compiled binary will be available at `target/release/strawberry`

## Usage

### Running the Server

```bash
# Run with default settings (serves from ./images directory)
./strawberry

# Or specify a custom image directory
IMAGE_DIR=/path/to/your/images ./strawberry
```

The server will start on `http://localhost:8000` by default.

### Directory Structure

```
.
├── strawberry          # The executable
├── images/            # Default image directory
│   ├── photo1.jpg
│   ├── photo2.png
│   └── ...
├── cache/             # Auto-generated cache directory
│   
```
