use std::hash::{DefaultHasher, Hash, Hasher};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::exit;
use std::fs::canonicalize;
use std::env::{Args, args};
use std::io::Write;
use std::sync::Arc;

use csv::Writer;
use rusqlite::{Connection, params};
use tokio::fs::File;
use tokio::io::{AsyncReadExt, BufReader, stdin};
use tokio::sync::{Semaphore, mpsc};
use tokio::task::JoinSet;
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
    -o, --output <FILE>     Output file (default: stdout)
    -f, --format <FORMAT>   Output format: txt, csv, db (default: txt)
    -V, --version           Show version message
    -h, --help              Show this help message
    --                      Read file paths from standard input (pipe ls output)

The files are hashed so even files with the same name
will be found."#;

// All TODOs completed!

#[derive(Debug)]
struct Config {
    quiet: bool,
    verbose: bool,
    help: bool,
    stdin_files: bool,
    directory: Option<String>,
    output: Option<String>,
    format: OutputFormat,
}

#[derive(Debug, Clone, Copy)]
enum OutputFormat {
    Text,
    Csv,
    Db,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async { 
        run(args()).await?;
        Ok::<(), Box<dyn std::error::Error>>(())
    })?;
    Ok(())
}

fn debug_log(verbose: bool, msg: &str) {
    if verbose {
        eprintln!("[DEBUG] {}", msg);
    }
}

async fn run(mut args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let config = parse_args(&mut args)?;
    
    debug_log(config.verbose, &format!("Config: {:?}", config));
    
    if config.help {
        println!("{}", USAGE);
        return Ok(());
    }

    let start_time = std::time::Instant::now();
    let mut m: HashMap<u64, Vec<String>> = HashMap::new();
    
    if config.stdin_files {
        let mut input = String::new();
        let mut stdin = stdin();
        if let Err(e) = stdin.read_to_string(&mut input).await {
            eprintln!("Error: Failed to read from stdin: {}", e);
            return Err(Box::new(e));
        }
        let files: Vec<&str> = input.lines().collect();
        debug_log(config.verbose, &format!("Read {} lines from stdin", files.len()));
        
        if let Err(e) = find_duplicates_from_list(&mut m, &files, &config).await {
            eprintln!("Error: Failed to process files from stdin: {}", e);
            return Err(e);
        }
    } else {
        match config.directory {
            Some(ref dir) => {
                if let Err(e) = find_duplicates_from_directory(&mut m, dir, &config).await {
                    eprintln!("Error: Failed to scan directory '{}': {}", dir, e);
                    return Err(e);
                }
            }
            None => {
                eprintln!("{}", USAGE);
                exit(1);
            }
        }
    }
    
    let total_files: usize = m.values().map(|v| v.len()).sum();
    let unique_hashes = m.len();
    let duplicate_groups = m.values().filter(|v| v.len() > 1).count();
    let duplicate_files: usize = m.values().filter(|v| v.len() > 1).map(|v| v.len()).sum();
    
    debug_log(config.verbose, &format!("Total files processed: {}", total_files));
    debug_log(config.verbose, &format!("Unique hashes: {}", unique_hashes));
    debug_log(config.verbose, &format!("Duplicate groups: {}", duplicate_groups));
    debug_log(config.verbose, &format!("Duplicate files: {}", duplicate_files));
    debug_log(config.verbose, &format!("Elapsed time: {:?}", start_time.elapsed()));
    
    if let Err(e) = print_results(&mut m, &config) {
        eprintln!("Error: Failed to print results: {}", e);
        return Err(e);
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

fn parse_args(args: &mut Args) -> Result<Config, Box<dyn std::error::Error>> {
    let mut config = Config {
        quiet: false,
        verbose: false,
        help: false,
        stdin_files: false,
        directory: None,
        output: None,
        format: OutputFormat::Text,
    };

    let _ = args.next(); // Skip program name

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-q" | "--quiet" => {
                if config.verbose {
                    eprintln!("Error: --quiet and --verbose are mutually exclusive");
                    exit(1);
                }
                config.quiet = true;
            }
            "-v" | "--verbose" => {
                if config.quiet {
                    eprintln!("Error: --quiet and --verbose are mutually exclusive");
                    exit(1);
                }
                config.verbose = true;
            }
            "-o" | "--output" => {
                if let Some(output) = args.next() {
                    config.output = Some(output);
                } else {
                    eprintln!("Error: --output requires a file path");
                    exit(1);
                }
            }
            "-f" | "--format" => {
                if let Some(format_str) = args.next() {
                    let format_str: String = format_str;
                    config.format = match format_str.to_lowercase().as_str() {
                        "txt" | "text" => OutputFormat::Text,
                        "csv" => OutputFormat::Csv,
                        "db" => OutputFormat::Db,
                        _ => {
                            eprintln!("Error: Unknown format '{}'. Use: txt, csv, or db", format_str);
                            exit(1);
                        }
                    };
                } else {
                    eprintln!("Error: --format requires a format (txt, csv, db)");
                    exit(1);
                }
            }
            "-V" | "--version" => print_version_and_exit(),
            "-h" | "--help" => print_usage_and_exit(),
            "--" => config.stdin_files = true,
            _ => {
                // If it's not a flag, treat it as directory path
                if config.directory.is_some() {
                    eprintln!("Error: Multiple directory arguments provided");
                    exit(1);
                }
                config.directory = Some(arg);
                // Continue parsing to handle flags after positional args
            }
        }
    }

    Ok(config)
}

