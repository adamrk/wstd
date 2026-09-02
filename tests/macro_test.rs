// Verify that the test macro is acutally running.

#[wstd::test]
async fn pure_computation() -> Result<(), String> {
    assert_eq!(1 + 1, 2);
    Ok(())
}
