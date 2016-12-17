// extern crate futures;
// extern crate bus;

/// Something that can be used to find/create a resource
pub trait Key {
    fn as_cache_id(&self) -> CacheId;
}

// TODO: Make a more generic strategy for keys
/// The identifier that the cache uses to make resources unique
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum CacheId {
    USize(usize),
    U32(u32),
    U64(u64),
    ISize(isize),
    I32(i32),
    I64(i64),
    Str(String),
}

impl Key for u32 { fn as_cache_id(&self) -> CacheId { CacheId::U32(*self) } }
impl Key for u64 { fn as_cache_id(&self) -> CacheId { CacheId::U64(*self) } }
impl Key for usize { fn as_cache_id(&self) -> CacheId { CacheId::USize(*self) } }
impl Key for i32 { fn as_cache_id(&self) -> CacheId { CacheId::I32(*self) } }
impl Key for i64 { fn as_cache_id(&self) -> CacheId { CacheId::I64(*self) } }
impl Key for isize { fn as_cache_id(&self) -> CacheId { CacheId::ISize(*self) } }
impl Key for String { fn as_cache_id(&self) -> CacheId { CacheId::Str(self.clone()) } }

/// Something that provides a resource
pub trait ResourceProvider<R> {
    /// The key this loader uses to load/create the resource
    type Key: Key; // Type of key used to load resource
    /// The error type returned when the resource loader fails to load/create the resource
    type Error; // How we indicate loading failed
    /// Loads/creates the resource
    fn load<K: Into<Self::Key>>(&self, k: K) -> Result<R, Self::Error>;
}

use std::rc::Rc;
use std::collections::HashMap;
use std::collections::hash_map::Entry;

/// A store of loaded resources
pub struct Cache<R> {
    // TODO: Provide Resource keeping strategies
    cache: HashMap<CacheId, Rc<R>>,

    // TODO: Add handling for async requests using futures and (maybe) bus
}

impl<R> Cache<R> {
    /// Creates a new Cache
    pub fn new() -> Cache<R> {
        Cache {
            cache: HashMap::new(),
        }
    }

    /// Use the specified ResourceLoader to load/create an resource, and cache it
    pub fn load<L: ResourceProvider<R>>(&mut self, provider: &L, key: L::Key) -> Result<Rc<R>, L::Error> {
        let cache_id = key.as_cache_id();
        match provider.load(key) {
            Ok(resource) => {
                let pointer = Rc::new(resource);
                match self.cache.entry(cache_id) {
                    Entry::Occupied(occ) => Ok(occ.get().clone()),
                    Entry::Vacant(vac) => {
                        vac.insert(pointer.clone());
                        Ok(pointer)
                    },
                }
            },
            Err(e) => Err(e)
        }
    }
}
