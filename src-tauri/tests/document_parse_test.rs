// SPDX-License-Identifier: MIT
//! Behavioral tests for document_parse Phase 1 type skeleton.

use impforge_app_lib::document_parse::{DocumentFormat, ParseResult};

#[test]
fn document_format_serializes_snake_case() {
    for (f, expected) in [
        (DocumentFormat::Pdf, "pdf"),
        (DocumentFormat::Docx, "docx"),
        (DocumentFormat::Xlsx, "xlsx"),
        (DocumentFormat::Pptx, "pptx"),
        (DocumentFormat::Odt, "odt"),
        (DocumentFormat::Ods, "ods"),
        (DocumentFormat::Odp, "odp"),
        (DocumentFormat::Markdown, "markdown"),
        (DocumentFormat::Html, "html"),
        (DocumentFormat::Plaintext, "plaintext"),
        (DocumentFormat::Unknown, "unknown"),
    ] {
        let j = serde_json::to_string(&f).expect("serialize DocumentFormat");
        assert_eq!(j, format!("\"{expected}\""));
    }
}

#[test]
fn parse_result_roundtrips_through_json() {
    let original = ParseResult {
        format: DocumentFormat::Pdf,
        text: "Hello PDF".into(),
        metadata: serde_json::json!({"pages": 3}),
    };
    let j = serde_json::to_string(&original).expect("serialize ParseResult");
    let back: ParseResult = serde_json::from_str(&j).expect("deserialize ParseResult");
    assert_eq!(original.text, back.text);
    assert_eq!(original.format, back.format);
}

#[test]
fn types_implement_required_traits() {
    fn assert_traits<T: Clone + std::fmt::Debug + serde::Serialize + for<'de> serde::Deserialize<'de>>()
    {
    }
    assert_traits::<DocumentFormat>();
    assert_traits::<ParseResult>();
}
