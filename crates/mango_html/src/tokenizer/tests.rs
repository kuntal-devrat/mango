use super::*;

#[test]
fn test_tokenize_basic_tags() {
    let html = "<div class=\"container\" id='main'>Hello</div>";
    let mut tokenizer = Tokenizer::new(html);

    match tokenizer.next_token() {
        Token::StartTag {
            name,
            attributes,
            self_closing,
        } => {
            assert_eq!(name, "div");
            assert_eq!(
                attributes,
                vec![
                    ("class".to_string(), "container".to_string()),
                    ("id".to_string(), "main".to_string())
                ]
            );
            assert!(!self_closing);
        }
        other => panic!("expected StartTag, got {:?}", other),
    }

    let mut text = String::new();
    loop {
        match tokenizer.next_token() {
            Token::Character(c) => text.push(c),
            Token::EndTag { name } => {
                assert_eq!(name, "div");
                break;
            }
            other => panic!("unexpected token {:?}", other),
        }
    }
    assert_eq!(text, "Hello");
}

#[test]
fn test_tokenize_doctype() {
    let html = "<!DOCTYPE html><html>";
    let mut tokenizer = Tokenizer::new(html);

    match tokenizer.next_token() {
        Token::Doctype { name, .. } => {
            assert_eq!(name, Some("html".to_string()));
        }
        other => panic!("expected Doctype, got {:?}", other),
    }

    match tokenizer.next_token() {
        Token::StartTag { name, .. } => {
            assert_eq!(name, "html");
        }
        other => panic!("expected StartTag, got {:?}", other),
    }
}

#[test]
fn test_tokenize_comment() {
    let html = "<!-- this is a comment -->";
    let mut tokenizer = Tokenizer::new(html);

    match tokenizer.next_token() {
        Token::Comment(text) => {
            assert_eq!(text, " this is a comment ");
        }
        other => panic!("expected Comment, got {:?}", other),
    }
}

#[test]
fn test_self_closing_tag() {
    let html = "<img src=\"mango.png\" alt=\"Mango\" />";
    let mut tokenizer = Tokenizer::new(html);

    match tokenizer.next_token() {
        Token::StartTag {
            name,
            attributes,
            self_closing,
        } => {
            assert_eq!(name, "img");
            assert!(self_closing);
            assert_eq!(attributes.len(), 2);
        }
        other => panic!("expected StartTag, got {:?}", other),
    }
}

#[test]
fn test_multi_char_and_unrecognized_entity_decoding() {
    // Unknown entity should not drop characters
    let html = "a &unknown; b";
    let mut tokenizer = Tokenizer::new(html);
    let mut text = String::new();
    loop {
        match tokenizer.next_token() {
            Token::Character(c) => text.push(c),
            Token::EndOfFile => break,
            _ => {}
        }
    }
    assert_eq!(text, "a &unknown; b");

    // Decoded entity
    let html2 = "one &amp; two";
    let mut tokenizer2 = Tokenizer::new(html2);
    let mut text2 = String::new();
    loop {
        match tokenizer2.next_token() {
            Token::Character(c) => text2.push(c),
            Token::EndOfFile => break,
            _ => {}
        }
    }
    assert_eq!(text2, "one & two");
}

#[test]
fn test_style_cdata_parsing() {
    let html = r#"<style type="text/css"><![CDATA[ div { color: green; } ]]></style>"#;
    let doc = crate::parse_html(html);
    let text = doc.text_content(doc.root());
    assert!(text.contains("div { color: green; }"));
}

#[test]
fn test_rcdata_title_and_textarea_entity_decoding() {
    let html = "<!doctype html><html><head><title>&quot;Mango &amp; Dillo &#9733;&quot; &copy; 2026</title></head><body><textarea>\nLine 1 &amp; &lt;b&gt;tag&lt;/b&gt; &#65;</textarea></body></html>";
    let doc = crate::parse_html(html);

    let title_node = doc
        .find_element_by_tag(doc.root(), "title")
        .expect("title element");
    let title_text = doc.text_content(title_node);
    assert_eq!(title_text, "\"Mango & Dillo ★\" © 2026");

    let textarea_node = doc
        .find_element_by_tag(doc.root(), "textarea")
        .expect("textarea element");
    let textarea_text = doc.text_content(textarea_node);
    assert_eq!(textarea_text, "Line 1 & <b>tag</b> A");
}

#[test]
fn test_rcdata_unclosed_legacy_entities() {
    let html = "<title>Tom &amp Jerry &copy 2026</title>";
    let doc = crate::parse_html(html);
    let title_node = doc
        .find_element_by_tag(doc.root(), "title")
        .expect("title element");
    let title_text = doc.text_content(title_node);
    assert_eq!(title_text, "Tom & Jerry © 2026");
}
