extern crate futures;
extern crate bus;

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
    type Key: Key + Send + Sync; // Type of key used to load resource
    /// The error type returned when the resource loader fails to load/create the resource
    type Error: Send + Sync + 'static; // How we indicate loading failed
    /// Loads/creates the resource
    fn load<K: Into<Self::Key>>(&self, k: K) -> Result<R, Self::Error>;
}

use std::rc::Rc;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use bus::{Bus, BusReader};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel, TryRecvError};

pub enum ResourceFuture<R, L: ResourceProvider<R>> {
    InProgress { succ: BusReader<Arc<R>>, fail: Receiver<L::Error> },
    Successful(Arc<R>),
    Failed(Rc<L::Error>)
}

impl<R, L: ResourceProvider<R>> ResourceFuture<R, L> {
    fn new(s: BusReader<Arc<R>>, f: Receiver<L::Error>) -> ResourceFuture<R, L> {
        ResourceFuture::InProgress {
            succ: s,
            fail: f}
    }
}

enum SEN<S, E> {
    Succ(S),
    Err(E),
    Null
}

use futures::{Future, Poll, Async};
impl<R, L: ResourceProvider<R>> Future for ResourceFuture<R, L> {
    type Item = Arc<R>;
    type Error = Rc<L::Error>;

    fn poll(&mut self) -> Poll<Arc<R>, Rc<L::Error>> {
        match *self {
            ResourceFuture::Successful(ref a) => return Ok(Async::Ready(a.clone())),
            ResourceFuture::Failed(ref e) => return Err(e.clone()),
            _ => ()
        };
        let mut sen = SEN::Null;
        let res;
        {
            let (mut sc, fc) = match *self {
                ResourceFuture::InProgress{ref mut succ, ref fail} => (succ, fail),
                _ => unreachable!()
            };
            res = match sc.try_recv() {
                Ok(r) => {
                    sen = SEN::Succ(r.clone());
                    Ok(Async::Ready(r))
                },
                Err(_) => match fc.try_recv() {
                    Ok(e) => {
                        let rce = Rc::new(e);
                        sen = SEN::Err(rce.clone());
                        Err(rce)
                    },
                    Err(_) => Ok(Async::NotReady)
                }
            };
        }
        match sen {
            SEN::Null => (),
            SEN::Succ(r) => *self = ResourceFuture::Successful(r),
            SEN::Err(e) => *self = ResourceFuture::Failed(e)
        };
        res

    }
}

/// A store of loaded resources
pub struct Cache<R> {
    // TODO: Provide Resource keeping strategies
    cache: HashMap<CacheId, Arc<R>>,
    items_to_cache: Vec<(CacheId, Rc<BusReader<Arc<R>>>)>,

    // TODO: Add handling for async requests using futures and (maybe) bus
}

impl<R> Cache<R> {
    /// Creates a new Cache
    pub fn new() -> Cache<R> {
        Cache {
            cache: HashMap::new(),
            items_to_cache: vec![],
        }
    }

    /// Use the specified ResourceLoader to load/create a resource, and cache it
    pub fn load<L: ResourceProvider<R>>(&mut self, provider: &L, key: L::Key) -> Result<Arc<R>, L::Error> {
        let cache_id = key.as_cache_id();
        match self.cache.entry(cache_id) {
            Entry::Occupied(occ) => Ok(occ.get().clone()),
            Entry::Vacant(vac) => {
                let pointer = match provider.load(key) {
                    Ok(resource) => Arc::new(resource),
                    Err(e) => return Err(e)
                };
                vac.insert(pointer.clone());
                Ok(pointer)
            }
        }
    }

    /// Checks on asynchrous cache requests, and caches them if available
    pub fn cache_async(&mut self) {
        let itc = self.items_to_cache.clone();
        self.items_to_cache = vec![]; // Delete the copies of Rc, essentially moving them.
        let temp = itc.into_iter().filter_map(|(k, mut x)| match Rc::get_mut(&mut x).unwrap().try_recv() {
            Ok(item) => {match self.cache.entry(k) {
                Entry::Occupied(_) => (),
                Entry::Vacant(vac) => {vac.insert(item);}
            }; None}, // We have the item.
            Err(TryRecvError::Empty) => Some((k, x)), // It's not in (yet).
            Err(TryRecvError::Disconnected) => None // It's never coming in.
        }).collect::<Vec<_>>();
        self.items_to_cache = temp;
    }

    /// Use the specifed ResourceLoader to load/create a resource on a separate thread, and cache it if successful
    pub fn load_async<L: ResourceProvider<R> + Send + Sync>(&mut self, provider: &L, key: L::Key) -> Result<Arc<R>, ResourceFuture<R, L>> where R: Send + Sync + 'static, L: Clone + 'static {
        use std::thread;

        let (ftx, frx) = channel();
        let mut resource_bus = Bus::new(2);
        let cache_id = key.as_cache_id();
        match self.cache.entry(cache_id.clone()) {
            Entry::Occupied(occ) => Ok(occ.get().clone()),
            Entry::Vacant(_) => {
                let cache_bus = resource_bus.add_rx();
                let future_bus = resource_bus.add_rx();
                let resource_handler = provider.clone();
                thread::spawn(move || {
                    match resource_handler.load(key) {
                        Ok(resource) => resource_bus.broadcast(Arc::new(resource)),
                        Err(e) => ftx.send(e).unwrap()
                    };
                });
                self.items_to_cache.push((cache_id.clone(), Rc::new(cache_bus)));
                Err(ResourceFuture::new(future_bus, frx))
            }
        }
    }
}
