use proc_macro::TokenStream;
use quote::quote;

const FORM_OR_MP: &str = "form_or_mp";

fn get_form_or_mp_generic(inp: &syn::FnArg) -> Option<syn::GenericArgument> {
    if let syn::FnArg::Typed(syn::PatType { ty, .. }) = inp {
        if let syn::Type::Path(syn::TypePath {
            path: syn::Path { segments, .. },
            ..
        }) = &**ty
        {
            let syn::PathSegment { ident, arguments } = segments.last().unwrap();
            let args = if let syn::PathArguments::AngleBracketed(args) = arguments {
                &args.args
            } else {
                unimplemented!()
            };
            if ident.to_string() == "FormOrMultipart" && args.len() == 1 {
                return Some(args[0].clone());
            }
        } else {
            unimplemented!()
        }
    } else {
        unimplemented!()
    }

    None
}

fn map_form_to_mp_to_future(inp: &syn::FnArg) -> syn::FnArg {
    match get_form_or_mp_generic(inp) {
        Some(tp) => {
            if let syn::GenericArgument::Type(tp) = tp {
                syn::parse_str(&format!(
                    "{}: awmpde::FormOrMultipartFuture<{}>",
                    FORM_OR_MP,
                    quote!(#tp).to_string()
                ))
            } else {
                unimplemented!()
            }
        }
        .unwrap(),
        None => inp.clone(),
    }
}

fn get_ident(inp: syn::FnArg) -> syn::Ident {
    if let syn::FnArg::Typed(syn::PatType { pat, .. }) = inp {
        if let syn::Pat::Ident(pat) = *pat {
            return pat.ident;
        }
    }
    unimplemented!()
}

fn assert_one_form_or_mp(inputs: &syn::punctuated::Punctuated<syn::FnArg, syn::token::Comma>) {
    let multiparts: Vec<_> = inputs
        .iter()
        .filter(|inp| get_form_or_mp_generic(inp).is_some())
        .collect();
    assert_eq!(multiparts.len(), 1);
}

pub fn form_or_multipart_unwrap(_: TokenStream, input: TokenStream) -> TokenStream {
    let syn::ItemFn {
        attrs,
        vis,
        block,
        sig:
            syn::Signature {
                ident,
                inputs,
                output,
                ..
            },
    } = syn::parse_macro_input!(input as syn::ItemFn);
    let output_type = match output {
        syn::ReturnType::Type(_, tp) => *tp,
        syn::ReturnType::Default => syn::Type::Tuple(syn::TypeTuple {
            paren_token: Default::default(),
            elems: Default::default(),
        }),
    };

    assert_one_form_or_mp(&inputs);

    let int_ident = syn::Ident::new(&format!("{}_internal", ident.to_string()), ident.span());
    let (int_inputs, attrs) = (inputs.iter(), attrs.iter());

    let int_cal_args = inputs.iter().map(map_form_to_mp_to_future).map(get_ident);
    let inputs = inputs.iter().map(map_form_to_mp_to_future);

    let out = quote! {
        #vis async fn #ident( #(#inputs,)* ) -> std::result::Result<#output_type, awmpde::Error> {
            #(#attrs)*
            async fn #int_ident( #(#int_inputs,)* ) -> #output_type {
                #block
            }

            let form_or_mp = awmpde::FormOrMultipart(form_or_mp.into_inner().await?);
            Ok(#int_ident( #(#int_cal_args,)* ).await)
        }
    };
    out.into()
}
