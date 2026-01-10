use std::io::Read;
use std::hash::Hash;
use std::hash::Hasher;
use std::collections::HashMap;

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
    let mut args = std::env::args();
    if args.len() != 2 {
        eprintln!("{}", USAGE);
        std::process::exit(1);
    }
    let directory = args.nth(1).unwrap();
    let mut m: HashMap<u64, Vec<String>> = HashMap::new();
    for entry in WalkDir::new(directory) {
        let abs_path = std::fs::canonicalize(entry?.path())?;
        if abs_path.is_dir() {
            println!("Searching {}...", abs_path.display());
            continue;
        } else {
            println!("Found file {}...", abs_path.display());
        }
        let abs_path_s = String::from(abs_path.clone().to_str().unwrap());
        let mut f = std::fs::File::open(abs_path)?;
        let mut buffer = Vec::new();
        // read the whole file
        f.read_to_end(&mut buffer)?;
        println!("buffer = {:?}", buffer);
        let mut hasher = std::hash::DefaultHasher::new();
        buffer.hash(&mut hasher);
        let hash_result = hasher.finish();
        if let Some(v) = m.get_mut(&hash_result) {
            v.push(abs_path_s);
        } else {
            m.insert(hash_result, vec![abs_path_s]);
        }
    }
    if !m.is_empty() {
        println!("DUPLICATES FOUND: {:#?}", m);
    }
    Ok(())
}
