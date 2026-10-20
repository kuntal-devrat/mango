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
