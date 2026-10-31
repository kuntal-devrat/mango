use std::collections::HashMap;

use mango_css::values::{Clear, Display, Float, Length, Position};

use crate::box_model::BoxType;
use crate::style_tree::StyledNode;

use super::media::{
    build_audio_element_with_maps, build_canvas_element, build_iframe_element_with_maps,
    build_video_element_with_maps,
};
use super::replaced::{
    build_embed_element, build_replaced_element_with_maps, build_svg_replaced_element,
    try_build_object_element_with_maps,
};
use super::types::{LayoutBox, MapArea};

#[allow(dead_code)]
pub fn collect_child_boxes(node: &StyledNode, out: &mut Vec<LayoutBox>) {
    collect_child_boxes_with_maps(node, out, None);
}

pub fn collect_child_boxes_with_maps(
    node: &StyledNode,
    out: &mut Vec<LayoutBox>,
    maps: Option<&HashMap<String, Vec<MapArea>>>,
) {
    if node.style.display == Display::Contents {
        for child in &node.children {
            collect_child_boxes_with_maps(child, out, maps);
        }
    } else {
        out.push(build_box_tree_with_maps(node, maps));
    }
}

/// Builds the complete layout box tree from a styled node.
pub fn build_box_tree(styled_node: &StyledNode) -> LayoutBox {
    build_box_tree_with_maps(styled_node, None)
}

