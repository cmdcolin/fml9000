use jwalk::WalkDir;

pub fn read_dir(path: &str) {
  println!("Hello");

  for entry in WalkDir::new(path) {
    println!("{}", entry.unwrap().path().display());
  }
}