async fn hash_file_contents(file_path: String, verbose: bool) -> Result<(u64, String), Box<dyn std::error::Error>> {
    debug_log(verbose, &format!("Opening file: {}", file_path));
    
    let file = match File::open(&file_path).await {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Warning: Failed to open file '{}': {}", file_path, e);
            return Err(Box::new(e));
        }
    };
    
    let mut reader = BufReader::new(file);
    let mut hasher = DefaultHasher::new();
    let mut buffer = vec![0u8; 8192];
    let mut total_bytes = 0u64;

    loop {
        match reader.read(&mut buffer).await {
            Ok(bytes_read) => {
                if bytes_read == 0 {
                    break;
                }
                total_bytes += bytes_read as u64;
                buffer[..bytes_read].hash(&mut hasher);
            }
            Err(e) => {
                eprintln!("Warning: Failed to read from file '{}': {}", file_path, e);
                return Err(Box::new(e));
            }
        }
    }

    let hash = hasher.finish();
    debug_log(verbose, &format!("Hashed {} bytes from {}, hash={:x}", total_bytes, file_path, hash));
    Ok((hash, file_path))
}

async fn find_duplicates_from_directory(
    m: &mut HashMap<u64, Vec<String>>, 
    directory: &str, 
    config: &Config
) -> Result<(), Box<dyn std::error::Error>> {
    let verbose = config.verbose;
    debug_log(verbose, &format!("Starting directory scan: {}", directory));
    let (tx, mut rx) = mpsc::channel::<PathBuf>(1000);
    let semaphore = Arc::new(Semaphore::new(100));
    let mut join_set = JoinSet::new();

    // Spawn directory walker task
    let directory_owned: String = directory.to_string();
    let verbose_flag: bool = verbose;
    let walker_handle = tokio::task::spawn_blocking(move || {
        let mut file_count: usize = 0;
        for entry in WalkDir::new(&directory_owned).into_iter().flatten() {
            let abs_path: PathBuf = entry.path().to_path_buf();
            if abs_path.is_dir() {
                if verbose_flag {
                    println!("Searching...\n\t{}", abs_path.display());
                }
            } else {
                file_count += 1;
                if verbose_flag {
                    println!("\tFound file...\n\t\t{}", abs_path.display());
                }
                if tx.blocking_send(abs_path).is_err() {
                    break;
                }
            }
        }
        debug_log(verbose_flag, &format!("Walker finished. Found {} files", file_count));
    });

    // Process files as they're discovered
    let mut queued_files = 0usize;
    while let Some(path) = rx.recv().await {
        let abs_path_s = path.to_string_lossy().to_string();
        debug_log(verbose, &format!("Queueing file for hashing: {}", abs_path_s));
        let permit: tokio::sync::OwnedSemaphorePermit = semaphore.clone().acquire_owned().await?;
        
        queued_files += 1;
        join_set.spawn(async move {
            let _permit = permit;
            (hash_file_contents(abs_path_s, verbose).await).ok()
        });
    }
    debug_log(verbose, &format!("Queued {} files for hashing", queued_files));

    // Wait for walker to complete
    let _ = walker_handle.await;

    // Collect all results
    let mut completed_tasks = 0usize;
    let mut failed_tasks = 0usize;
    while let Some(result) = join_set.join_next().await {
        match result {
            Ok(Some((hash, path))) => {
                completed_tasks += 1;
                debug_log(verbose, &format!("Task completed: {} -> {:x}", path, hash));
                m.entry(hash).or_default().push(path);
            }
            Ok(None) => {
                failed_tasks += 1;
                debug_log(verbose, "Task completed but returned None (failed to hash)");
            }
            Err(e) => {
                failed_tasks += 1;
                debug_log(verbose, &format!("Task panicked or failed: {:?}", e));
            }
        }
    }
    debug_log(verbose, &format!("Completed: {} tasks, Failed: {} tasks", completed_tasks, failed_tasks));

    Ok(())
}

