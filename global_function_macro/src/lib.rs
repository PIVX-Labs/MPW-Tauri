/// Procedural macro to generate tauri functions based on a struct impl.
/// The methods name are `struct_name_as_snake`_case`_`method_name`.
/// A global ref to that struct must be provided. That can be done with a tokio OnceCell

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, GenericArgument, ItemImpl, PathArguments, ReturnType, Type};
use proc_macro2::{Ident, Span};

fn pascal_case_to_snake_case(name: &str) -> String {
    let mut result = String::new();
    
    for (i, c) in name.chars().enumerate() {
        if c.is_uppercase() {
            if i != 0 {
                result.push('_');
            }
            result.push(c.to_ascii_lowercase());
        } else {
            result.push(c);
        }
    }
    
    result
}

#[proc_macro_attribute]
pub fn generate_global_functions(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemImpl);
    // Get struct name
    let struct_name = match *input.self_ty {
        syn::Type::Path(ref type_path) => &type_path.path.segments[0].ident,
        _ => panic!("Expected a struct name"),
    };
    let struct_name_snake_case = pascal_case_to_snake_case(&struct_name.to_string());

    let mut generated_functions = vec![];

    for method in input.items.iter() {
        if let syn::ImplItem::Fn(method) = method {
	    let method_name = &method.sig.ident;
            let function_name = &Ident::new(&format!("{}_{}", struct_name_snake_case, method_name.to_string()), method_name.span());
	    let return_type = match &method.sig.output {
		ReturnType::Default => panic!("Return a type you lazy ass"),
		ReturnType::Type(arrow, return_type) => {
		    &match &**return_type {
			Type::Path(return_type) => {
			    match return_type.path.segments.last() {
				Some(segment) => {
				    if let PathArguments::AngleBracketed(ref generics) = segment.arguments {
					if let Some(GenericArgument::Type(inner_type)) = generics.args.first() {
					    Box::new(inner_type.clone())
					} else {
					    panic!("Unsupported type")
					}
				    } else {
					panic!("Unsupported type.")
				    }
				}
				None => panic!("Unsupported type.")
			    }

			}
			_ => panic!("Unsupported type. Change to result manually.")
		    }
		}
	    };
            let inputs = &method.sig.inputs;

            // Collect input argument names and types
            let mut arg_names = vec![];
            let mut arg_types = vec![];

            for input in inputs.iter() {
                if let syn::FnArg::Typed(pat_type) = input {
                    if let syn::Pat::Ident(pat_ident) = *pat_type.pat.clone() {
                        arg_names.push(pat_ident.ident.clone());
                        arg_types.push(*pat_type.ty.clone());
                    }
                }
            }

	    let instance_name = Ident::new(&format!("get_{}", struct_name_snake_case), Span::call_site());

            generated_functions.push(quote! {
		#[tauri::command]
                pub async fn #function_name( #( #arg_names: #arg_types ),* ) -> Result<#return_type, ()> {
                    Ok(#instance_name().await.#method_name( #( #arg_names ),* ).await.unwrap())
                }
            });
        }
    }
    (quote! {
        #input
	pub(crate) mod auto_generated {
	    use super::*;
            #(#generated_functions)*
	}
    }).into()

}
