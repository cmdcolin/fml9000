use fml9000_core::{
    init_db, load_tracks, run_scan_with_progress, delete_tracks_by_filename,
    settings::{read_settings, write_settings},
    CoreSettings, ScanProgress,
};
use std::collections::HashSet;
use std::io::{self, BufRead, Write};

fn prompt_line(msg: &str) -> String {
    print!("{msg}");
    let _ = io::stdout().flush();
    let mut input = String::new();
    let _ = io::stdin().lock().read_line(&mut input);
    input.trim().to_string()
}

fn confirm(msg: &str) -> bool {
    prompt_line(msg).eq_ignore_ascii_case("y")
}

fn setup_folders(settings: &mut CoreSettings) {
    println!("No music folders configured.");
    println!("Enter directory paths to add to your library (empty line to finish):");
    println!();

    loop {
        let path = prompt_line("  Add folder: ");
        if path.is_empty() {
            break;
        }

        let expanded = if path.starts_with('~') {
            if let Some(home) = std::env::var_os("HOME") {
                let home = home.to_string_lossy();
                path.replacen('~', &home, 1)
            } else {
                path
            }
        } else {
            path
        };

        let p = std::path::Path::new(&expanded);
        if !p.is_dir() {
            eprintln!("    Not a valid directory: {expanded}");
            continue;
        }

        let canonical = match p.canonicalize() {
            Ok(c) => c.to_string_lossy().to_string(),
            Err(_) => expanded,
        };

        settings.add_folder(canonical.clone());
        println!("    Added: {canonical}");
    }

    if settings.folders.is_empty() {
        eprintln!("No folders added. Exiting.");
        std::process::exit(0);
    }

    if let Err(e) = write_settings(settings) {
        eprintln!("Warning: Failed to save settings: {e}");
    } else {
        println!();
        println!("Settings saved.");
    }
}

fn run_scan(folders: Vec<String>) {
    let tracks = load_tracks().unwrap_or_default();
    let mut existing_complete: HashSet<String> = HashSet::new();
    let mut existing_incomplete: HashSet<String> = HashSet::new();
    for track in &tracks {
        if track.duration_seconds.is_some() {
            existing_complete.insert(track.filename.clone());
        } else {
            existing_incomplete.insert(track.filename.clone());
        }
    }

    println!("Scanning {} folder(s)...", folders.len());
    for folder in &folders {
        println!("  {folder}");
    }
    println!("{} tracks already in library", tracks.len());
    println!();

    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        run_scan_with_progress(folders, existing_complete, existing_incomplete, tx);
    });

    for progress in rx {
        match progress {
            ScanProgress::StartingFolder(folder) => {
                println!("Scanning: {folder}");
            }
            ScanProgress::FoundFile(found, skipped, _file) => {
                print!("\r  Found {found} files ({skipped} existing)...");
                let _ = io::stdout().flush();
            }
            ScanProgress::ScannedFile(found, skipped, added, updated, _file) => {
                if updated > 0 {
                    print!("\r  {found} files, {skipped} existing, {added} new, {updated} updated");
                } else {
                    print!("\r  {found} files, {skipped} existing, {added} new");
                }
                let _ = io::stdout().flush();
            }
            ScanProgress::Complete(found, skipped, added, updated, stale_files) => {
                println!();
                println!();
                println!("Scan complete:");
                println!("  {found} files found");
                println!("  {skipped} already up to date");
                println!("  {added} added");
                if updated > 0 {
                    println!("  {updated} updated");
                }

                if !stale_files.is_empty() {
                    println!();
                    println!("{} tracks no longer found on disk:", stale_files.len());
                    for f in &stale_files {
                        println!("  {f}");
                    }
                    println!();
                    if confirm("Remove these from the library? [y/N] ") {
                        match delete_tracks_by_filename(&stale_files) {
                            Ok(count) => println!("Removed {count} tracks."),
                            Err(e) => eprintln!("Failed to remove tracks: {e}"),
                        }
                    } else {
                        println!("Skipped.");
                    }
                }
            }
        }
    }
}

fn main() {
    let mut settings: CoreSettings = read_settings();

    let folders_from_args: Vec<String> = std::env::args().skip(1).collect();

    if !folders_from_args.is_empty() {
        for folder in &folders_from_args {
            let p = std::path::Path::new(folder);
            if !p.is_dir() {
                eprintln!("Not a valid directory: {folder}");
                std::process::exit(1);
            }
        }
        let mut new_folders = Vec::new();
        for folder in &folders_from_args {
            let canonical = match std::path::Path::new(folder).canonicalize() {
                Ok(c) => c.to_string_lossy().to_string(),
                Err(_) => folder.clone(),
            };
            settings.add_folder(canonical.clone());
            new_folders.push(canonical);
        }
        if let Err(e) = write_settings(&settings) {
            eprintln!("Warning: Failed to save settings: {e}");
        } else {
            println!("Added {} folder(s) to config.", new_folders.len());
        }
    }

    if settings.folders.is_empty() {
        setup_folders(&mut settings);
    }

    if let Err(e) = init_db() {
        eprintln!("Failed to initialize database: {e}");
        std::process::exit(1);
    }

    run_scan(settings.folders.clone());
}
