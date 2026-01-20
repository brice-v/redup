use std::hash::{DefaultHasher, Hash, Hasher};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::exit;
use std::fs::canonicalize;
use std::env::{Args, args};

use tokio::task;
use tokio::fs::File;
use tokio::io::{BufReader, AsyncReadExt, stdin};
use walkdir::WalkDir;

const VERSION: &str = env!("REDUP_VERSION");

const USAGE: &str = r#"
redup is a tool for finding duplicate files

Usage: redup [OPTIONS] [DIR]

Arguments:
    DIR                     Directory to recursively search

Options:
    -q, --quiet                Suppress output message
    -v, --verbose              Show detailed progress
    -V, --version              Show version message
    -h, --help                 Show this help message
    -o, --output               The filepath of output (Default: print to stdout)
    -f, --format [csv,sql,txt] Choose the output format (Default: txt)
    --                         Read file paths from standard input (pipe ls output)

The files are hashed so even files with the same name
will be found."#;

// TODOs:
//  - Update to only have quiet or verbose
//  - Better Report Errors to user
//     - Fix up error handling in general
//  - Start hashing while iterating over directories
//  - Cleanup output (make verbose more verbose and make sure default output can be read by other tools)

#[derive(Debug)]
enum OutputFormat {
    Csv,
    Sql,
    Text
}

#[derive(Debug)]
struct Config {
    quiet: bool,
    verbose: bool,
    help: bool,
    stdin_files: bool,
    directory: Option<String>,
    output_filepath: Option<String>,
    output_format: OutputFormat,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async { 
        run(args()).await?;
        Ok::<(), Box<dyn std::error::Error>>(())
    })?;
    Ok(())
}

async fn run(mut args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let config = parse_args(&mut args)?;
    
    if config.help {
        println!("{}", USAGE);
        return Ok(());
    }

    let mut m: HashMap<u64, Vec<String>> = HashMap::new();
    if config.stdin_files {
        let mut input = String::new();
        let mut stdin = stdin();
        stdin.read_to_string(&mut input).await?;
        let files: Vec<&str> = input.lines().collect();
        if config.verbose {
            println!("stdin files = {:#?}", files);
        }
        
        find_duplicates_from_list(&mut m, &files, &config).await?;
    } else {
        match config.directory {
            Some(ref dir) => find_duplicates_from_directory(&mut m, &dir, &config).await?,
            None => {
                eprintln!("{}", USAGE);
                exit(1);
            }
        }
    }
    match config.output_format {
        OutputFormat::Csv => println!("TODO: Handle CSV"),
        OutputFormat::Sql => println!("TODO: Handler SQL"),
        OutputFormat::Text => print_results(&mut m, &config)?,
    }
    Ok(())
}

fn print_usage_and_exit() {
    println!("{}", USAGE);
    exit(0);
}

fn print_version_and_exit() {
    println!("{}", VERSION);
    exit(0);
}

fn print_error_message_with_usage_and_exit(msg: &str) {
    println!("{}\n{}", msg, USAGE);
    exit(1);
}

fn parse_args(args: &mut Args) -> Result<Config, Box<dyn std::error::Error>> {
    let mut config = Config {
        quiet: false,
        verbose: false,
        help: false,
        stdin_files: false,
        directory: None,

        output_filepath: None,
        output_format: OutputFormat::Text,
    };
    
    let args_iter = args;
    args_iter.next(); // Skip program name

    while let Some(arg) = args_iter.next() {
        match arg.as_str() {
            "-q" | "--quiet" => config.quiet = true,
            "-v" | "--verbose" => config.verbose = true,
            "-V" | "--version" => print_version_and_exit(),
            "-h" | "--help" => print_usage_and_exit(),
            "-o" | "--output" => {
                if let Some(output_filepath) = args_iter.next() {
                    if std::path::Path::new(output_filepath.as_str()).exists() {
                        print_error_message_with_usage_and_exit(format!("ERROR: {output_filepath} already exists").as_str());
                    }
                    // TODO: Check if we have permissions to open this file for writing in current directory
                    // if not set to tmpdir file and log message
                    config.output_filepath = Some(output_filepath)
                } else {
                    print_error_message_with_usage_and_exit("ERROR: output expects a filepath");
                }
            },
            "-f" | "--format" => {
                if let Some(format_type) = args_iter.next() {
                    match format_type.to_ascii_lowercase().as_str() {
                        "txt" | "text" => config.output_format = OutputFormat::Text,
                        "csv" => config.output_format = OutputFormat::Csv,
                        "sql" | "sqlite" | "db" => config.output_format = OutputFormat::Sql,
                        _ => {
                            print_error_message_with_usage_and_exit(format!("ERROR: Invalid file format {format_type}").as_str());
                        }
                    }
                } else {
                    print_error_message_with_usage_and_exit("ERROR: format expects one of txt, csv, or sql");
                }
            }
            "--" => config.stdin_files = true,
            _ => {
                // If it's not a flag, treat it as directory path
                config.directory = Some(arg);
                break;
            }
        }
    }
    
    Ok(config)
}

