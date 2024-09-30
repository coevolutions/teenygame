pub(crate) mod audio;
pub(crate) mod font;
pub(crate) mod image;
pub(crate) mod raw;

pub use audio::Audio;
pub use font::Font;
pub use image::{Image, ImageAndMetadata};
pub use raw::Raw;

use std::{
    future::Future,
    sync::{Arc, Mutex},
};

use crate::{
    futures,
    marker::{MaybeSend, MaybeSync},
};

pub struct Asset<T>(Arc<Mutex<AssetState<T>>>);

impl<T> Clone for Asset<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

pub trait Loadable
where
    Self: Sized + MaybeSend + MaybeSync + 'static,
{
    fn load(path: &str) -> impl Future<Output = Result<Self, anyhow::Error>> + MaybeSend;
}

pub fn load<T>(path: &str) -> Asset<T>
where
    T: Loadable,
{
    let r = Asset::pending();
    {
        let path = path.to_string();
        let r = r.clone();

        let fut = async move {
            let res = T::load(&path)
                .await
                .map_err(|e| Arc::new(e))
                .map(|v| Arc::new(v));
            *r.0.lock().unwrap() = Some(res);
        };

        futures::spawn(fut);
    }
    r
}

type Error = Arc<anyhow::Error>;

type AssetState<T> = Option<Result<Arc<T>, Error>>;

impl<T> Asset<T> {
    fn pending() -> Self {
        Self(Arc::new(Mutex::new(None)))
    }

    pub fn get(&self) -> AssetState<T> {
        self.0.lock().unwrap().clone()
    }
}

trait AnyAsset {
    fn status(&self) -> Option<Result<(), Error>>;
}

impl<T> AnyAsset for Asset<T> {
    fn status(&self) -> Option<Result<(), Error>> {
        self.0
            .lock()
            .unwrap()
            .as_ref()
            .map(|r| r.as_ref().map(|_| ()).map_err(|e| e.clone()))
    }
}

pub struct AssetLoadTracker {
    assets: Vec<Box<dyn AnyAsset>>,
}

impl AssetLoadTracker {
    pub fn new() -> Self {
        Self { assets: vec![] }
    }

    pub fn add<T>(&mut self, asset: &Asset<T>)
    where
        Asset<T>: 'static,
    {
        self.assets.push(Box::new(asset.clone()));
    }

    pub fn len(&self) -> usize {
        self.assets.len()
    }

    pub fn num_loaded(&self) -> Result<usize, Error> {
        self.assets.iter().try_fold(0, |acc, x| {
            Ok(acc
                + match x.status() {
                    Some(Ok(())) => 1,
                    Some(Err(e)) => {
                        return Err(e);
                    }
                    None => 0,
                })
        })
    }
}

pub trait Metadata
where
    Self: Sized,
{
    fn load(raw: &[u8]) -> Result<Self, anyhow::Error>;
}
