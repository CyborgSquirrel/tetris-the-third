use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DataEnum, DeriveInput, parse_macro_input};

#[proc_macro_derive(EnumSelect)]
pub fn next_enum_variant(input: TokenStream) -> TokenStream {
	let DeriveInput {data,ident,..} = parse_macro_input!(input);
	match data {
		Data::Enum(DataEnum{variants,..}) => {
			let next_iter = variants.iter();
			let next_shifted_iter = variants.iter().skip(1).chain(variants.iter().take(1));
			
			let prev_iter = variants.iter();
			let prev_shifted_iter = variants.iter().skip(1).chain(variants.iter().take(1));
			
			quote!(
				impl EnumSelect for #ident {
					fn next_variant(self) -> Self {
						match self {
							#(#ident::#next_iter => #ident::#next_shifted_iter),*
						}
					}
					fn prev_variant(self) -> Self {
						match self {
							#(#ident::#prev_shifted_iter => #ident::#prev_iter),*
						}
					}
				}
			).into()
		}
		Data::Struct(_) => panic!("This macro doesn't work for structs"),
		Data::Union(_) => panic!("This macro doesn't work for unions"),
	}
}