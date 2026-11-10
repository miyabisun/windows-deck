//! A long list's pictures: only those used lately are kept, so a list scrolled to its end does
//! not hold every picture it passed. One let go is read again when the list comes back to it.

use std::{collections::HashMap, sync::Arc};

use gpui::{
    App, AppContext as _, Entity, ImageCache, ImageCacheError, ImageCacheItem, RenderImage,
    Resource, Window, hash,
};

use crate::model::recent::Recent;

pub struct RecentImages {
    order: Recent<u64>,
    items: HashMap<u64, ImageCacheItem>,
}

impl RecentImages {
    /// A cache of `capacity` pictures (more only while a single screen draws more).
    pub fn new(capacity: usize, cx: &mut App) -> Entity<Self> {
        let cache = cx.new(|_| Self {
            order: Recent::new(capacity),
            items: HashMap::new(),
        });
        // The windows hold a texture for each picture drawn; give them back with the cache.
        cx.observe_release(&cache, |cache, cx| {
            for (_, item) in cache.items.drain() {
                if let Some(Ok(image)) = item.get() {
                    cx.drop_image(image, None);
                }
            }
        })
        .detach();
        cache
    }

    /// Call when the list is about to be drawn: what this frame and the last one use is kept.
    pub fn next_frame(&mut self) {
        self.order.next_frame();
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl ImageCache for RecentImages {
    fn load(
        &mut self,
        resource: &Resource,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Result<Arc<RenderImage>, ImageCacheError>> {
        let key = hash(resource);
        for old in self.order.touch(key) {
            if let Some(item) = self.items.remove(&old)
                && let Some(Ok(image)) = item.get()
            {
                cx.drop_image(image, Some(window));
            }
        }
        self.items
            .entry(key)
            .or_insert_with(|| ImageCacheItem::new(resource, cx))
            .use_image(window)
    }
}
