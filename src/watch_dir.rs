use notify::{watcher, RecursiveMode, Watcher};
use std::sync::mpsc::channel;
use std::time::Duration;

pub fn watch_dir(path: &str) {
  let (sender, receiver) = channel();
  let mut watcher = watcher(sender, Duration::from_secs(1)).unwrap();
  watcher.watch(path, RecursiveMode::Recursive).unwrap();

  loop {
    match receiver.recv() {
      Ok(event) => println!("{:?}", event),
      Err(e) => println!("watch error: {:?}", e),
    }
  }
}
