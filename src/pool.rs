use std::{
    cell::RefCell,
    ffi::OsStr,
    fmt::Debug,
    hash::{Hash, Hasher},
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
    thread::LocalKey,
};

use serde::{Serialize, Serializer};

macro_rules! define_pool {
    ($name:ident, $items_name:ident, $type_name:ty) => {
        thread_local! {
            static $items_name: RefCell<Vec<$type_name>> = RefCell::new(Vec::new());
        }

        pub static $name: Pool<$type_name> = Pool::<$type_name>::new(&$items_name);
    };
}

define_pool!(STRING_POOL, STRING_POOL_ITEMS, String);
define_pool!(PATH_POOL, PATH_POOL_ITEMS, PathBuf);
define_pool!(UTF16_POOL, UTF16_POOL_ITEMS, Vec<u16>);

macro_rules! format_pooled {
    ($($arg:tt)*) => {{
        use std::fmt::Write;

        let mut string = crate::pool::STRING_POOL.new_item();
        let _ = write!(string, $($arg)*);

        string
    }};
}

pub(crate) use format_pooled;

pub trait Poolable {
    fn clear(&mut self);
    fn push(&mut self, other: &Self);
}

impl Poolable for String {
    fn clear(&mut self) {
        self.clear();
    }

    fn push(&mut self, other: &Self) {
        self.push_str(other);
    }
}

impl Poolable for PathBuf {
    fn clear(&mut self) {
        self.clear();
    }

    fn push(&mut self, other: &Self) {
        self.push(other);
    }
}

impl Poolable for Vec<u16> {
    fn clear(&mut self) {
        self.clear();
    }

    fn push(&mut self, other: &Self) {
        self.extend_from_slice(other);
    }
}

pub struct Pooled<T: Default + Poolable + 'static> {
    pool: &'static Pool<T>,
    item: Option<T>,
}

impl<T: Default + Poolable + 'static> Pooled<T> {
    pub fn new(pool: &'static Pool<T>) -> Self {
        Self {
            pool,
            item: Some(Default::default()),
        }
    }

    pub fn from(item: T, pool: &'static Pool<T>) -> Self {
        Self {
            pool,
            item: Some(item),
        }
    }
}

impl<T: Default + Poolable + 'static> Drop for Pooled<T> {
    fn drop(&mut self) {
        let item = self.item.take().unwrap();

        self.pool.items.with(|items| {
            let mut items = items.borrow_mut();
            items.push(item);
        });
    }
}

impl<T: Default + Poolable + 'static> Clone for Pooled<T> {
    fn clone(&self) -> Self {
        let mut clone = Self::new(self.pool);
        clone.push(self);

        clone
    }
}

impl<T: Default + Poolable + 'static> Deref for Pooled<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.item.as_ref().unwrap()
    }
}

impl<T: Default + Poolable + 'static> DerefMut for Pooled<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.item.as_mut().unwrap()
    }
}

impl<T: Debug + Default + Poolable + 'static> Debug for Pooled<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.deref().fmt(f)
    }
}

impl<T: Hash + Default + Poolable + 'static> Hash for Pooled<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.deref().hash(state);
    }
}

impl<T: PartialEq + Default + Poolable + 'static> PartialEq for Pooled<T> {
    fn eq(&self, other: &Self) -> bool {
        self.deref().eq(other.deref())
    }
}

impl<T: Eq + Default + Poolable + 'static> Eq for Pooled<T> {}

impl<T: Serialize + Debug + Default + Poolable + 'static> Serialize for Pooled<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.deref().serialize(serializer)
    }
}

impl AsRef<Path> for Pooled<PathBuf> {
    fn as_ref(&self) -> &Path {
        self.deref()
    }
}

impl AsRef<OsStr> for Pooled<String> {
    fn as_ref(&self) -> &OsStr {
        OsStr::new(self.deref())
    }
}

pub struct Pool<T: Default + Poolable + 'static> {
    items: &'static LocalKey<RefCell<Vec<T>>>,
}

impl<T: Default + Poolable + 'static> Pool<T> {
    pub const fn new(items: &'static LocalKey<RefCell<Vec<T>>>) -> Self {
        Self { items }
    }

    pub fn new_item(&'static self) -> Pooled<T> {
        let mut item = None;

        self.items.with(|items| {
            let mut items = items.borrow_mut();
            item = items.pop();
        });

        if let Some(mut item) = item {
            item.clear();

            return Pooled::<T>::from(item, self);
        }

        Pooled::<T>::new(self)
    }

    pub fn init_item(&'static self, mut init_fn: impl FnMut(&mut Pooled<T>)) -> Pooled<T> {
        let mut item = self.new_item();
        init_fn(&mut item);

        item
    }
}