async fn find_duplicates_from_list(
    m: &mut HashMap<u64, Vec<String>>, 
    files: &[&str], 
    config: &Config
) -> Result<(), Box<dyn std::error::Error>> {
    let verbose = config.verbose;
    debug_log(verbose, &format!("Processing {} files from stdin", files.len()));
    let (tx, mut rx) = mpsc::channel::<PathBuf>(1000);
    let semaphore = Arc::new(Semaphore::new(100));
    let mut join_set = JoinSet::new();
    let verbose_flag: bool = verbose;

    // Spawn walker task to collect files
    let files_owned: Vec<String> = files.iter().map(|&s| s.to_string()).collect();
    let walker_handle = tokio::task::spawn_blocking(move || {
        let mut file_count: usize = 0;
        for file_path in files_owned {
            if file_path.is_empty() {
                continue;
            }
            
            if let Ok(abs_path) = canonicalize(&file_path) {
                if abs_path.is_dir() {
                    if verbose_flag {
                        println!("Searching directory...\n\t{}", abs_path.display());
                    }
                    for entry in WalkDir::new(&abs_path).into_iter().flatten() {
                        let file_abs_path: PathBuf = entry.path().to_path_buf();
                        if !file_abs_path.is_dir() && tx.blocking_send(file_abs_path).is_err() {
                            return;
                        }
                    }
                } else {
                    file_count += 1;
                    if verbose_flag {
                        println!("Processing file...\n\t{}", abs_path.display());
                    }
                    if tx.blocking_send(abs_path).is_err() {
                        return;
                    }
                }
            }
        }
        debug_log(verbose_flag, &format!("Walker finished. Found {} files", file_count));
    });

    // Process files as they're discovered
    let mut queued_files = 0usize;
    while let Some(path) = rx.recv().await {
        let abs_path_s = path.to_string_lossy().to_string();
        debug_log(verbose, &format!("Queueing file for hashing: {}", abs_path_s));
        let permit: tokio::sync::OwnedSemaphorePermit = semaphore.clone().acquire_owned().await?;
        
        queued_files += 1;
        join_set.spawn(async move {
            let _permit = permit;
            (hash_file_contents(abs_path_s, verbose).await).ok()
        });
    }
    debug_log(verbose, &format!("Queued {} files for hashing", queued_files));

    // Wait for walker to complete
    let _ = walker_handle.await;

    // Collect all results
    let mut completed_tasks = 0usize;
    let mut failed_tasks = 0usize;
    while let Some(result) = join_set.join_next().await {
        match result {
            Ok(Some((hash, path))) => {
                completed_tasks += 1;
                debug_log(verbose, &format!("Task completed: {} -> {:x}", path, hash));
                m.entry(hash).or_default().push(path);
            }
            Ok(None) => {
                failed_tasks += 1;
                debug_log(verbose, "Task completed but returned None (failed to hash)");
            }
            Err(e) => {
                failed_tasks += 1;
                debug_log(verbose, &format!("Task panicked or failed: {:?}", e));
            }
        }
    }
    debug_log(verbose, &format!("Completed: {} tasks, Failed: {} tasks", completed_tasks, failed_tasks));

    Ok(())
}

