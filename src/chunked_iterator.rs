pub struct ChunkedIterator<T, R>
where
  T: Iterator<Item = R>,
{
  source: T,
  inner: Vec<R>,
  size: usize,
}

impl<T, R> ChunkedIterator<T, R>
where
  T: Iterator<Item = R>,
{
  pub fn new(source: T, size: usize) -> Self {
    ChunkedIterator {
      size: size - 1,
      inner: vec![],
      source,
    }
  }
}
impl<T, R> Iterator for ChunkedIterator<T, R>
where
  T: Iterator<Item = R>,
{
  type Item = Vec<R>;

  fn next(&mut self) -> Option<Vec<R>> {
    loop {
      match self.source.next() {
        Some(inner_item) => {
          self.inner.push(inner_item);
          if self.inner.len() > self.size {
            return Some(self.inner.split_off(0));
          }
        }
        None => {
          if self.inner.is_empty() {
            return None;
          }
          return Some(self.inner.split_off(0));
        }
      }
    }
  }
}
