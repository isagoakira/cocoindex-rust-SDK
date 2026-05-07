//! Procedural macros for CocoIndex
//!
//! - `#[cocoindex::cached]` - Automatic cache key generation and LMDB lookup
//! - `#[cocoindex::component]` - Marks pipeline components for stats tracking

use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{Expr, FnArg, ItemFn, Pat};

mod key_gen;
use key_gen::generate_cache_key;

/// Parse options for the cached macro
struct CachedOptions {
    key_expr: Option<Expr>,
}

impl Parse for CachedOptions {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.is_empty() {
            return Ok(CachedOptions { key_expr: None });
        }
        let key_expr = input.parse()?;
        Ok(CachedOptions { key_expr: Some(key_expr) })
    }
}

/// Extract the Ok type from a Result type
fn extract_ok_type(ty: &syn::Type) -> Option<syn::Type> {
    // Handle Result<T, E>
    if let syn::Type::Path(type_path) = ty {
        if type_path.qself.is_some() {
            return None;
        }
        if type_path.path.segments.len() != 1 {
            return None;
        }
        let segment = type_path.path.segments.first()?;
        if segment.ident != "Result" {
            return None;
        }
        if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
            if args.args.len() != 2 {
                return None;
            }
            // First type argument is Ok type
            if let syn::GenericArgument::Type(ok_type) = args.args.first()? {
                return Some(ok_type.clone());
            }
        }
    }
    None
}

/// Check if a type is Ctx (handles &Ctx, &mut Ctx, Ctx)
fn is_ctx_type(ty: &syn::Type) -> bool {
    match ty {
        syn::Type::Reference(type_ref) => {
            // &Ctx or &mut Ctx — unwrap reference and recurse
            is_ctx_type(&type_ref.elem)
        }
        syn::Type::Path(type_path) if type_path.qself.is_none() => {
            type_path.path.segments.last()
                .map(|seg| seg.ident == "Ctx")
                .unwrap_or(false)
        }
        _ => false,
    }
}

/// Find the name of the Ctx parameter in a function's argument list
fn find_ctx_param_name(fn_args: &syn::punctuated::Punctuated<FnArg, syn::Token![,]>) -> proc_macro2::Ident {
    for arg in fn_args.iter() {
        if let FnArg::Typed(pat_type) = arg {
            if is_ctx_type(&pat_type.ty) {
                if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                    return pat_ident.ident.clone();
                }
            }
        }
    }
    // Fallback: use "ctx" as the assumed name
    proc_macro2::Ident::new("ctx", proc_macro2::Span::call_site())
}

