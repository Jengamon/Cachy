extern crate cachy;

use cachy::{Cache, ResourceProvider};
use std::rc::Rc;

#[derive(Debug)]
struct GenericResource(i32);
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
    let resource: Result<Rc<GenericResource>, _> = cache.load(&loader, 1);
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

#[ignore]
#[test]
fn load_resource_async() {
    let loader = GResourceLoader;
    assert!(false); // TODO write this test
}
