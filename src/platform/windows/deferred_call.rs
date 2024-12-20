macro_rules! defer {
    ($e:expr) => {
        let _defer = $crate::platform::platform_impl::deferred_call::DeferredCall::new(|| $e);
    };
}

pub(super) use defer;

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
