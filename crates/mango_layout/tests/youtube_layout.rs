use mango_core::Size;
use mango_html::parse_html;
use mango_layout::layout_document;

#[test]
fn test_youtube_masthead_layout() {
    let html = r##"<!DOCTYPE html>
<html>
<head>
<style>
body { padding: 0; margin: 0; }
ytd-app { display: block; }
ytd-masthead.shell {
    background-color: #fff !important;
    position: fixed;
    top: 0;
    right: 0;
    left: 0;
    display: flex;
    height: 56px;
    align-items: center;
}
ytd-masthead.shell svg {
    width: 40px;
    height: 40px;
    padding: 8px;
    margin-right: 8px;
    box-sizing: border-box;
    color: #606060;
    fill: currentColor;
}
#masthead-logo {
    display: flex;
    width: 129px;
}
#masthead-logo a {
    display: flex !important;
    align-items: center;
}
#masthead-skeleton-icons {
    display: flex;
    flex: 1;
    flex-direction: row;
    justify-content: flex-end;
}
.masthead-skeleton-icon {
    border-radius: 50%;
    height: 32px;
    width: 32px;
    margin: 0 8px;
    background-color: #e3e3e3;
}
#home-page-skeleton {
    margin-top: 56px;
    height: 500px;
}
</style>
</head>
<body>
<ytd-app>
  <ytd-masthead id="masthead" logo-type="YOUTUBE_LOGO" slot="masthead" class="shell ">
    <div id="search-container" class="ytd-searchbox-spt" slot="search-container"></div>
    <div id="search-input" class="ytd-searchbox-spt" slot="search-input"><input id="search" autocapitalize="none" autocomplete="off" autocorrect="off" hidden name="search_query" tabindex="0" type="text" spellcheck="false"></div>
    <svg id="menu-icon" class="external-icon" preserveAspectRatio="xMidYMid meet"><g id="menu" class="yt-icons-ext" viewBox="0 0 24 24"><path d="M21,6H3V5h18V6z M21,11H3v1h18V11z M21,17H3v1h18V17z"/></g></svg>
    <div id="masthead-logo" slot="masthead-logo"><a style="display: none;" href="/" title="YouTube"><svg xmlns="http://www.w3.org/2000/svg" id="yt-ringo2-svg" width="93" height="20" viewBox="0 0 93 20"><g><path d="M14.4848 20C14.4848 20 23.5695 20 25.8229 19.4C27.0917 19.06 28.0459 18.08 28.3808 16.87C29 14.65 29 9.98 29 9.98C29 9.98 29 5.34 28.3808 3.14C28.0459 1.9 27.0917 0.94 25.8229 0.61C23.5695 0 14.4848 0 14.4848 0C14.4848 0 5.42037 0 3.17711 0.61C1.9286 0.94 0.954148 1.9 0.59888 3.14C0 5.34 0 9.98 0 9.98C0 9.98 0 14.65 0.59888 16.87C0.954148 18.08 1.9286 19.06 3.17711 19.4C5.42037 20 14.4848 20 14.4848 20Z" fill="#FF0033"/><path d="M19 10L11.5 5.75V14.25L19 10Z" fill="white"/></g></svg></a><span id="country-code"></span></div>
    <div id="masthead-skeleton-icons" slot="masthead-skeleton"><div class="masthead-skeleton-icon"></div><div class="masthead-skeleton-icon"></div><div class="masthead-skeleton-icon"></div></div>
  </ytd-masthead>
</ytd-app>
<div id="home-page-skeleton"></div>
</body>
</html>"##;

    let doc = parse_html(html);
    let author_styles = mango_layout::extract_style_elements(&doc);
    let author_style_refs: Vec<&_> = author_styles.iter().collect();
    let styled_root =
        mango_layout::build_style_tree_with_size(&doc, &author_style_refs, 1280.0, 800.0).unwrap();

    fn print_styled(node: &mango_layout::StyledNode, depth: usize) {
        let indent = "  ".repeat(depth);
        let tag = node
            .tag_name
            .as_deref()
            .unwrap_or(node.text.as_deref().unwrap_or("anon"));
        println!(
            "{indent}<{tag}> display={:?} pos={:?}",
            node.style.display, node.style.position
        );
        for child in &node.children {
            print_styled(child, depth + 1);
        }
    }
    println!("=== STYLED TREE ===");
    print_styled(&styled_root, 0);
    println!("===================");

    let raw_root_box = mango_layout::build_box_tree(&styled_root);
    println!("=== RAW BOX TREE ===");
    print_box(&raw_root_box, 0);
    println!("====================");

    let (root_box, dl) = layout_document(&doc, &author_style_refs, Size::new(1280.0, 800.0));

    fn print_box(b: &mango_layout::LayoutBox, depth: usize) {
        let indent = "  ".repeat(depth);
        let tag = b.tag_name.as_deref().unwrap_or("anon");
        let border = b.dimensions.border_box();
        let content = b.dimensions.content;
        println!(
            "{indent}<{tag}> border=({:.1},{:.1} {:.1}x{:.1}) content=({:.1},{:.1} {:.1}x{:.1})",
            border.x(),
            border.y(),
            border.width(),
            border.height(),
            content.x(),
            content.y(),
            content.width(),
            content.height()
        );
        for child in &b.children {
            print_box(child, depth + 1);
        }
    }

    print_box(&root_box, 0);
    println!("Display list commands count: {}", dl.len());
    for (i, item) in dl.as_slice().iter().enumerate() {
        println!("DL item #{}: {:?}", i, item);
    }
}
