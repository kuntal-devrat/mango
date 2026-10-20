pub mod builder;
pub mod hit_test;
pub mod media;
pub mod node;
pub mod replaced;
pub mod scroll;
pub mod types;

#[cfg(test)]
mod tests;

pub use builder::{
    build_box_tree, build_box_tree_with_maps, collect_child_boxes, collect_child_boxes_with_maps,
};
pub use media::{
    build_audio_element, build_audio_element_with_maps, build_canvas_element, build_iframe_element,
    build_iframe_element_with_maps, build_video_element, build_video_element_with_maps,
};
pub use replaced::{
    build_embed_element, build_replaced_element, build_replaced_element_with_maps,
    build_svg_replaced_element, parse_srcset_candidates, try_build_object_element,
    try_build_object_element_with_maps,
};
pub use types::{
    AreaShape, FormControlHit, LayoutBox, MapArea, MediaClickAction, MediaControlHit,
    clear_image_maps, get_image_map, register_image_map,
};
