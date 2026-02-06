# Redup - Duplicate File Finder

A command-line tool for finding duplicate files by hashing their contents, even when they have different names.

## Features

- Recursively searches directories for duplicate files
- Uses file content hashing to detect duplicates (not just filename-based)
- Async concurrent file processing with Tokio
- Streams file contents to minimize memory usage
- Multiple output formats: text, CSV, SQLite database
- Can read file paths from stdin for piping from other commands
- Quiet and verbose modes for controlling output
- Debug logging when verbose mode is enabled

## Installation

```bash
# build from source
git clone https://github.com/brice-v/redup.git
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
    -o, --output <FILE>     Output file (default: stdout)
    -f, --format <FORMAT>   Output format: txt, csv, db (default: txt)
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

# Output to CSV file
redup -f csv -o results.csv /path/to/directory

# Output to SQLite database
redup -f db -o results.db /path/to/directory

# Verbose mode with debug output
redup -v /path/to/directory
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
2. Streams file contents asynchronously using 8KB buffers
3. Computes a hash of the file contents using `DefaultHasher`
4. Groups files by their hash values
5. Reports any groups containing more than one file (duplicates)

## Output Formats

### Text (default)
Human-readable output showing duplicate groups separated by `-`

### CSV
Comma-separated values with columns: `hash,file_path,group_id`

### SQLite Database
Creates a database with two tables:
- `duplicate_groups` (id, hash)
- `duplicate_files` (id, group_id, file_path)