/// Builds the complete layout box tree from a styled node with an optional explicit image map registry.
pub fn build_box_tree_with_maps(
    styled_node: &StyledNode,
    maps: Option<&HashMap<String, Vec<MapArea>>>,
) -> LayoutBox {
    // Detect <img> elements — generate a ReplacedElement box
    if styled_node.tag_name.as_deref() == Some("img") {
        return build_replaced_element_with_maps(styled_node, maps);
    }

    // Detect <embed> elements — generate a ReplacedElement box or fallback container
    if styled_node.tag_name.as_deref() == Some("embed") {
        return build_embed_element(styled_node);
    }

    // Detect <object> elements — decode media or gracefully fall back to child nodes
    if styled_node.tag_name.as_deref() == Some("object")
        && let Some(obj_box) = try_build_object_element_with_maps(styled_node, maps)
    {
        return obj_box;
    }

    // Detect <svg> elements — generate a ReplacedElement box with vector rasterization
    if styled_node.tag_name.as_deref() == Some("svg") {
        return build_svg_replaced_element(styled_node);
    }

    // Detect <iframe> elements — generate a nested browsing context box
    if styled_node.tag_name.as_deref() == Some("iframe") {
        return build_iframe_element_with_maps(styled_node, maps);
    }

    // Detect <video> elements — generate a Video replaced box
    if styled_node.tag_name.as_deref() == Some("video") {
        return build_video_element_with_maps(styled_node, maps);
    }

    // Detect <audio> elements — generate an Audio replaced box
    if styled_node.tag_name.as_deref() == Some("audio") {
        return build_audio_element_with_maps(styled_node, maps);
    }

    // Detect <canvas> elements — generate a Canvas replaced box
    if styled_node.tag_name.as_deref() == Some("canvas") {
        return build_canvas_element(styled_node);
    }

    // Detect <select> elements — extract selected option and suppress child option text flow
    if styled_node.tag_name.as_deref() == Some("select") {
        let is_multiple = styled_node
            .attributes
            .iter()
            .any(|(k, _)| k.eq_ignore_ascii_case("multiple"));
        let mut selected_texts = Vec::new();
        let mut first_text = None;

        fn extract_option_text(node: &StyledNode) -> String {
            let mut s = String::new();
            if let Some(ref t) = node.text {
                s.push_str(t);
            }
            for c in &node.children {
                s.push_str(&extract_option_text(c));
            }
            s
        }

        fn collect_options<'a>(node: &'a StyledNode, options: &mut Vec<&'a StyledNode>) {
            for c in &node.children {
                if c.tag_name.as_deref() == Some("option") {
                    options.push(c);
                } else if c.tag_name.as_deref() == Some("optgroup") {
                    collect_options(c, options);
                }
            }
        }

        let mut all_options = Vec::new();
        collect_options(styled_node, &mut all_options);

        for opt in all_options {
            let opt_text = extract_option_text(opt).trim().to_string();
            let is_selected = opt
                .attributes
                .iter()
                .any(|(k, _)| k.eq_ignore_ascii_case("selected"));
            if is_selected && !opt_text.is_empty() {
                selected_texts.push(opt_text.clone());
            }
            if first_text.is_none() && !opt_text.is_empty() {
                first_text = Some(opt_text);
            }
        }

        let box_type = match styled_node.style.display {
            Display::Block => BoxType::BlockNode,
            _ => BoxType::InlineBlock,
        };
        let mut root_box = LayoutBox::new(box_type, Some(styled_node.style.clone()));
        root_box.node_id = styled_node.node_id;
        root_box.tag_name = styled_node.tag_name.clone();
        root_box.attributes = styled_node.attributes.clone();

        if is_multiple {
            root_box
                .attributes
                .push(("_mango_is_multiple".to_string(), "true".to_string()));
            let text = if selected_texts.is_empty() {
                String::new()
            } else {
                selected_texts.join(", ")
            };
            root_box
                .attributes
                .push(("_mango_selected_text".to_string(), text));
        } else if let Some(text) = selected_texts.into_iter().next().or(first_text) {
            root_box
                .attributes
                .push(("_mango_selected_text".to_string(), text));
        }
        return root_box;
    }

    let box_type = if let Some(text) = &styled_node.text {
        BoxType::TextNode(text.clone())
    } else {
        match styled_node.style.display {
            Display::Block
            | Display::FlowRoot
            | Display::Grid
            | Display::Flex
            | Display::Table
            | Display::TableRow
            | Display::TableCell
            | Display::TableCaption
            | Display::TableRowGroup
            | Display::TableHeaderGroup
            | Display::TableFooterGroup
            | Display::TableColumn
            | Display::TableColumnGroup
            | Display::ListItem => BoxType::BlockNode,
            Display::Inline | Display::Ruby | Display::RubyBase | Display::RubyText => {
                BoxType::InlineNode
            }
            Display::InlineBlock | Display::InlineFlex | Display::InlineGrid => {
                BoxType::InlineBlock
            }
            Display::None | Display::Contents => BoxType::AnonymousBlock, // Handled before reaching here
        }
    };

    // Computed styles are cloned onto text nodes for inheritance, but box
    // properties such as position and margin are not inherited. Keeping the
    // ancestor's out-of-flow values on a text box creates stale descendants
    // when an absolute/flex item is relaid out.
    let box_style = if styled_node.text.is_some() {
        let mut style = (*styled_node.style).clone();
        style.display = Display::Inline;
        style.position = Position::Static;
        style.float = Float::None;
        style.clear = Clear::None;
        style.margin_top = Length::Px(0.0);
        style.margin_right = Length::Px(0.0);
        style.margin_bottom = Length::Px(0.0);
        style.margin_left = Length::Px(0.0);
        style.padding_top = Length::Px(0.0);
        style.padding_right = Length::Px(0.0);
        style.padding_bottom = Length::Px(0.0);
        style.padding_left = Length::Px(0.0);
        style.width = Length::Auto;
        style.height = Length::Auto;
        style.min_width = Length::Auto;
        style.min_height = Length::Auto;
        style.max_width = Length::Auto;
        style.max_height = Length::Auto;
        Some(std::sync::Arc::new(style))
    } else {
        Some(styled_node.style.clone())
    };
    let mut root_box = LayoutBox::new(box_type, box_style);
    root_box.node_id = styled_node.node_id;
    root_box.tag_name = styled_node.tag_name.clone();
    root_box.attributes = styled_node.attributes.clone();

    // Recursively build children, hoisting Display::Contents children directly into parent
    let mut raw_children: Vec<LayoutBox> = Vec::new();
    for child in &styled_node.children {
        collect_child_boxes_with_maps(child, &mut raw_children, maps);
    }

    let is_flex = styled_node.style.display == Display::Flex
        || styled_node.style.display == Display::InlineFlex;
    let is_grid = styled_node.style.display == Display::Grid
        || styled_node.style.display == Display::InlineGrid;
    let is_table_context = matches!(
        styled_node.style.display,
        Display::Table
            | Display::TableRow
            | Display::TableRowGroup
            | Display::TableHeaderGroup
            | Display::TableFooterGroup
            | Display::TableColumn
            | Display::TableColumnGroup
    ) || matches!(
        styled_node.tag_name.as_deref(),
        Some("table")
            | Some("tbody")
            | Some("thead")
            | Some("tfoot")
            | Some("tr")
            | Some("colgroup")
    );

    if is_flex || is_grid {
        // In CSS Flexbox and Grid, children become items directly without anonymous block wrapping.
        // Stray text runs must be wrapped in anonymous flex/grid items (CSS Flexbox §4).
        let mut items = Vec::with_capacity(raw_children.len());
        for mut child in raw_children {
            let is_stray_text = matches!(child.box_type, BoxType::TextNode(_));
            if is_stray_text {
                // Drop purely collapsible whitespace; wrap significant text.
                let is_ws = child
                    .text()
                    .map(|t| t.chars().all(|ch| ch.is_ascii_whitespace()))
                    .unwrap_or(true);
                if is_ws {
                    continue;
                }
                let anon_style = create_anonymous_box_style(&styled_node.style, Display::Block);
                let mut anon = LayoutBox::new_anonymous_block(Some(anon_style.into()));
                anon.is_independent_formatting_context = true;
                child.is_independent_formatting_context = false;
                anon.children.push(child);
                items.push(anon);
            } else {
                child.is_independent_formatting_context = true;
                if matches!(child.box_type, BoxType::InlineNode | BoxType::InlineBlock) {
                    child.box_type = BoxType::BlockNode;
                }
                items.push(child);
            }
        }
        root_box.children = items;
    } else if is_table_context {
        // Filter out whitespace-only text nodes between table rows, cells, and colgroups
        let filtered: Vec<LayoutBox> = raw_children
            .into_iter()
            .filter(|c| {
                if let Some(t) = c.text() {
                    !t.chars().all(|ch| ch.is_ascii_whitespace())
                } else {
                    true
                }
            })
            .collect();

        let is_table_root = styled_node.style.display == Display::Table
            || styled_node.tag_name.as_deref() == Some("table");

        if is_table_root {
            // Apply CSS 2.1 §17.2.1 Anonymous table objects
            let is_table_part = |c: &LayoutBox| -> bool {
                let disp = c.style.as_ref().map(|s| s.display);
                matches!(
                    disp,
                    Some(Display::TableRow)
                        | Some(Display::TableRowGroup)
                        | Some(Display::TableHeaderGroup)
                        | Some(Display::TableFooterGroup)
                        | Some(Display::TableCaption)
                        | Some(Display::TableColumn)
                        | Some(Display::TableColumnGroup)
                ) || matches!(
                    c.tag_name.as_deref(),
                    Some("tr")
                        | Some("tbody")
                        | Some("thead")
                        | Some("tfoot")
                        | Some("caption")
                        | Some("col")
                        | Some("colgroup")
                )
            };

            let has_non_table_children = filtered.iter().any(|c| !is_table_part(c));
            if has_non_table_children {
                let mut fixed_children = Vec::new();
                let mut non_table_seq = Vec::new();

                let flush_non_table = |seq: &mut Vec<LayoutBox>, out: &mut Vec<LayoutBox>| {
                    if seq.is_empty() {
                        return;
                    }
                    // If items are table cells without a row, wrap in anonymous row
                    let all_cells = seq.iter().all(|c| {
                        c.style.as_ref().map(|s| s.display) == Some(Display::TableCell)
                            || c.tag_name.as_deref() == Some("td")
                            || c.tag_name.as_deref() == Some("th")
                    });
                    if all_cells {
                        let anon_row_style =
                            create_anonymous_box_style(&styled_node.style, Display::TableRow);
                        let mut anon_row =
                            LayoutBox::new_anonymous_block(Some(anon_row_style.into()));
                        anon_row.children = std::mem::take(seq);
                        out.push(anon_row);
                    } else {
                        // Non-table content: wrap in anonymous table cell, then anonymous table row
                        let anon_cell_style =
                            create_anonymous_box_style(&styled_node.style, Display::TableCell);
                        let mut anon_cell =
                            LayoutBox::new_anonymous_block(Some(anon_cell_style.into()));
                        anon_cell.children = std::mem::take(seq);

                        let anon_row_style =
                            create_anonymous_box_style(&styled_node.style, Display::TableRow);
                        let mut anon_row =
                            LayoutBox::new_anonymous_block(Some(anon_row_style.into()));
                        anon_row.children.push(anon_cell);
                        out.push(anon_row);
                    }
                };

                for child in filtered {
                    if is_table_part(&child) {
                        flush_non_table(&mut non_table_seq, &mut fixed_children);
                        fixed_children.push(child);
                    } else {
                        non_table_seq.push(child);
                    }
                }
                flush_non_table(&mut non_table_seq, &mut fixed_children);
                root_box.children = fixed_children;
            } else {
                root_box.children = filtered;
            }
        } else {
            root_box.children = filtered;
        }
    } else if root_box.is_block() || root_box.box_type == BoxType::InlineBlock {
        // Apply CSS 2.1 §9.2.1.1: If block container has mixed block and inline children,
        // wrap contiguous sequences of inline children into anonymous block boxes.
        let has_blocks = raw_children.iter().any(|c| c.is_in_flow_block());
        let has_inlines = raw_children.iter().any(|c| c.is_inline());

        if has_blocks && has_inlines {
            let mut final_children = Vec::new();
            let mut current_anonymous: Option<LayoutBox> = None;

            let is_ignorable_whitespace_box = |b: &LayoutBox| -> bool {
                match &b.box_type {
                    BoxType::TextNode(t) => t.chars().all(|ch| ch.is_ascii_whitespace()),
                    _ => false,
                }
            };

            for child in raw_children {
                if child.is_out_of_flow() {
                    if let Some(anon) = current_anonymous.take() {
                        let is_all_whitespace = anon.children.is_empty()
                            || anon.children.iter().all(is_ignorable_whitespace_box);
                        if !is_all_whitespace {
                            final_children.push(anon);
                        }
                    }
                    final_children.push(child);
                } else if child.is_inline() {
                    let anon = current_anonymous.get_or_insert_with(|| {
                        let anon_style =
                            create_anonymous_box_style(&styled_node.style, Display::Block);
                        LayoutBox::new_anonymous_block(Some(anon_style.into()))
                    });
                    anon.children.push(child);
                } else {
                    if let Some(anon) = current_anonymous.take() {
                        let is_all_whitespace = anon.children.is_empty()
                            || anon.children.iter().all(is_ignorable_whitespace_box);
                        if !is_all_whitespace {
                            final_children.push(anon);
                        }
                    }
                    final_children.push(child);
                }
            }

            if let Some(anon) = current_anonymous.take() {
                let is_all_whitespace = anon.children.is_empty()
                    || anon.children.iter().all(is_ignorable_whitespace_box);
                if !is_all_whitespace {
                    final_children.push(anon);
                }
            }

            root_box.children = final_children;
        } else {
            root_box.children = raw_children;
        }
    } else {
        root_box.children = raw_children;
    }

    if styled_node.tag_name.as_deref() == Some("a")
        && let Some((_, href)) = styled_node
            .attributes
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("href"))
    {
        let href_str = href.clone();
        let frame_str = styled_node
            .attributes
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("target"))
            .map(|(_, target)| target.clone());

        root_box.link_target = Some(href_str.clone());
        root_box.link_frame = frame_str.clone();
        for child in &mut root_box.children {
            propagate_link_target(child, &href_str, frame_str.as_deref());
        }
    }

    for (k, v) in &styled_node.attributes {
        if k == "_mango_scroll_top" {
            if let Ok(st) = v.parse::<f32>() {
                root_box.scroll_offset_y = st;
            }
        } else if k == "_mango_scroll_left" {
            if let Ok(sl) = v.parse::<f32>() {
                root_box.scroll_offset_x = sl;
            }
        }
    }

    super::margin_trim::apply_margin_trim(&mut root_box);

    root_box
}