/// # cached macro
///
/// Marks a function as cached with automatic LMDB memoization.
///
/// The cache key is generated from function arguments (excluding `Ctx`).
/// Optionally, a custom key expression can be provided via `key_expr = "..."`.
///
/// # Example
///
/// ```rust,ignore
/// #[cocoindex::cached]
/// async fn process_file(ctx: &Ctx, path: &str) -> Result<String> {
///     // ...
/// }
/// ```
///
/// With custom key:
///
/// ```rust,ignore
/// #[cocoindex::cached(key_expr = { format!("{}:{}", path, hash) })]
/// async fn process_file(ctx: &Ctx, path: &str, hash: u64) -> Result<String> {
///     // ...
/// }
/// ```
#[proc_macro_attribute]
pub fn cached(attr: TokenStream, item: TokenStream) -> TokenStream {
    let options = syn::parse_macro_input!(attr as CachedOptions);
    let input_fn = syn::parse_macro_input!(item as ItemFn);

    let fn_name = &input_fn.sig.ident;
    let fn_args = &input_fn.sig.inputs;
    let fn_output = &input_fn.sig.output;
    let fn_block = &input_fn.block;
    let fn_visibility = &input_fn.vis;
    let fn_async = input_fn.sig.asyncness;
    let fn_generics = &input_fn.sig.generics;
    let fn_where = &input_fn.sig.generics.where_clause.clone();

    // Find ctx parameter and non-ctx parameters
    let ctx_name = find_ctx_param_name(fn_args);
    let non_ctx_params: Vec<_> = fn_args
        .iter()
        .filter(|arg| match arg {
            FnArg::Typed(pat_type) => !is_ctx_type(&pat_type.ty),
            _ => true,
        })
        .collect();

    // Extract parameter names for non-ctx params
    let param_names: Vec<_> = non_ctx_params
        .iter()
        .filter_map(|arg| match arg {
            FnArg::Typed(pat_type) => Some(&pat_type.pat),
            _ => None,
        })
        .collect();

    let key_expr = if let Some(expr) = options.key_expr {
        quote! { #expr }
    } else {
        // Generate key from parameter values
        let names: Vec<&Pat> = param_names.iter().map(|p| p.as_ref()).collect();
        generate_cache_key(&names)
    };

    // Extract the value type and check if it's a Result
    let (is_result, value_type) = match &fn_output {
        syn::ReturnType::Type(_, ty) => {
            let ok_type = extract_ok_type(ty);
            (ok_type.is_some(), ok_type)
        }
        _ => (false, None),
    };

    let expanded = if fn_async.is_some() {
        if is_result {
            if let Some(ok_type) = value_type {
                let ctx_name = &ctx_name;
                quote! {
                    #fn_visibility #fn_async fn #fn_name #fn_generics (#fn_args) #fn_output #fn_where {
                        use xxhash_rust::xxh3::xxh3_64;
                        use serde_json;

                        let cache_key = #key_expr;

                        // Try to get from cache with stats counting
                        match #ctx_name.cache_get(&cache_key) {
                            Ok(Some(cached_value)) => {
                                if let Ok(result) = serde_json::from_slice::<#ok_type>(&cached_value) {
                                    return Ok(result);
                                }
                            }
                            Ok(None) => {} // miss already counted by cache_get
                            Err(_) => {}
                        }

                        // Call the original function
                        let result = (|| async { #fn_block })().await;

                        // Cache the result (only on success)
                        if let Ok(ref ok_result) = result {
                            if let Ok(encoded) = serde_json::to_vec(ok_result) {
                                let _ = #ctx_name.cache_set(&cache_key, &encoded);
                            }
                        }

                        result
                    }
                }
            } else {
                quote! {
                    #fn_visibility #fn_async fn #fn_name #fn_generics (#fn_args) #fn_output #fn_where {
                        #fn_block
                    }
                }
            }
        } else {
            // Direct return type, no Result wrapping
            let ctx_name = &ctx_name;
            quote! {
                #fn_visibility #fn_async fn #fn_name #fn_generics (#fn_args) #fn_output #fn_where {
                    use xxhash_rust::xxh3::xxh3_64;
                    use serde_json;

                    let cache_key = #key_expr;

                    // Try to get from cache with stats counting
                    match #ctx_name.cache_get(&cache_key) {
                        Ok(Some(cached_value)) => {
                            if let Ok(result) = serde_json::from_slice::<_>(&cached_value) {
                                return result;
                            }
                        }
                        Ok(None) => {}
                        Err(_) => {}
                    }

                    // Call the original function
                    let result = (|| async { #fn_block })().await;

                    // Cache the result
                    if let Ok(encoded) = serde_json::to_vec(&result) {
                        let _ = #ctx_name.cache_set(&cache_key, &encoded);
                    }

                    result
                }
            }
        }
    } else {
        if is_result {
            if let Some(ok_type) = value_type {
                let ctx_name = &ctx_name;
                quote! {
                    #fn_visibility fn #fn_name #fn_generics (#fn_args) #fn_output #fn_where {
                        use xxhash_rust::xxh3::xxh3_64;
                        use serde_json;

                        let cache_key = #key_expr;

                        // Try to get from cache with stats counting
                        match #ctx_name.cache_get(&cache_key) {
                            Ok(Some(cached_value)) => {
                                if let Ok(result) = serde_json::from_slice::<#ok_type>(&cached_value) {
                                    return Ok(result);
                                }
                            }
                            Ok(None) => {}
                            Err(_) => {}
                        }

                        // Call the original function
                        let result = (|| #fn_block)();

                        // Cache the result (only on success)
                        if let Ok(ref ok_result) = result {
                            if let Ok(encoded) = serde_json::to_vec(ok_result) {
                                let _ = #ctx_name.cache_set(&cache_key, &encoded);
                            }
                        }

                        result
                    }
                }
            } else {
                quote! {
                    #fn_visibility fn #fn_name #fn_generics (#fn_args) #fn_output #fn_where {
                        #fn_block
                    }
                }
            }
        } else {
            // Direct return type, no Result wrapping
            let ctx_name = &ctx_name;
            quote! {
                #fn_visibility fn #fn_name #fn_generics (#fn_args) #fn_output #fn_where {
                    use xxhash_rust::xxh3::xxh3_64;
                    use serde_json;

                    let cache_key = #key_expr;

                    // Try to get from cache with stats counting
                    match #ctx_name.cache_get(&cache_key) {
                        Ok(Some(cached_value)) => {
                            if let Ok(result) = serde_json::from_slice::<_>(&cached_value) {
                                return result;
                            }
                        }
                        Ok(None) => {}
                        Err(_) => {}
                    }

                    // Call the original function
                    let result = (|| #fn_block)();

                    // Cache the result
                    if let Ok(encoded) = serde_json::to_vec(&result) {
                        let _ = #ctx_name.cache_set(&cache_key, &encoded);
                    }

                    result
                }
            }
        }
    };

    expanded.into()
}

