use jwalk::WalkDir;

pub fn read_dir() {
  println!("Hello");

  for entry in WalkDir::new("/home/cdiesh/Music") {
    println!("{}", entry.unwrap().path().display());
  }
}
