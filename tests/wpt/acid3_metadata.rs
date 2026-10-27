//! Acid3 Subtest Conformance Metadata and Bucket Taxonomy.
//!
//! Provides static definitions of all 100 Acid3 subtests (0 to 99),
//! their target buckets, descriptive titles, and bucket mapping functions.

#![allow(dead_code)]

pub struct TestMeta {
    pub number: u32,
    pub bucket: u8,
    pub title: &'static str,
}

pub const ACID3_METADATA: &[TestMeta] = &[
    TestMeta {
        number: 0,
        bucket: 0,
        title: "Last-child style recomputation",
    },
    TestMeta {
        number: 1,
        bucket: 1,
        title: "NodeFilters and Exceptions",
    },
    TestMeta {
        number: 2,
        bucket: 1,
        title: "Removing nodes during iteration",
    },
    TestMeta {
        number: 3,
        bucket: 1,
        title: "The infinite iterator",
    },
    TestMeta {
        number: 4,
        bucket: 1,
        title: "Ignoring whitespace text nodes with NodeIterator",
    },
    TestMeta {
        number: 5,
        bucket: 1,
        title: "Ignoring whitespace text nodes with TreeWalker",
    },
    TestMeta {
        number: 6,
        bucket: 1,
        title: "Walking outside a tree",
    },
    TestMeta {
        number: 7,
        bucket: 1,
        title: "Basic DOM Range tests",
    },
    TestMeta {
        number: 8,
        bucket: 1,
        title: "Moving Range boundary points",
    },
    TestMeta {
        number: 9,
        bucket: 1,
        title: "Range.extractContents() in a Document",
    },
    TestMeta {
        number: 10,
        bucket: 1,
        title: "Ranges and Attribute Nodes",
    },
    TestMeta {
        number: 11,
        bucket: 1,
        title: "Ranges and Comments",
    },
    TestMeta {
        number: 12,
        bucket: 1,
        title: "Ranges under text node mutations",
    },
    TestMeta {
        number: 13,
        bucket: 1,
        title: "Ranges under node deletions",
    },
    TestMeta {
        number: 14,
        bucket: 1,
        title: "HTTP Content-Type: image/png",
    },
    TestMeta {
        number: 15,
        bucket: 1,
        title: "HTTP Content-Type: text/plain",
    },
    TestMeta {
        number: 16,
        bucket: 1,
        title: "<object> fallback and HTTP status codes",
    },
    TestMeta {
        number: 17,
        bucket: 2,
        title: "hasAttribute()",
    },
    TestMeta {
        number: 18,
        bucket: 2,
        title: "nodeType verification",
    },
    TestMeta {
        number: 19,
        bucket: 2,
        title: "DOM Constants",
    },
    TestMeta {
        number: 20,
        bucket: 2,
        title: "Null bytes in DOM strings",
    },
    TestMeta {
        number: 21,
        bucket: 2,
        title: "Basic XML Namespace handling",
    },
    TestMeta {
        number: 22,
        bucket: 2,
        title: "createElement() tag name validation",
    },
    TestMeta {
        number: 23,
        bucket: 2,
        title: "createElementNS() namespace validation",
    },
    TestMeta {
        number: 24,
        bucket: 2,
        title: "Inline event handler attributes",
    },
    TestMeta {
        number: 25,
        bucket: 2,
        title: "Namespace checking in createDocumentType()",
    },
    TestMeta {
        number: 26,
        bucket: 2,
        title: "DOM tree survives detachment",
    },
    TestMeta {
        number: 27,
        bucket: 2,
        title: "Tree manipulation continuation",
    },
    TestMeta {
        number: 28,
        bucket: 2,
        title: "getElementById() & name attribute",
    },
    TestMeta {
        number: 29,
        bucket: 2,
        title: "Whitespace survival during cloneNode()",
    },
    TestMeta {
        number: 30,
        bucket: 2,
        title: "EventTarget.dispatchEvent()",
    },
    TestMeta {
        number: 31,
        bucket: 2,
        title: "stopPropagation() and Event Capture",
    },
    TestMeta {
        number: 32,
        bucket: 2,
        title: "Event bubbling through Document",
    },
    TestMeta {
        number: 33,
        bucket: 3,
        title: "Basic CSS Selectors: classes & attributes",
    },
    TestMeta {
        number: 34,
        bucket: 3,
        title: ":lang() pseudo-class & [|=]",
    },
    TestMeta {
        number: 35,
        bucket: 3,
        title: ":first-child pseudo-class",
    },
    TestMeta {
        number: 36,
        bucket: 3,
        title: ":last-child pseudo-class",
    },
    TestMeta {
        number: 37,
        bucket: 3,
        title: ":only-child pseudo-class",
    },
    TestMeta {
        number: 38,
        bucket: 3,
        title: ":empty pseudo-class",
    },
    TestMeta {
        number: 39,
        bucket: 3,
        title: ":nth-child() & :nth-last-child()",
    },
    TestMeta {
        number: 40,
        bucket: 3,
        title: "*-of-type pseudo-classes",
    },
    TestMeta {
        number: 41,
        bucket: 3,
        title: ":root & :not()",
    },
    TestMeta {
        number: 42,
        bucket: 3,
        title: "Combinators in dynamic situations",
    },
    TestMeta {
        number: 43,
        bucket: 3,
        title: "UI Pseudo-classes",
    },
    TestMeta {
        number: 44,
        bucket: 3,
        title: "Universal selector spacing",
    },
    TestMeta {
        number: 45,
        bucket: 3,
        title: "cssFloat and inline style attribute",
    },
    TestMeta {
        number: 46,
        bucket: 3,
        title: "CSS Media Queries",
    },
    TestMeta {
        number: 47,
        bucket: 3,
        title: "cursor property & CSS3 values",
    },
    TestMeta {
        number: 48,
        bucket: 3,
        title: ":link and :visited pseudo-classes",
    },
    TestMeta {
        number: 49,
        bucket: 4,
        title: "Table DOM accessors ping test",
    },
    TestMeta {
        number: 50,
        bucket: 4,
        title: "Table construction & hierarchy",
    },
    TestMeta {
        number: 51,
        bucket: 4,
        title: "Row creation and ordering",
    },
    TestMeta {
        number: 52,
        bucket: 4,
        title: "<form> and form.elements",
    },
    TestMeta {
        number: 53,
        bucket: 4,
        title: "Changing <input> dynamically",
    },
    TestMeta {
        number: 54,
        bucket: 4,
        title: "Changing parsed <input>",
    },
    TestMeta {
        number: 55,
        bucket: 4,
        title: "Moving checkboxes retains state",
    },
    TestMeta {
        number: 56,
        bucket: 4,
        title: "Cloned radio buttons retain state",
    },
    TestMeta {
        number: 57,
        bucket: 4,
        title: "HTMLSelectElement.add()",
    },
    TestMeta {
        number: 58,
        bucket: 4,
        title: "HTMLOptionElement.defaultSelected",
    },
    TestMeta {
        number: 59,
        bucket: 4,
        title: "<button> attributes",
    },
    TestMeta {
        number: 60,
        bucket: 4,
        title: "className vs class attribute",
    },
    TestMeta {
        number: 61,
        bucket: 4,
        title: "className whitespace preservation",
    },
    TestMeta {
        number: 62,
        bucket: 4,
        title: "DOM attributes vs Content attributes",
    },
    TestMeta {
        number: 63,
        bucket: 4,
        title: "<area> element attributes",
    },
    TestMeta {
        number: 64,
        bucket: 4,
        title: "<object> attributes & non-existent properties",
    },
    TestMeta {
        number: 65,
        bucket: 5,
        title: "Dynamic SVG & HTML subresource loading",
    },
    TestMeta {
        number: 66,
        bucket: 5,
        title: "localName on non-element nodes",
    },
    TestMeta {
        number: 67,
        bucket: 5,
        title: "removeNamedItemNS on missing attributes",
    },
    TestMeta {
        number: 68,
        bucket: 5,
        title: "UTF-16 surrogate pairs in DOM strings",
    },
    TestMeta {
        number: 69,
        bucket: 5,
        title: "Support files loaded check",
    },
    TestMeta {
        number: 70,
        bucket: 5,
        title: "XML encoding test",
    },
    TestMeta {
        number: 71,
        bucket: 5,
        title: "HTML parsing edge cases",
    },
    TestMeta {
        number: 72,
        bucket: 5,
        title: "Dynamic modification of <style> text nodes",
    },
    TestMeta {
        number: 73,
        bucket: 5,
        title: "Nested DOM events dispatch",
    },
    TestMeta {
        number: 74,
        bucket: 5,
        title: "getSVGDocument() on <iframe> / <object>",
    },
    TestMeta {
        number: 75,
        bucket: 5,
        title: "SVG Element creation & SVG DOM interface",
    },
    TestMeta {
        number: 76,
        bucket: 5,
        title: "SVG element lookup",
    },
    TestMeta {
        number: 77,
        bucket: 5,
        title: "SVG text elements & getNumberOfChars()",
    },
    TestMeta {
        number: 78,
        bucket: 5,
        title: "SVG document root validation",
    },
    TestMeta {
        number: 79,
        bucket: 5,
        title: "SVG namespace mutation",
    },
    TestMeta {
        number: 80,
        bucket: 5,
        title: "Frame cleanup & XHTML script execution check",
    },
    TestMeta {
        number: 81,
        bucket: 6,
        title: "Array elisions at end",
    },
    TestMeta {
        number: 82,
        bucket: 6,
        title: "Array elisions in middle",
    },
    TestMeta {
        number: 83,
        bucket: 6,
        title: "Array built-in methods",
    },
    TestMeta {
        number: 84,
        bucket: 6,
        title: "Number to string conversion",
    },
    TestMeta {
        number: 85,
        bucket: 6,
        title: "String operations",
    },
    TestMeta {
        number: 86,
        bucket: 6,
        title: "Date methods without arguments",
    },
    TestMeta {
        number: 87,
        bucket: 6,
        title: "Date year boundary handling",
    },
    TestMeta {
        number: 88,
        bucket: 6,
        title: "Unicode escapes in identifiers",
    },
    TestMeta {
        number: 89,
        bucket: 6,
        title: "Regular Expressions: empty character classes",
    },
    TestMeta {
        number: 90,
        bucket: 6,
        title: "Regular Expressions: backreferences",
    },
    TestMeta {
        number: 91,
        bucket: 6,
        title: "Property enumerability",
    },
    TestMeta {
        number: 92,
        bucket: 6,
        title: "Function object properties",
    },
    TestMeta {
        number: 93,
        bucket: 6,
        title: "FunctionExpression semantics",
    },
    TestMeta {
        number: 94,
        bucket: 6,
        title: "Catch block exception scope",
    },
    TestMeta {
        number: 95,
        bucket: 6,
        title: "Expression typing (typeof)",
    },
    TestMeta {
        number: 96,
        bucket: 6,
        title: "URI encoding & null bytes",
    },
    TestMeta {
        number: 97,
        bucket: 0,
        title: "Data URI script execution",
    },
    TestMeta {
        number: 98,
        bucket: 0,
        title: "XHTML and DOM",
    },
    TestMeta {
        number: 99,
        bucket: 0,
        title: "Sanity: a.href setter does not overwrite child text",
    },
];

pub fn bucket_for_test(num: u32) -> u8 {
    match num {
        0 | 97..=99 => 0,
        1..=16 => 1,
        17..=32 => 2,
        33..=48 => 3,
        49..=64 => 4,
        65..=80 => 5,
        81..=96 => 6,
        _ => 0,
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_acid3_metadata_integrity() {
        assert_eq!(ACID3_METADATA.len(), 100);
        for (i, meta) in ACID3_METADATA.iter().enumerate() {
            assert_eq!(meta.number, i as u32);
            assert_eq!(meta.bucket, bucket_for_test(meta.number));
            assert!(!meta.title.is_empty());
        }
    }
}
