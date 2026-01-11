# Redup - Duplicate File Finder

A command-line tool for finding duplicate files by hashing their contents, even when they have different names.

## Features

- Recursively searches directories for duplicate files
- Uses file content hashing to detect duplicates (not just filename-based)
- Supports standard CLI flags for customization
- Can read file paths from stdin for piping from other commands
- Quiet and verbose modes for controlling output

## Installation

```bash
# Or build from source
git clone https://github.com/yourusername/redup.git
cd redup
cargo build --release
```

## Usage

```
redup is a tool for finding duplicate files

Usage: redup [OPTIONS] [DIR]

Arguments:
    DIR                     Directory to recursively search

Options:
    -q, --quiet             Suppress output message
    -v, --verbose           Show detailed progress
    -V, --version           Show version message
    -h, --help              Show this help message
    --                      Read file paths from standard input (pipe ls output)
```

## Examples

### Basic Usage
```bash
# Search current directory
redup

# Search specific directory
redup /path/to/directory

# Search with verbose output
redup -v /path/to/directory

# Suppress output (quiet mode)
redup -q /path/to/directory
```

### Piping from Other Commands
```bash
# Find duplicates from ls output
ls /path/to/directory | redup --

# Find duplicates from find command
find /path/to/directory -type f | redup --
```

## How It Works

1. Recursively walks through the specified directory
2. For each file, reads the entire content into memory
3. Computes a hash of the file contents using `DefaultHasher`
4. Groups files by their hash values
5. Reports any groups containing more than one file (duplicates)
