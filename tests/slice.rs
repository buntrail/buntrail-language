use buntrail_core::Document;
use buntrail_language::analyze;
#[test]
fn html_script_injection_has_document_coordinates_and_nested_folds() {
    let doc = Document::new("<html>\r\n<body>😀<script>\r\nfunction greet() {\r\n return 'hello';\r\n}\r\n</script></body></html>".into());
    let result = analyze(doc.snapshot()).unwrap();
    assert_eq!(result.revision, 0);
    assert!(result.spans.iter().any(|s| s.kind == 2 && s.length > 0));
    assert!(result.spans.iter().any(|s| s.kind == 1 && s.length > 20));
    assert!(result.spans.iter().any(|s| s.kind == 3 && s.start > 10));
}
