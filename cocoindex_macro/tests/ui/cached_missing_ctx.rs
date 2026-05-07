use cocoindex_macro::cached;

// This function has no Ctx parameter, so #[cached] should fail
// The generated code references ctx.cache_get() which won't exist
#[cached]
async fn no_ctx_function(input: String) -> Result<String, ()> {
    Ok(input)
}

fn main() {}
