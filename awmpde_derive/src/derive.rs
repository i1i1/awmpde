use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input, Attribute, Data, DataStruct, DeriveInput, Field, Fields, FieldsNamed,
    GenericArgument, PathArguments, Type, TypePath,
};

fn from_json_attr(f: &Field) -> Option<&Attribute> {
    f.attrs
        .iter()
        .find(|attr| attr.path.segments.len() == 1 && attr.path.segments[0].ident == "serde_json")
}

fn get_struct_arg<'a, 'b>(s: &'a str, ty: &'b TypePath) -> Option<&'b GenericArgument> {
    let segs = &ty.path.segments;
    if segs.len() != 1 || segs[0].ident != s {
        return None;
    }

    if let PathArguments::AngleBracketed(ref bracargs) = segs[0].arguments {
        assert_eq!(bracargs.args.len(), 1);
        Some(&bracargs.args[0])
    } else {
        unimplemented!()
    }
}

pub fn derive_actix_multipart(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let ident = &ast.ident;

    let fields = if let Data::Struct(DataStruct {
        fields: Fields::Named(FieldsNamed { ref named, .. }),
        ..
    }) = ast.data
    {
        named
    } else {
        unimplemented!();
    };

    let struct_fields = fields.iter().map(|f| {
        let name = f.ident.as_ref().unwrap();
        let ty = &f.ty;

        if let Type::Path(ref typ) = ty {
            if let Some(vty) = get_struct_arg("Option", typ) {
                return quote! { #name: std::option::Option<#vty> };
            }
            if let Some(vty) = get_struct_arg("Vec", typ) {
                return quote! { #name: std::vec::Vec<#vty> };
            }
        }
        quote! { #name: std::result::Result<#ty, awmpde::Error> }
    });

    let struct_field_values = fields.iter().map(|f| {
        let name = f.ident.as_ref().unwrap();
        let ty = &f.ty;

        if let Type::Path(ref typ) = ty {
            if get_struct_arg("Option", typ).is_some() {
                return quote! { #name: std::option::Option::None };
            }
            if get_struct_arg("Vec", typ).is_some() {
                return quote! { #name: std::vec::Vec::new() };
            }
        }
        quote! {
            #name: std::result::Result::Err(awmpde::Error::FieldError(stringify!(#name)))
        }
    });

    let matched = fields
        .iter()
        .map(|f| {
            let name = f.ident.as_ref().unwrap();
            let ty = &f.ty;

            let code = if from_json_attr(f).is_some() {
                quote! {
                    <awmpde::Json<#ty> as awmpde::FromField>::from_field(field)
                        .await?.0
                }
            } else if let Type::Path(ref typ) = ty {
                if let Some(vty) = get_struct_arg("Vec", typ) {
                    let code = quote! {{
                        let f =
                            <#vty as awmpde::FromField>::from_field(field).await?;
                        mpstruct.#name.push(f);
                    }};
                    return (name, code);
                } else if let Some(vty) = get_struct_arg("Option", typ) {
                    let code = quote! {{
                        let f =
                            <#vty as awmpde::FromField>::from_field(field).await?;
                        mpstruct.#name = Some(f);
                    }};
                    return (name, code);
                } else {
                    quote! {
                        <#ty as awmpde::FromField>::from_field(field).await?
                    }
                }
            } else {
                unimplemented!()
            };

            let code = quote! {{ mpstruct.#name = Ok(#code); }};

            (name, code)
        })
        .map(|(name, code)| quote! { stringify!(#name) => #code });
    let fields = fields.iter().map(|f| {
        let name = &f.ident;
        let ty = &f.ty;
        if let Type::Path(ref typ) = ty {
            if get_struct_arg("Option", typ).is_some() || get_struct_arg("Vec", typ).is_some() {
                return quote! { #name: mpstruct.#name };
            }
        }
        quote! { #name: mpstruct.#name? }
    });

    let expanded = quote! {
        impl<'a> awmpde::FromMultipart<'a> for #ident {
            type Error = awmpde::Error;
            type Future = awmpde::futures::future::LocalBoxFuture<
               'a, std::result::Result<Self, awmpde::Error>
            >;

            #[inline]
            fn from_multipart(
                mut mp: awmpde::actix_multipart::Multipart,
            ) -> Self::Future {
                use awmpde::futures::{TryStreamExt, future::FutureExt};

                async move {
                    struct MPStructure {
                        #(#struct_fields,)*
                    };

                    let mut e: std::option::Option<awmpde::Error> = None;

                    let mut mpstruct = MPStructure {
                        #(#struct_field_values,)*
                    };

                    while let Ok(Some(field)) = mp.try_next().await {
                        let mut disp = awmpde::get_content_disposition(&field);
                        let name = disp.remove("name").unwrap();

                        if e.is_some() {
                            drop(<std::vec::Vec<u8> as awmpde::FromField>::from_field(field).await?);
                            continue;
                        }

                        match &name[..] {
                            #(#matched,)*
                            _ => {
                                e = Some(awmpde::Error::NoFieldError(name.to_string()));
                                drop(<std::vec::Vec<u8> as awmpde::FromField>::from_field(field).await?);
                            },
                        }
                    }

                    if let Some(e) = e {
                        Err(e)
                    } else {
                        Ok(Self{ #(#fields,)* })
                    }
                }
                .boxed_local()
            }
        }
    };

    expanded.into()
}
