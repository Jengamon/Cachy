extern crate cachy;
extern crate futures;

use cachy::{Cache, ResourceProvider};
use std::sync::Arc;

#[derive(Debug)]
struct GenericResource(i32);
#[derive(Clone)]
struct GResourceLoader;
impl ResourceProvider<GenericResource> for GResourceLoader {
    type Key = i32;
    type Error = ();
    fn load<K: Into<i32>>(&self, k: K) -> Result<GenericResource, ()> {
        Ok(GenericResource(k.into()))
    }
}

#[derive(Debug)]
struct TooLowError(i32); // Number was too high
#[derive(Clone)]
struct GResourceLoader2 {
    cutoff: i32,
}
impl ResourceProvider<GenericResource> for GResourceLoader2 {
    type Key = i32;
    type Error = TooLowError;
    fn load<K: Into<i32>>(&self, k: K) -> Result<GenericResource, TooLowError> {
        let i = k.into();
        if i <= self.cutoff {
            Err(TooLowError(i))
        } else {
            Ok(GenericResource(i))
        }
    }
}

#[test]
fn load_resource() {
    let loader = GResourceLoader;
    let mut cache = Cache::new();
    let resource: Result<Arc<GenericResource>, _> = cache.load(&loader, 1);
    assert!(resource.is_ok());
    assert!(resource.unwrap().0 == 1);
}

#[test]
fn load_resource_error() {
    let loader = GResourceLoader2 {
        cutoff: 0
    };
    let mut cache = Cache::new();
    let resource = cache.load(&loader, -1);
    assert!(resource.is_err());
    assert!(resource.unwrap_err().0 == -1);
}

#[test]
fn load_resource_async() {
    let loader = GResourceLoader;
    let mut cache = Cache::new();
    let resource = cache.load_async(&loader, 1);
    assert!(resource.is_err());
    let mut future = resource.err().unwrap();

    use futures::{Future, Async};
    while let Ok(p) = future.poll() {
        if p.is_not_ready() { continue }
        match p {
            Async::NotReady => unreachable!(),
            Async::Ready(resource) => {
                // assert!(resource.is_ok());
                assert!(resource.0 == 1);
                cache.cache_async();
                return;
            }
        }
    }
    assert!(false); // It's not supposed to reach here.
    // let resource = future.wait();
}

#[test]
fn load_resource_async_error() {
    let loader = GResourceLoader2 {cutoff: 0};
    let mut cache = Cache::new();
    let resource = cache.load_async(&loader, -1);
    assert!(resource.is_err());
    let mut future = resource.err().unwrap();

    use futures::{Future, Async};
    loop {
        match future.poll() {
            Ok(p) => {
                if p.is_not_ready() { continue }
                match p {
                    Async::NotReady => unreachable!(),
                    Async::Ready(_) => assert!(false), // It's not supposed to go here.
                }
            },
            Err(e) => {
                assert!(e.0 == -1);
                cache.cache_async();
                return;
            }
        }
    }
}
