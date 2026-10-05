//! Byte-bounded LRU cache of decoded preview frames, keyed by frame index.

use std::collections::HashMap;
use std::sync::Arc;

use egui::ColorImage;

pub struct FrameCache {
    frames: HashMap<u64, (Arc<ColorImage>, u64)>,
    tick: u64,
    bytes: usize,
    budget: usize,
}

impl FrameCache {
    pub fn new(budget_bytes: usize) -> Self {
        Self {
            frames: HashMap::new(),
            tick: 0,
            bytes: 0,
            budget: budget_bytes,
        }
    }

    fn size_of(img: &ColorImage) -> usize {
        img.pixels.len() * 4
    }

    pub fn get(&mut self, index: u64) -> Option<Arc<ColorImage>> {
        self.tick += 1;
        let tick = self.tick;
        self.frames.get_mut(&index).map(|(img, t)| {
            *t = tick;
            img.clone()
        })
    }

    pub fn contains(&self, index: u64) -> bool {
        self.frames.contains_key(&index)
    }

    pub fn insert(&mut self, index: u64, img: Arc<ColorImage>) {
        self.tick += 1;
        let size = Self::size_of(&img);
        if let Some((old, _)) = self.frames.insert(index, (img, self.tick)) {
            self.bytes -= Self::size_of(&old);
        }
        self.bytes += size;
        while self.bytes > self.budget && self.frames.len() > 1 {
            let Some((&oldest, _)) = self.frames.iter().min_by_key(|(_, (_, t))| *t) else {
                break;
            };
            if let Some((img, _)) = self.frames.remove(&oldest) {
                self.bytes -= Self::size_of(&img);
            }
        }
    }

    pub fn clear(&mut self) {
        self.frames.clear();
        self.bytes = 0;
    }

    pub fn len(&self) -> usize {
        self.frames.len()
    }

    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    pub fn bytes(&self) -> usize {
        self.bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn img() -> Arc<ColorImage> {
        Arc::new(ColorImage::new([10, 10], vec![egui::Color32::BLACK; 100]))
    }

    #[test]
    fn evicts_least_recently_used() {
        let mut c = FrameCache::new(400 * 3);
        c.insert(1, img());
        c.insert(2, img());
        c.insert(3, img());
        assert!(c.get(1).is_some()); // touch 1
        c.insert(4, img()); // evicts 2
        assert!(c.contains(1));
        assert!(!c.contains(2));
        assert!(c.contains(3) && c.contains(4));
        assert_eq!(c.bytes(), 1200);
    }
}
