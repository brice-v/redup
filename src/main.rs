use std::io::{self, Read};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::collections::HashMap;
use std::process::exit;
use std::fs::{File, canonicalize};
use std::env::{Args, args};

use walkdir::WalkDir;

const VERSION: &str = env!("REDUP_VERSION");

const USAGE: &str = r#"
redup is a tool for finding duplicate files

Usage: redup [OPTIONS] [DIR]

Arguments:
    DIR                     Directory to recursively search

Options:
    -q, --quiet             Suppress output message
    -v, --verbose           Show detailed progress
    -h, --help              Show this help message
    --                      Read file paths from standard input (pipe ls output)

The files are hashed so even files with the same name
will be found."#;

#[derive(Debug)]
struct Config {
    quiet: bool,
    verbose: bool,
    help: bool,
    stdin_files: bool,
    directory: Option<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    run(args())
}

fn run(mut args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let config = parse_args(&mut args)?;
    
    if config.help {
        println!("{}", USAGE);
        return Ok(());
    }

    let mut m: HashMap<u64, Vec<String>> = HashMap::new();
    if config.stdin_files {
        // Read from stdin
        let mut input = String::new();
        io::stdin().read_to_string(&mut input)?;
        let files: Vec<&str> = input.lines().collect();
        if config.verbose {
            println!("stdin files = {:#?}", files);
        }
        
        find_duplicates_from_list(&mut m, &files, &config)?;
    } else {
        // Read from directory
        match config.directory {
            Some(ref dir) => find_duplicates_from_directory(&mut m, &dir, &config)?,
            None => {
                eprintln!("{}", USAGE);
                exit(1);
            }
        }
    }
    print_results(&mut m, &config)?;
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

fn parse_args(args: &mut Args) -> Result<Config, Box<dyn std::error::Error>> {
    let mut config = Config {
        quiet: false,
        verbose: false,
        help: false,
        stdin_files: false,
        directory: None,
    };
    
    let args_iter = args;
    args_iter.next(); // Skip program name
    
    // Process arguments
    while let Some(arg) = args_iter.next() {
        match arg.as_str() {
            "-q" | "--quiet" => config.quiet = true,
            "-v" | "--verbose" => config.verbose = true,
            "-V" | "--version" => print_version_and_exit(),
            "-h" | "--help" => print_usage_and_exit(),
            "--" => config.stdin_files = true,
            _ => break,
        }
    }
    
    Ok(config)
}

fn find_duplicates_from_directory(m: &mut HashMap<u64, Vec<String>>, directory: &str, config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    for entry in WalkDir::new(directory).into_iter() {
        let entry = entry?;
        let abs_path = canonicalize(entry.path())?;
        
        if abs_path.is_dir() {
            if config.verbose {
                println!("Searching...\n\t{}", abs_path.display());
            }
            continue;
        } else if config.verbose {
            println!("\tFound file...\n\t\t{}", abs_path.display());
        }
        
        let abs_path_s = abs_path.to_string_lossy().to_string();
        let mut f = File::open(&abs_path)?;
        let mut buffer = Vec::new();
        
        // read the whole file
        f.read_to_end(&mut buffer)?;
        
        let mut hasher = DefaultHasher::new();
        buffer.hash(&mut hasher);
        let hash_result = hasher.finish();
        
        if let Some(v) = m.get_mut(&hash_result) {
            v.push(abs_path_s);
        } else {
            m.insert(hash_result, vec![abs_path_s]);
        }
    }
    Ok(())
}

fn find_duplicates_from_list(m: &mut HashMap<u64, Vec<String>>, files: &[&str], config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    for file_path in files {
        if file_path.is_empty() {
            continue;
        }
        
        let abs_path = canonicalize(file_path)?;
        
        // Check if this is a directory
        if abs_path.is_dir() {
            // Recursively search the directory
            if config.verbose {
                println!("Searching directory...\n\t{}", abs_path.display());
            }
            
            let abs_path_s = abs_path.to_string_lossy().to_string();
            find_duplicates_from_directory(m, &abs_path_s, &config)?;
        } else {
            // Handle as regular file
            if config.verbose {
                println!("Processing file...\n\t{}", abs_path.display());
            }
            
            let abs_path_s = abs_path.to_string_lossy().to_string();
            let mut f = File::open(&abs_path)?;
            let mut buffer = Vec::new();
            
            // read the whole file
            f.read_to_end(&mut buffer)?;
            
            let mut hasher = DefaultHasher::new();
            buffer.hash(&mut hasher);
            let hash_result = hasher.finish();
            
            if let Some(v) = m.get_mut(&hash_result) {
                v.push(abs_path_s);
            } else {
                m.insert(hash_result, vec![abs_path_s]);
            }
        }
    }
    Ok(())
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