async fn hash_file_contents(file_path: &str) -> Result<Option<u64>, tokio::io::Error> {
    let file = File::open(file_path).await?;
    let mut reader = BufReader::new(file);

    let hash_result = task::spawn_blocking(async move || {
        let mut hasher = DefaultHasher::new();
        let mut buffer = [0u8; 8192];

        loop {
            let bytes_read = reader.read(&mut buffer).await?;
            if bytes_read == 0 {
                break;
            }
            buffer[..bytes_read].hash(&mut hasher);
        }
        Ok::<u64, tokio::io::Error>(hasher.finish())
    }).await?.await?;

    Ok(Some(hash_result))
}

async fn find_duplicates_from_directory(
    m: &mut HashMap<u64, Vec<String>>, 
    directory: &str, 
    config: &Config
) -> Result<(), Box<dyn std::error::Error>> {
    let mut files = Vec::new();
    for entry in WalkDir::new(directory).into_iter() {
        let entry = entry?;
        let abs_path = entry.path().to_path_buf();

        if abs_path.is_dir() {
            if config.verbose {
                println!("Searching...\n\t{}", abs_path.display());
            }
            continue;
        } else if config.verbose {
            println!("\tFound file...\n\t\t{}", abs_path.display());
        }

        files.push(abs_path);
    }

    let results = get_hash_and_file_path(files).await;

    for (hash, path) in results {
        if let Some(v) = m.get_mut(&hash) {
            v.push(path);
        } else {
            m.insert(hash, vec![path]);
        }
    }

    Ok(())
}

async fn find_duplicates_from_list(
    m: &mut HashMap<u64, Vec<String>>, 
    files: &[&str], 
    config: &Config
) -> Result<(), Box<dyn std::error::Error>> {
    let mut file_paths = Vec::new();
    
    for file_path in files {
        if file_path.is_empty() {
            continue;
        }

        let abs_path = canonicalize(file_path)?;

        if abs_path.is_dir() {
            if config.verbose {
                println!("Searching directory...\n\t{}", abs_path.display());
            }
            let abs_path_s = abs_path.to_string_lossy().to_string();
            for entry in WalkDir::new(&abs_path_s).into_iter() {
                let entry = entry?;
                let file_abs_path = entry.path().to_path_buf();

                if !file_abs_path.is_dir() {
                    file_paths.push(file_abs_path);
                }
            }
        } else {
            // Handle as regular file
            if config.verbose {
                println!("Processing file...\n\t{}", abs_path.display());
            }

            file_paths.push(abs_path);
        }
    }

    let results = get_hash_and_file_path(file_paths).await;

    for (hash, path) in results {
        if let Some(v) = m.get_mut(&hash) {
            v.push(path);
        } else {
            m.insert(hash, vec![path]);
        }
    }

    Ok(())
}

async fn get_hash_and_file_path(file_paths: Vec<PathBuf>) -> Vec<(u64, String)> {
    let mut results = Vec::new();
    
    for abs_path in file_paths {
        let abs_path_s = abs_path.to_string_lossy().to_string();
        if let Ok(hash) = hash_file_contents(&abs_path_s).await {
            if let Some(hash) = hash {
                results.push((hash, abs_path_s));
            }
        };
    }
    results
}

fn print_results(m: &mut HashMap<u64, Vec<String>>, config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let duplicates_found = m.values().any(|e| e.len() > 1);

    if !config.quiet {
        if duplicates_found {
            println!("\nDUPLICATES FOUND!");
        } else {
            println!("\nNo Duplicates Found!");
        }
    }

    for item in m.values() {
        if item.len() <= 1 {
            continue;
        }
        println!("-");
        for e in item {
            println!("{}", e);
        }
    }

    Ok(())
}
