// Cache key generation for the #[cocoindex::cached] macro

use proc_macro2::TokenStream;
use quote::quote;
use syn::Pat;

/// Generate a cache key expression from function parameter names
pub fn generate_cache_key(param_names: &[&Pat]) -> TokenStream {
    if param_names.is_empty() {
        quote! { String::new() }
    } else {
        let key_parts: Vec<TokenStream> = param_names
            .iter()
            .map(|name| generate_key_part(name))
            .collect();

        quote! {
            {
                use xxhash_rust::xxh3::xxh3_64;
                let encoded = serde_json::to_vec(&(#(#key_parts,)*))
                    .unwrap_or_default();
                format!("{:016x}", xxh3_64(&encoded))
            }
        }
    }
}

/// Generate a key part from a parameter pattern
fn generate_key_part(pat: &Pat) -> TokenStream {
    match pat {
        Pat::Ident(ident) => {
            let name = &ident.ident;
            quote!(#name)
        }
        Pat::Tuple(tuple) => {
            let elems: Vec<TokenStream> = tuple
                .elems
                .iter()
                .map(|p| generate_key_part(p))
                .collect();
            quote!( (#(#elems),*) )
        }
        Pat::Struct(struct_pat) => {
            let fields: Vec<TokenStream> = struct_pat
                .fields
                .iter()
                .map(|f| generate_key_part(&f.pat))
                .collect();
            quote!( (#(#fields),*) )
        }
        Pat::Slice(slice) => {
            let elems: Vec<TokenStream> = slice
                .elems
                .iter()
                .map(|p| generate_key_part(p))
                .collect();
            quote!( [#(#elems),*] )
        }
        _ => quote!("unknown"),
    }
}
