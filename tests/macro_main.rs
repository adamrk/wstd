// Verifies that the `main` macro is compiling.

#[wstd::main]
async fn main() {
    assert_eq!(1 + 1, 2);
}