fn propagate_link_target(box_node: &mut LayoutBox, href: &str, frame: Option<&str>) {
    if box_node.link_target.is_none() {
        box_node.link_target = Some(href.to_string());
        box_node.link_frame = frame.map(|s| s.to_string());
    }
    for child in &mut box_node.children {
        propagate_link_target(child, href, frame);
    }
}

fn create_anonymous_box_style(
    parent_style: &mango_css::ComputedStyle,
    display: Display,
) -> mango_css::ComputedStyle {
    let mut style = parent_style.clone();
    style.display = display;
    // Per CSS 2.1 § 17.2.1, anonymous table boxes inherit only inheritable properties.
    // Non-inheritable properties take their initial values:
    style.background_color = mango_core::Color::TRANSPARENT;
    style.background_image = None;
    style.background_layers = Vec::new();
    style.border_top_width = 0.0;
    style.border_right_width = 0.0;
    style.border_bottom_width = 0.0;
    style.border_left_width = 0.0;
    style.padding_top = mango_css::values::Length::Px(0.0);
    style.padding_right = mango_css::values::Length::Px(0.0);
    style.padding_bottom = mango_css::values::Length::Px(0.0);
    style.padding_left = mango_css::values::Length::Px(0.0);
    style.margin_top = mango_css::values::Length::Px(0.0);
    style.margin_right = mango_css::values::Length::Px(0.0);
    style.margin_bottom = mango_css::values::Length::Px(0.0);
    style.margin_left = mango_css::values::Length::Px(0.0);
    style.width = mango_css::values::Length::Auto;
    style.height = mango_css::values::Length::Auto;
    style.min_width = mango_css::values::Length::Auto;
    style.max_width = mango_css::values::Length::Auto;
    style.min_height = mango_css::values::Length::Auto;
    style.max_height = mango_css::values::Length::Auto;
    style.box_shadow = None;
    style.box_shadows = Vec::new();
    style
}
