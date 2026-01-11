use std::io::Read;
use std::hash::{DefaultHasher, Hash};
use std::hash::Hasher;
use std::collections::HashMap;
use std::process::exit;
use std::fs::{File, canonicalize};
use std::env::{Args, args};

use walkdir::WalkDir;

const USAGE: &str = r#"
redup is a tool for finding duplicate files

Usage:
    redup <dir>

          <dir> is the directory to recursively search

The files are hashed so even files with the same name
will be found.
"#;

// TODO: Handle Errors Properly
// TODO: Add Flags
// -q --quiet (supress errors)
// -v --verbose (print while doing everything)
// -- read from stdin a list of files (possibly from ls or otherwise and get duplicates from it?)
// TODO: Fix up imports

fn main() -> Result<(), Box<dyn std::error::Error>> {
    run(args())
}

fn run(mut args: Args) -> Result<(), Box<dyn std::error::Error>> {
    if args.len() != 2 {
        eprintln!("{}", USAGE);
        exit(1);
    }
    let directory = args.nth(1).unwrap();
    let mut m: HashMap<u64, Vec<String>> = HashMap::new();
    for entry in WalkDir::new(directory) {
        let abs_path = canonicalize(entry?.path())?;
        if abs_path.is_dir() {
            println!("Searching...\n\t{}", abs_path.display());
            continue;
        } else {
            println!("\tFound file...\n\t\t{}", abs_path.display());
        }
        let abs_path_s = String::from(abs_path.clone().to_str().unwrap());
        let mut f = File::open(abs_path)?;
        let mut buffer = Vec::new();
        // read the whole file
        f.read_to_end(&mut buffer)?;
        // println!("buffer = {:?}", buffer);
        let mut hasher = DefaultHasher::new();
        buffer.hash(&mut hasher);
        let hash_result = hasher.finish();
        if let Some(v) = m.get_mut(&hash_result) {
            v.push(abs_path_s);
        } else {
            m.insert(hash_result, vec![abs_path_s]);
        }
    }
    if m.values().any(|e| e.len() > 1) {
        println!("\nDUPLICATES FOUND!");
    } else {
        println!("\nNo Duplicates Found!");
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