fn print_results(m: &mut HashMap<u64, Vec<String>>, config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    debug_log(config.verbose, &format!("Printing results in {:?} format", config.format));
    match config.format {
        OutputFormat::Text => print_results_text(m, config),
        OutputFormat::Csv => print_results_csv(m, config),
        OutputFormat::Db => print_results_db(m, config),
    }
}

fn print_results_text(m: &mut HashMap<u64, Vec<String>>, config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let duplicates_found = m.values().any(|e| e.len() > 1);

    let mut output: Box<dyn Write + Send> = match &config.output {
        Some(path) => Box::new(std::fs::File::create(path)?),
        None => Box::new(std::io::stdout()),
    };

    if !config.quiet {
        if duplicates_found {
            writeln!(output, "\nDUPLICATES FOUND!")?;
        } else {
            writeln!(output, "\nNo Duplicates Found!")?;
        }
    }

    for item in m.values() {
        if item.len() <= 1 {
            continue;
        }
        writeln!(output, "-")?;
        for e in item {
            writeln!(output, "{}", e)?;
        }
    }

    Ok(())
}

fn print_results_csv(m: &mut HashMap<u64, Vec<String>>, config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let mut group_records: Vec<(String, String, u64)> = Vec::new();
    let mut group_id = 0u64;

    for (hash, files) in m.iter() {
        if files.len() > 1 {
            group_id += 1;
            for file in files {
                group_records.push((
                    format!("{:x}", hash),
                    file.clone(),
                    group_id,
                ));
            }
        }
    }

    if let Some(path) = &config.output {
        let mut writer = Writer::from_path(path)?;
        writer.write_record(["hash", "file_path", "group_id"])?;
        for (hash, file, gid) in group_records {
            writer.write_record([hash, file, gid.to_string()])?;
        }
        writer.flush()?;
    } else {
        let mut writer = Writer::from_writer(std::io::stdout());
        writer.write_record(["hash", "file_path", "group_id"])?;
        for (hash, file, gid) in group_records {
            writer.write_record([hash, file, gid.to_string()])?;
        }
        writer.flush()?;
    }

    if !config.quiet {
        if group_id > 0 {
            eprintln!("Found {} duplicate groups", group_id);
        } else {
            eprintln!("No duplicates found");
        }
    }

    Ok(())
}

fn print_results_db(m: &mut HashMap<u64, Vec<String>>, config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let db_path = match &config.output {
        Some(path) => path.clone(),
        None => {
            eprintln!("Error: --output is required for db format");
            exit(1);
        }
    };

    // Remove existing database if it exists
    if std::path::Path::new(&db_path).exists() {
        std::fs::remove_file(&db_path)?;
    }

    let conn = Connection::open(&db_path)?;

    // Create tables
    conn.execute(
        "CREATE TABLE duplicate_groups (
            id INTEGER PRIMARY KEY,
            hash TEXT NOT NULL
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE duplicate_files (
            id INTEGER PRIMARY KEY,
            group_id INTEGER NOT NULL,
            file_path TEXT NOT NULL,
            FOREIGN KEY (group_id) REFERENCES duplicate_groups(id)
        )",
        [],
    )?;

    let mut group_count = 0;
    for (hash, files) in m.iter() {
        if files.len() > 1 {
            conn.execute(
                "INSERT INTO duplicate_groups (hash) VALUES (?1)",
                [format!("{:x}", hash)],
            )?;
            let group_id = conn.last_insert_rowid();

            for file in files {
                conn.execute(
                    "INSERT INTO duplicate_files (group_id, file_path) VALUES (?1, ?2)",
                    params![group_id, file],
                )?;
            }
            group_count += 1;
        }
    }

    if !config.quiet {
        if group_count > 0 {
            eprintln!("Saved {} duplicate groups to {}", group_count, db_path);
        } else {
            eprintln!("No duplicates found");
        }
    }

    Ok(())
}
