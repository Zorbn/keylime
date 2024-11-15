#[macro_export]
macro_rules! defer {
    ($e:expr) => {
        let _defer = DeferredCall::new(|| $e);
    };
}

pub struct DeferredCall<F: FnOnce()> {
    f: Option<F>,
}

impl<F: FnOnce()> DeferredCall<F> {
    pub fn new(f: F) -> Self {
        Self { f: Some(f) }
    }
}

impl<F: FnOnce()> Drop for DeferredCall<F> {
    fn drop(&mut self) {
        self.f.take().unwrap()();
    }
}
