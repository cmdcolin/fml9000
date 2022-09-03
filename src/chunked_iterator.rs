struct ChunkedIterator<T, R>
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
  fn new(source: T, size: usize) -> Self {
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
    while let inner_opt = self.source.next() {
      match inner_opt {
        Some(inner_item) => {
          self.inner.push(inner_item);
          if self.inner.len() > self.size {
            return Some(self.inner.split_off(0));
          }
        }
        None => match self.inner.len() {
          0 => return None,
          _ => return Some(self.inner.split_off(0)),
        },
      }
    }
    None
  }
}
