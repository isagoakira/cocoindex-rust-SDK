//! Test: #[cocoindex::cached] with proper Ctx parameter

use cocoindex::Ctx;

#[cocoindex::cached]
async fn cached_function(ctx: &Ctx, input: &str) -> String {
    input.to_uppercase()
}

fn main() {}
