#[test]
fn ui_tests() {
    let t = trybuild::TestCases::new();

    // Tests that should compile successfully
    t.pass("tests/ui/cached_pass.rs");

    // Tests that should fail to compile
    t.compile_fail("tests/ui/cached_missing_ctx.rs");
}
