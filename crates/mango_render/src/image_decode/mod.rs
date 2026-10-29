//! Image decoding: data URI parsing and PNG/JPEG/GIF/WebP image decoding.

pub mod cache;
pub mod decoder;
pub mod image;

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) use cache::BoundedImageStore;
pub use cache::{
    cache_animated_image, cache_image, cache_scaled_image, decode_image_async, get_animated_frame,
    get_cached_animated_image, get_cached_image, get_or_decode_async, get_or_resize_cached,
    get_scaled_image, prune_image_caches,
};
pub use decoder::{
    base64_decode, base64_encode, create_placeholder, decode_animated_gif,
    decode_animated_image_bytes, decode_data_uri, decode_image_bytes,
    decode_progressive_jpeg_scans, is_progressive_jpeg, percent_decode,
};
pub use image::{AnimatedImage, AnimationFrame, DecodedImage, ProgressiveScan};
