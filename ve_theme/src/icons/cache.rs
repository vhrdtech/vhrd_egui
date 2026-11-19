//! The per-context cache of icon masks as textures: one entry per (icon, size in pixels), each holding a
//! white coverage texture per layer. Colour is not part of the key: a theme switch only changes the tint.
//!
//! Bounded by texture bytes ([`CACHE_BYTES_MAX`]): past it the least recently used entries go. An entry not
//! drawn for [`CACHE_IDLE_PASSES_MAX`] passes goes too, so the sizes of a scale no longer in use are dropped.

use std::collections::HashMap;
use std::sync::Arc;

use egui::mutex::Mutex;
use egui::{Color32, ColorImage, Context, Id, TextureHandle, TextureOptions};

use super::raster::{IconError, rasterise};
use super::{Slot, SvgIcon};

/// Texture bytes the icon cache holds at most (RGBA, 4 bytes a texel).
pub const CACHE_BYTES_MAX: usize = 32 * 1024 * 1024;

/// An entry not drawn for this many passes is dropped.
pub const CACHE_IDLE_PASSES_MAX: u64 = 3600;

/// The filter the masks are sampled with: they are drawn texel for pixel, so no texel is ever blended.
pub(crate) const MASK_FILTER: TextureOptions = TextureOptions::NEAREST;

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
struct Key {
    /// The SVG text's address and length: a `&'static str` is one place in the binary.
    svg: usize,
    len: usize,
    px: u32,
}

/// One rasterised icon: its layers' textures in paint order, or the error, kept so a bad icon is parsed once.
pub(crate) type Masks = Arc<Result<Vec<(Slot, TextureHandle)>, String>>;

struct Entry {
    masks: Masks,
    bytes: usize,
    used_pass: u64,
}

struct Cache {
    entries: HashMap<Key, Entry>,
    bytes: usize,
    bytes_max: usize,
    rasters: u64,
}

impl Cache {
    fn new(bytes_max: usize) -> Self {
        Self {
            entries: HashMap::new(),
            bytes: 0,
            bytes_max,
            rasters: 0,
        }
    }

    fn get(&mut self, key: &Key, pass: u64) -> Option<Masks> {
        let e = self.entries.get_mut(key)?;
        e.used_pass = pass;
        Some(Arc::clone(&e.masks))
    }

    /// Insert, then drop idle entries and, while over the byte bound, the least recently used ones.
    fn insert(&mut self, key: Key, masks: Masks, bytes: usize, pass: u64) {
        self.rasters += 1;
        let entry = Entry {
            masks,
            bytes,
            used_pass: pass,
        };
        if let Some(old) = self.entries.insert(key, entry) {
            self.bytes -= old.bytes;
        }
        self.bytes += bytes;
        let idle_before = pass.saturating_sub(CACHE_IDLE_PASSES_MAX);
        self.entries.retain(|_, e| {
            let keep = e.used_pass >= idle_before;
            if !keep {
                self.bytes -= e.bytes;
            }
            keep
        });
        while self.bytes > self.bytes_max {
            let oldest = self
                .entries
                .iter()
                .filter(|(k, _)| **k != key)
                .min_by_key(|(_, e)| e.used_pass)
                .map(|(k, _)| *k);
            let Some(oldest) = oldest else { break };
            if let Some(e) = self.entries.remove(&oldest) {
                self.bytes -= e.bytes;
            }
        }
    }
}

#[derive(Clone)]
struct Shared(Arc<Mutex<Cache>>);

impl Default for Shared {
    fn default() -> Self {
        Self(Arc::new(Mutex::new(Cache::new(CACHE_BYTES_MAX))))
    }
}

fn shared(ctx: &Context) -> Shared {
    ctx.data_mut(|d| {
        d.get_temp_mut_or_default::<Shared>(Id::new("ve_theme::icons::cache"))
            .clone()
    })
}

fn upload(ctx: &Context, icon: &SvgIcon, px: u32) -> Result<Vec<(Slot, TextureHandle)>, IconError> {
    let side = px as usize;
    Ok(rasterise(icon.svg, px)?
        .into_iter()
        .enumerate()
        .map(|(i, m)| {
            let pixels = m
                .alpha
                .iter()
                .map(|&a| Color32::from_white_alpha(a))
                .collect();
            let image = ColorImage::new([side, side], pixels);
            let name = format!("ve_icon:{}:{px}:{i}:{}", icon.name, m.slot.name());
            (m.slot, ctx.load_texture(name, image, MASK_FILTER))
        })
        .collect())
}

/// The masks of `icon` at `px` pixels, rasterised on the first call.
pub(crate) fn masks(ctx: &Context, icon: &SvgIcon, px: u32) -> Masks {
    let key = Key {
        svg: icon.svg.as_ptr() as usize,
        len: icon.svg.len(),
        px,
    };
    let pass = ctx.cumulative_pass_nr();
    let cache = shared(ctx);
    if let Some(m) = cache.0.lock().get(&key, pass) {
        return m;
    }
    // Rasterised and uploaded with no lock held: `load_texture` takes the context's lock.
    let result = upload(ctx, icon, px).map_err(|e| {
        log::warn!("icon {}: {e}", icon.name);
        e.to_string()
    });
    let layers = result.as_ref().map_or(0, Vec::len);
    let bytes = layers * px as usize * px as usize * 4;
    let masks = Arc::new(result);
    cache.0.lock().insert(key, Arc::clone(&masks), bytes, pass);
    masks
}

/// How many times an icon was rasterised in `ctx` so far (each a first use of an icon at a pixel size).
pub fn raster_count(ctx: &Context) -> u64 {
    shared(ctx).0.lock().rasters
}

/// Texture bytes the icon cache of `ctx` holds now.
pub fn cache_bytes(ctx: &Context) -> usize {
    shared(ctx).0.lock().bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(px: u32) -> Key {
        Key { svg: 1, len: 1, px }
    }

    fn masks() -> Masks {
        Arc::new(Ok(Vec::new()))
    }

    /// Past the byte bound the least recently used entry goes, never the one just added.
    #[test]
    fn the_bound_evicts_the_least_recently_used() {
        let mut c = Cache::new(300);
        c.insert(key(1), masks(), 100, 1);
        c.insert(key(2), masks(), 100, 2);
        c.insert(key(3), masks(), 100, 3);
        assert!(c.get(&key(1), 4).is_some(), "key 1 used again at pass 4");
        c.insert(key(4), masks(), 100, 5);
        assert!(c.entries.contains_key(&key(1)));
        assert!(!c.entries.contains_key(&key(2)), "key 2 is the oldest");
        assert_eq!(c.bytes, 300);
        c.insert(key(5), masks(), 1000, 6);
        assert_eq!(
            c.entries.len(),
            1,
            "one entry over the bound keeps only itself"
        );
        assert_eq!(c.bytes, 1000);
    }

    /// Sizes not drawn for a while (an old scale) are dropped when another one is added.
    #[test]
    fn idle_entries_are_dropped() {
        let mut c = Cache::new(usize::MAX);
        c.insert(key(16), masks(), 10, 1);
        c.insert(key(20), masks(), 10, 2 + CACHE_IDLE_PASSES_MAX);
        assert!(!c.entries.contains_key(&key(16)));
        assert_eq!(c.bytes, 10);
    }
}
