use super::*;
use crate::values::Value;
use std::collections::HashMap;

#[test]
fn test_property_id_roundtrip() {
    for &id in CssPropertyId::all() {
        let name = id.name();
        let resolved = CssPropertyId::from_name(name);
        assert_eq!(resolved, Some(id), "roundtrip failed for {name}");
    }
}

#[test]
fn test_unknown_property_returns_none() {
    assert_eq!(CssPropertyId::from_name("made-up-property"), None);
    assert_eq!(CssPropertyId::from_name(""), None);
}

#[test]
fn test_inherited_properties() {
    assert!(CssPropertyId::Color.is_inherited());
    assert!(CssPropertyId::FontSize.is_inherited());
    assert!(CssPropertyId::Visibility.is_inherited());
    assert!(!CssPropertyId::Display.is_inherited());
    assert!(!CssPropertyId::Width.is_inherited());
    assert!(!CssPropertyId::Opacity.is_inherited());
}

#[test]
fn test_animatable_properties() {
    assert!(CssPropertyId::Opacity.is_animatable());
    assert!(CssPropertyId::Transform.is_animatable());
    assert!(CssPropertyId::Width.is_animatable());
    assert!(!CssPropertyId::Display.is_animatable());
    assert!(!CssPropertyId::Position.is_animatable());
}

#[test]
fn test_property_count() {
    assert_eq!(CssPropertyId::all().len(), CSS_PROPERTY_COUNT);
}

#[test]
fn test_property_map_basic_operations() {
    let mut map = PropertyMap::new();
    assert!(map.is_empty());
    assert_eq!(map.len(), 0);

    map.insert(CssPropertyId::Display, Value::Keyword("block".to_string()));
    map.insert(CssPropertyId::Color, Value::Keyword("red".to_string()));

    assert_eq!(map.len(), 2);
    assert!(map.contains(CssPropertyId::Display));
    assert!(map.contains(CssPropertyId::Color));
    assert!(!map.contains(CssPropertyId::Width));

    assert_eq!(
        map.get(CssPropertyId::Display),
        Some(&Value::Keyword("block".to_string()))
    );

    let removed = map.remove(CssPropertyId::Display);
    assert_eq!(removed, Some(Value::Keyword("block".to_string())));
    assert_eq!(map.len(), 1);
    assert!(!map.contains(CssPropertyId::Display));
}

#[test]
fn test_property_map_string_conversion() {
    let mut str_map = HashMap::new();
    str_map.insert("font-size".to_string(), Value::Keyword("16px".to_string()));
    str_map.insert("color".to_string(), Value::Keyword("blue".to_string()));
    str_map.insert(
        "unknown-custom-prop".to_string(),
        Value::Keyword("test".to_string()),
    );

    let prop_map = PropertyMap::from_string_map(&str_map);
    assert_eq!(prop_map.len(), 2);
    assert!(prop_map.contains(CssPropertyId::FontSize));
    assert!(prop_map.contains(CssPropertyId::Color));

    let roundtrip = prop_map.to_string_map();
    assert_eq!(roundtrip.len(), 2);
    assert_eq!(
        roundtrip.get("font-size"),
        Some(&Value::Keyword("16px".to_string()))
    );
    assert_eq!(
        roundtrip.get("color"),
        Some(&Value::Keyword("blue".to_string()))
    );
}