/// Parse options for the component macro
struct ComponentOptions {
    name: Option<String>,
}

impl Parse for ComponentOptions {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.is_empty() {
            return Ok(ComponentOptions { name: None });
        }
        let name_lit: syn::LitStr = input.parse()?;
        Ok(ComponentOptions { name: Some(name_lit.value()) })
    }
}

/// # component macro
///
/// Marks a function as a pipeline component with automatic stats collection.
///
/// # Example
///
/// ```rust,ignore
/// #[cocoindex::component]
/// async fn my_component(ctx: &Ctx, input: &str) -> Result<String> {
///     // ... component logic
///     Ok(result)
/// }
/// ```
#[proc_macro_attribute]
pub fn component(attr: TokenStream, item: TokenStream) -> TokenStream {
    let options = syn::parse_macro_input!(attr as ComponentOptions);
    let input_fn = syn::parse_macro_input!(item as ItemFn);

    let fn_name = &input_fn.sig.ident;
    let fn_args = &input_fn.sig.inputs;
    let fn_output = &input_fn.sig.output;
    let fn_block = &input_fn.block;
    let fn_visibility = &input_fn.vis;
    let fn_async = input_fn.sig.asyncness;
    let fn_generics = &input_fn.sig.generics;
    let fn_where = &input_fn.sig.generics.where_clause.clone();

    let component_name = options
        .name
        .unwrap_or_else(|| fn_name.to_string());

    // Find Ctx parameter name for stats access
    let ctx_name = find_ctx_param_name(fn_args);

    let expanded = quote! {
        #fn_visibility #fn_async fn #fn_name #fn_generics (#fn_args) #fn_output #fn_where {
            let start = std::time::Instant::now();
            let result = (|| #fn_block)();
            let elapsed = start.elapsed().as_millis() as u64;

            // Record component stats via Ctx
            {
                let mut stats = #ctx_name.stats_mut();
                stats.components_executed += 1;
            }

            // Track component name and timing (available for future detailed reporting)
            let _component_name = #component_name;
            let _elapsed = elapsed;
            let _ = (_component_name, _elapsed);

            result
        }
    };

    expanded.into()
}
