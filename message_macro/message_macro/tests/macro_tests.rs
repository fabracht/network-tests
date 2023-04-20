use trybuild::TestCases;

#[test]
fn ui_tests() {
    let t = TestCases::new();
    t.compile_fail("tests/ui/unsupported_structure.rs");
    t.pass("tests/ui/unnamed_fields.rs");
}
