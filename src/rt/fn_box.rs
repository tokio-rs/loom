pub trait FnBox {
    fn call(self: Box<Self>);
}

impl<T> FnBox for T
where
    T: FnOnce(),
{
    fn call(self: Box<Self>) {
        self()
    }
}
