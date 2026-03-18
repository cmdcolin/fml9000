use fml9000_core::{delete_tracks_by_filename, is_audio_file, scan_single_file};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::mpsc;

pub enum FileWatchEvent {
  Added(PathBuf),
  Removed(PathBuf),
}

pub fn start_file_watcher(
  folders: &[String],
) -> Option<(RecommendedWatcher, mpsc::Receiver<FileWatchEvent>)> {
  let (notify_tx, notify_rx) = mpsc::channel();
  let (event_tx, event_rx) = mpsc::channel();

  let mut watcher = match RecommendedWatcher::new(
    move |result: Result<Event, notify::Error>| {
      if let Ok(event) = result {
        let _ = notify_tx.send(event);
      }
    },
    notify::Config::default(),
  ) {
    Ok(w) => w,
    Err(e) => {
      eprintln!("Warning: Could not create file watcher: {e}");
      return None;
    }
  };

  for folder in folders {
    let path = std::path::Path::new(folder);
    if path.is_dir() {
      if let Err(e) = watcher.watch(path, RecursiveMode::Recursive) {
        eprintln!("Warning: Could not watch folder {folder}: {e}");
      }
    }
  }

  std::thread::spawn(move || {
    while let Ok(event) = notify_rx.recv() {
      match event.kind {
        EventKind::Create(_) | EventKind::Modify(_) => {
          for path in &event.paths {
            if path.is_file() && is_audio_file(path) {
              let _ = event_tx.send(FileWatchEvent::Added(path.clone()));
            }
          }
        }
        EventKind::Remove(_) => {
          for path in &event.paths {
            if is_audio_file(path) {
              let _ = event_tx.send(FileWatchEvent::Removed(path.clone()));
            }
          }
        }
        _ => {}
      }
    }
  });

  Some((watcher, event_rx))
}

pub fn handle_file_event(event: FileWatchEvent) -> FileChangeResult {
  match event {
    FileWatchEvent::Added(path) => {
      match scan_single_file(&path) {
        Ok(Some(_track)) => FileChangeResult::Added,
        Ok(None) => FileChangeResult::NoChange,
        Err(e) => {
          eprintln!("Warning: Failed to scan {}: {e}", path.display());
          FileChangeResult::NoChange
        }
      }
    }
    FileWatchEvent::Removed(path) => {
      if let Some(path_str) = path.to_str() {
        match delete_tracks_by_filename(&[path_str.to_string()]) {
          Ok(count) if count > 0 => FileChangeResult::Removed,
          Ok(_) => FileChangeResult::NoChange,
          Err(e) => {
            eprintln!("Warning: Failed to remove {}: {e}", path.display());
            FileChangeResult::NoChange
          }
        }
      } else {
        FileChangeResult::NoChange
      }
    }
  }
}

pub enum FileChangeResult {
  Added,
  Removed,
  NoChange,
}
