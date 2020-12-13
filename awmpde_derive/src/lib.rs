#![recursion_limit = "128"]

mod attrib;
mod derive;

use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn form_or_multipart_unwrap(
    args: TokenStream,
    input: TokenStream,
) -> TokenStream {
    attrib::form_or_multipart_unwrap(args, input)
}

#[proc_macro_derive(FromActixMultipart, attributes(serde_json))]
pub fn derive_actix_multipart(input: TokenStream) -> TokenStream {
    derive::derive_actix_multipart(input)
}
