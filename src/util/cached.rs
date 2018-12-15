use std::ops;

#[derive(Debug, Serialize, Deserialize)]
pub struct CachedVec<T> {
    inner: Vec<T>,
    len: usize,
}

impl<T: Default> CachedVec<T> {
    pub fn new() -> CachedVec<T> {
        CachedVec {
            inner: vec![],
            len: 0,
        }
    }

    pub fn push<F>(&mut self, f: F)
    where
        F: FnOnce(&mut T)
    {
        if self.len == self.inner.len() {
            self.inner.push(T::default());
        }

        f(&mut self.inner[self.len]);
        self.len += 1;
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn pop(&mut self) {
        self.len -= 1;
    }
}

impl<T> ops::Deref for CachedVec<T> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        &self.inner[..self.len]
    }
}

impl<T> ops::DerefMut for CachedVec<T> {
    fn deref_mut(&mut self) -> &mut [T] {
        &mut self.inner[..self.len]
    }
}
