extern crate proc_macro;
use proc_macro::TokenStream;
use quote::{__private::Span, quote, quote_spanned};
use syn::{
    parenthesized, parse_macro_input, spanned::Spanned, AngleBracketedGenericArguments, Data,
    DeriveInput, Fields, LitInt,
};

#[proc_macro_derive(BeBytes, attributes(U8))]
pub fn derive_be_bytes(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident.clone();
    let my_trait_path: syn::Path = syn::parse_str("BeBytes").unwrap();

    match input.data {
        Data::Struct(data) => match data.fields {
            Fields::Named(fields) => {
                let mut errors = Vec::new();
                let mut field_limit_check = Vec::new();
                let mut field_parsing = Vec::new();
                let mut field_writing = Vec::new();
                // initialize the bit sum to 0
                let mut u8_bit_sum = 0;
                let mut non_bit_fields = 0;
                let mut total_size = 0;
                // initialize the last position to None
                // let mut last_pos = None;
                // get the last field
                let last_field = fields.named.last();
                let mut is_last_field = false;

                for field in fields.named.clone().into_iter() {
                    if let Some(last_field) = last_field {
                        is_last_field = last_field.ident == field.ident;
                    }
                    // initialize u8 flag to false
                    let mut u8_attribute_present = false;

                    // get the attributes of the field
                    let attributes = field.attrs.clone();

                    // get the name of the field
                    let field_name = field.ident.clone().unwrap();
                    // get the type of the field
                    let field_type = &field.ty;

                    // retrieve position and size from attributes
                    let (pos, size) = parse_u8_attribute(
                        attributes,
                        &mut u8_attribute_present,
                        &mut errors,
                        &mut non_bit_fields,
                    );

                    // check if the field is of type u8
                    if u8_attribute_present {
                        if let syn::Type::Path(ref tp) = field.ty {
                            if let Some(ident) = tp.path.get_ident() {
                                if ident != "u8" {
                                    let error = syn::Error::new(
                                        ident.span(),
                                        "U8 attribute can only be used with the u8 type",
                                    );
                                    errors.push(error.to_compile_error());
                                    continue;
                                }
                            }
                        }
                        if pos.is_none() && size.is_none() {
                            let error = syn::Error::new(
                                field.span(),
                                "U8 attribute must have a size and a position",
                            );
                            errors.push(error.to_compile_error());
                            continue;
                        }
                        // Deal with the position and size
                        if let (Some(pos), Some(size)) = (pos, size) {
                            // set the bit mask
                            let mask = (1 << size) - 1;

                            // increase the bit sum by the size requested
                            u8_bit_sum += size;
                            // check which byte we're in
                            let u8_byte_index = (u8_bit_sum - 1) / 8;

                            // add runtime check if the value requested is in the valid range for that type
                            field_limit_check.push(quote! {
                                if #field_name > #mask as #field_type {
                                    let err_msg = format!(
                                        "Value of field {} is out of range (max value: {})",
                                        stringify!(#field_name),
                                        #mask
                                    );

                                    let err = std::io::Error::new(std::io::ErrorKind::Other, err_msg);
                                    return Err(std::boxed::Box::new(err));
                                }
                            });

                            // check if the position is in sequence
                            if pos != total_size % 8 {
                                let message = format!(
                                "U8 attributes must obey the sequence specified by the previous attributes. Expected position {} but got {}",
                                total_size % 8, pos
                            );
                                errors.push(
                                    syn::Error::new_spanned(&field, message).to_compile_error(),
                                );
                            }
                            // add the parsing code for the field
                            field_parsing.push(quote! {
                                // println!("{} byte_index: {} bit_sum: {}", stringify!(#field_name), #u8_byte_index, bit_sum);
                                bit_sum += #size;
                                let #field_name = ((bytes[#u8_byte_index] as #field_type) >> (7 - (#size + #pos - 1) as #field_type )) & (#mask as #field_type);
                            });

                            // add the writing code for the field
                            field_writing.push(quote! {
                            if (#field_name as u8) & !(#mask as u8) != 0 {
                                panic!(
                                    "Value {} for field {} exceeds the maximum allowed value {}.",
                                    #field_name,
                                    stringify!(#field_name),
                                    #mask
                                );
                            }
                            if bytes.len() <= #u8_byte_index {
                                bytes.resize(#u8_byte_index + 1, 0);
                            }
                            bytes[#u8_byte_index] |= (#field_name as u8) << (7 - (#size - 1) - #pos );

                        });
                            // last_pos = Some(pos);
                            total_size += size;
                        }
                    } else {
                        // if field is not U8, total_size has to be a multiple of 8
                        if total_size % 8 != 0 {
                            errors.push(
                                syn::Error::new_spanned(
                                    &field,
                                    "U8 attributes must add up to 8 before any other field",
                                )
                                .to_compile_error(),
                            );
                        }
                        // supported types
                        match field_type {
                            // if field is number type, we apply be bytes conversion
                            syn::Type::Path(tp)
                                if tp.path.is_ident("i8")
                                    || tp.path.is_ident("u8")
                                    || tp.path.is_ident("i16")
                                    || tp.path.is_ident("u16")
                                    || tp.path.is_ident("i32")
                                    || tp.path.is_ident("u32")
                                    || tp.path.is_ident("f32")
                                    || tp.path.is_ident("i64")
                                    || tp.path.is_ident("u64")
                                    || tp.path.is_ident("f64")
                                    || tp.path.is_ident("i128")
                                    || tp.path.is_ident("u128") =>
                            {
                                // get the size of the number in bytes
                                let field_size =
                                    match get_number_size(field_type, &field, &mut errors) {
                                        Some(value) => value,
                                        None => continue,
                                    };

                                // write the parse and writing code for the field
                                parse_write_number(
                                    field_size,
                                    &mut field_parsing,
                                    &field_name,
                                    field_type,
                                    &mut field_writing,
                                );
                            }
                            // if field is an Array
                            syn::Type::Array(tp) => {
                                eprintln!("tp {:#?}", tp);
                                let array_length: usize;
                                let len = tp.len.clone();
                                match len {
                                    syn::Expr::Lit(expr_lit) => {
                                        if let syn::Lit::Int(token) = expr_lit.lit {
                                            array_length = token.base10_parse().unwrap();
                                        } else {
                                            let error = syn::Error::new(
                                                field.ty.span(),
                                                "Expected integer type for N",
                                            );
                                            errors.push(error.to_compile_error());
                                            continue;
                                        }
                                    }
                                    _ => {
                                        let error = syn::Error::new(
                                            tp.span(),
                                            "Unsupported type for [T; N]",
                                        );
                                        errors.push(error.to_compile_error());
                                        continue;
                                    }
                                }
                                if let syn::Type::Path(elem) = *tp.elem.clone() {
                                    // Retrieve type segments
                                    let syn::TypePath {
                                        path: syn::Path { segments, .. },
                                        ..
                                    } = elem;

                                    match &segments[0] {
                                        syn::PathSegment {
                                            ident,
                                            arguments: syn::PathArguments::None,
                                        } if ident == "u8" => {
                                            field_parsing.push(quote! {
                                                byte_index = bit_sum / 8;
                                                // println!("{} by te_index: {} bit_sum: {}", stringify!(#field_name), byte_index, bit_sum);
                                                let mut #field_name = [0u8; #array_length];
                                                #field_name.copy_from_slice(&bytes[byte_index..#array_length]);
                                                bit_sum += 8 * #array_length;
                                            });
                                            field_writing.push(quote! {
                                                // Vec type
                                                bytes.extend_from_slice(&#field_name);
                                            });
                                        }
                                        _ => {
                                            let error = syn::Error::new(
                                                field.ty.span(),
                                                "Unsupported type for [T; N]",
                                            );
                                            errors.push(error.to_compile_error());
                                            continue;
                                        }
                                    };
                                }
                            }
                            // if field is a non-empty Vec
                            syn::Type::Path(tp)
                                if tp.path.segments.len() > 0
                                    && tp.path.segments[0].ident.to_string() == "Vec" =>
                            {
                                let inner_type = match solve_for_inner_type(&tp, "Vec") {
                                    Some(t) => t,
                                    None => {
                                        let error = syn::Error::new(
                                            field.ty.span(),
                                            "Unsupported type for Vec<T>",
                                        );
                                        errors.push(error.to_compile_error());
                                        continue;
                                    }
                                };

                                if let syn::Type::Path(inner_tp) = &inner_type {
                                    if inner_tp.path.is_ident("i8")
                                        || inner_tp.path.is_ident("u8")
                                        || inner_tp.path.is_ident("i16")
                                        || inner_tp.path.is_ident("u16")
                                        || inner_tp.path.is_ident("i32")
                                        || inner_tp.path.is_ident("u32")
                                        || inner_tp.path.is_ident("f32")
                                        || inner_tp.path.is_ident("i64")
                                        || inner_tp.path.is_ident("u64")
                                        || inner_tp.path.is_ident("f64")
                                        || inner_tp.path.is_ident("i128")
                                        || inner_tp.path.is_ident("u128")
                                    {
                                        field_parsing.push(quote! {
                                            // Vec type
                                            byte_index = bit_sum / 8;
                                            // println!("{} byte_index: {} bit_sum: {}", stringify!(#field_name), byte_index, bit_sum);
                                            let #field_name = Vec::from(&bytes[byte_index..]);
                                        });
                                        field_writing.push(quote! {
                                            // Vec type
                                            bytes.extend_from_slice(&#field_name);
                                        });

                                        // If the current field is not the last field, raise an error
                                        if !is_last_field {
                                            let error = syn::Error::new(
                                                field.ty.span(),
                                                "Vectors can only be used for padding the end of a struct",
                                            );
                                            errors.push(error.to_compile_error());
                                        }
                                    } else {
                                        let error = syn::Error::new(
                                            inner_type.span(),
                                            "Unsupported type for Vec<T>",
                                        );
                                        errors.push(error.to_compile_error());
                                        continue;
                                    }
                                }
                            }
                            syn::Type::Path(tp)
                                if tp.path.segments.len() > 0
                                    && tp.path.segments[0].ident.to_string() == "Option" =>
                            {
                                // if field is a non-empty Option
                                if tp.path.segments.len() > 0
                                    && tp.path.segments[0].ident.to_string() == "Option"
                                {
                                    let inner_type = match solve_for_inner_type(&tp, "Option") {
                                        Some(t) => t,
                                        None => {
                                            let error = syn::Error::new(
                                                field.ty.span(),
                                                "Unsupported type for Option<T>",
                                            );
                                            errors.push(error.to_compile_error());
                                            continue;
                                        }
                                    };

                                    if let syn::Type::Path(inner_tp) = &inner_type {
                                        if inner_tp.path.is_ident("i8")
                                            || inner_tp.path.is_ident("u8")
                                            || inner_tp.path.is_ident("i16")
                                            || inner_tp.path.is_ident("u16")
                                            || inner_tp.path.is_ident("i32")
                                            || inner_tp.path.is_ident("u32")
                                            || inner_tp.path.is_ident("f32")
                                            || inner_tp.path.is_ident("i64")
                                            || inner_tp.path.is_ident("u64")
                                            || inner_tp.path.is_ident("f64")
                                            || inner_tp.path.is_ident("i128")
                                            || inner_tp.path.is_ident("u128")
                                        {
                                            // get the size of the number in bytes
                                            let field_size = match get_number_size(
                                                &inner_type,
                                                &field,
                                                &mut errors,
                                            ) {
                                                Some(value) => value,
                                                None => continue,
                                            };
                                            field_parsing.push(quote! {
                                                // Option type
                                                byte_index = bit_sum / 8;
                                                end_byte_index = byte_index + #field_size;
                                                let #field_name = if bytes[byte_index..end_byte_index] == [0_u8; #field_size] {
                                                    None
                                                } else {
                                                    // println!("{} byte_index: {} bit_sum: {}", stringify!(#field_name), byte_index, bit_sum);
                                                    bit_sum += 8 * #field_size;
                                                    Some(<#inner_tp>::from_be_bytes({
                                                        let slice = &bytes[byte_index..end_byte_index];
                                                        let mut arr = [0; #field_size];
                                                        arr.copy_from_slice(slice);
                                                        arr
                                                    }))
                                                };
                                            });
                                            field_writing.push(quote! {
                                                bytes.extend_from_slice(&#field_name.unwrap_or(0).to_be_bytes());
                                            });
                                        } else {
                                            let error = syn::Error::new(
                                                inner_type.span(),
                                                "Unsupported type for Option<T>",
                                            );
                                            errors.push(error.to_compile_error());
                                            continue;
                                        }
                                    }
                                }
                            }
                            syn::Type::Path(tp)
                                if tp.path.segments.len() > 0
                                    && !is_primitive_type(&tp.path.segments[0].ident) =>
                            {
                                // Struct case
                                field_parsing.push(quote_spanned! { field.span() =>
                                    byte_index = bit_sum / 8;
                                    let predicted_size = core::mem::size_of::<#field_type>();
                                    end_byte_index = byte_index + predicted_size;
                                    // println!("{} byte_index: {} bit_sum: {}", stringify!(#field_name), byte_index, bit_sum);
                                    bit_sum += (end_byte_index - byte_index) * 8;
                                    let (#field_name, bytes_written) = #field_type::try_from_be_bytes(&bytes[byte_index..end_byte_index])?;
                                    // println!("----------  {} bytes_written: {}", stringify!(#field_name), bytes_written);
                                    bit_sum -= (predicted_size - bytes_written) * 8;
                                });
                                field_writing.push(quote_spanned! { field.span() =>
                                    bytes.extend_from_slice(&message_macro::BeBytes::to_be_bytes(&#field_name));
                                });
                            }
                            _ => {
                                let error_message = format!(
                                    "Unsupported type for field {}",
                                    field_name.to_string()
                                );
                                let error = syn::Error::new(field.ty.span(), error_message);
                                errors.push(error.to_compile_error());
                                continue;
                            }
                        }
                    }
                }

                let struct_field_names = fields.named.iter().map(|f| &f.ident).collect::<Vec<_>>();
                let constructor_arg_list = fields.named.iter().map(|f| {
                    let field_ident = &f.ident;
                    let field_type = &f.ty;
                    quote! { #field_ident: #field_type }
                });
                let expanded = quote! {
                    impl #my_trait_path for #name {
                        fn try_from_be_bytes(bytes: &[u8]) -> Result<(Self, usize), Box<dyn std::error::Error>> {
                            let mut bit_sum = 0;
                            let mut byte_index = 0;
                            let mut end_byte_index = 0;
                            #(#field_parsing)*
                            Ok((Self {
                                #( #struct_field_names: #struct_field_names, )*
                            }, bit_sum / 8))
                        }

                        fn to_be_bytes(&self) -> Vec<u8> {
                            let mut bytes = Vec::new();
                            #( {
                                let #struct_field_names = self.#struct_field_names.to_owned();
                                #field_writing
                            } )*
                            bytes
                        }

                        fn field_size(&self) -> usize {
                            std::mem::size_of_val(self)
                        }
                    }

                    impl #name {
                        #[allow(clippy::too_many_arguments)]
                        pub fn new(#(#constructor_arg_list,)*) -> Result<Self, Box<dyn std::error::Error>> {
                            #(#field_limit_check)*

                            Ok(Self {
                                #( #struct_field_names: #struct_field_names, )*
                            })
                        }

                    }

                };

                let output = quote! {
                    #expanded
                    #(#errors)*
                };

                output.into()
            }
            field => {
                let error = syn::Error::new(field.span(), "Only named fields are supported")
                    .to_compile_error();
                let output = quote! {
                    #error
                };

                output.into()
            }
        },
        Data::Enum(data_enum) => {
            eprintln!("data_enum {:#?}", data_enum);
            let output = quote! {};

            output.into()
        }
        _ => {
            let error =
                syn::Error::new(Span::call_site(), "Only Structs are supported").to_compile_error();
            let output = quote! {
                #error
            };

            output.into()
        }
    }
}

fn parse_write_number(
    field_size: usize,
    field_parsing: &mut Vec<quote::__private::TokenStream>,
    field_name: &syn::Ident,
    field_type: &syn::Type,
    field_writing: &mut Vec<quote::__private::TokenStream>,
) {
    field_parsing.push(quote! {
        byte_index = bit_sum / 8;
        // println!("{} pwn byte_index: {} bit_sum: {}", stringify!(#field_name), byte_index, bit_sum);
        end_byte_index = byte_index + #field_size;
        bit_sum += 8 * #field_size;
        let #field_name = <#field_type>::from_be_bytes({
            let slice = &bytes[byte_index..end_byte_index];
            let mut arr = [0; #field_size];
            arr.copy_from_slice(slice);
            arr
        });
    });
    field_writing.push(quote! {
        // bytes[#byte_index..#end_byte_index].copy_from_slice(&#field_name.to_be_bytes());
        bytes.extend_from_slice(&#field_name.to_be_bytes());
    });
}

fn get_number_size(
    field_type: &syn::Type,
    field: &syn::Field,
    errors: &mut Vec<quote::__private::TokenStream>,
) -> Option<usize> {
    if let syn::Type::Path(ref tp) = field_type {
        if let Some(inner_type) = solve_for_inner_type(tp, "Vec") {
            return match &inner_type {
                syn::Type::Path(tp) if tp.path.is_ident("u8") => Some(1),
                _ => {
                    let error = syn::Error::new(inner_type.span(), "Unsupported type for Vec<T>");
                    errors.push(error.to_compile_error());
                    None
                }
            };
        }
    }
    let field_size = match &field_type {
        syn::Type::Path(tp) if tp.path.is_ident("i8") || tp.path.is_ident("u8") => 1,
        syn::Type::Path(tp) if tp.path.is_ident("i16") || tp.path.is_ident("u16") => 2,
        syn::Type::Path(tp)
            if tp.path.is_ident("i32") || tp.path.is_ident("u32") || tp.path.is_ident("f32") =>
        {
            4
        }
        syn::Type::Path(tp)
            if tp.path.is_ident("i64") || tp.path.is_ident("u64") || tp.path.is_ident("f64") =>
        {
            8
        }
        syn::Type::Path(tp) if tp.path.is_ident("i128") || tp.path.is_ident("u128") => 16,
        _ => {
            let error = syn::Error::new(field.ty.span(), "Unsupported type");
            errors.push(error.to_compile_error());
            return None;
        }
    };
    Some(field_size)
}

fn parse_u8_attribute(
    attributes: Vec<syn::Attribute>,
    u8_attribute_present: &mut bool,
    errors: &mut Vec<quote::__private::TokenStream>,
    non_bit_fields: &mut usize,
) -> (Option<usize>, Option<usize>) {
    let mut pos = None;
    let mut size = None;

    for attr in attributes {
        if attr.path().is_ident("U8") {
            *u8_attribute_present = true;
            let nested_result = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("pos") || meta.path.is_ident("size") {
                    if meta.path.is_ident("pos") {
                        let content;
                        parenthesized!(content in meta.input);
                        let lit: LitInt = content.parse()?;
                        let n: usize = lit.base10_parse()?;
                        pos = Some(n);
                        return Ok(());
                    }
                    if meta.path.is_ident("size") {
                        let content;
                        parenthesized!(content in meta.input);
                        let lit: LitInt = content.parse()?;
                        let n: usize = lit.base10_parse()?;
                        size = Some(n);
                        return Ok(());
                    }
                } else {
                    return Err(meta.error(format!(
                        "Allowed attributes are `pos` and `size` - Example: #[U8(pos=1, size=3)]"
                    )));
                }
                Ok(())
            });
            if let Err(e) = nested_result {
                errors.push(e.to_compile_error());
            }
        } else {
            *non_bit_fields += 1;
        }
    }
    (pos, size)
}

/// Given a type and an identifier, `solve_for_inner_type` attempts to retrieve the inner type of the input type
/// that is wrapped by the provided identifier. If the input type does not contain the specified identifier or
/// has more than one generic argument, the function returns `None`.
fn solve_for_inner_type(input: &syn::TypePath, identifier: &str) -> Option<syn::Type> {
    // Retrieve type segments
    let syn::TypePath {
        path: syn::Path { segments, .. },
        ..
    } = input;

    let args = match &segments[0] {
        syn::PathSegment {
            ident,
            arguments:
                syn::PathArguments::AngleBracketed(AngleBracketedGenericArguments { args, .. }),
        } if ident == identifier && args.len() == 1 => args,
        _ => return None,
    };

    let inner_type = match &args[0] {
        syn::GenericArgument::Type(t) => t,
        _ => return None,
    };

    Some(inner_type.clone())
}

// Helper function to check if a given identifier is a primitive type
fn is_primitive_type(ident: &syn::Ident) -> bool {
    let primitives = [
        "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize",
        "f32", "f64", "char", "bool", "str",
    ];

    primitives.iter().any(|&primitive| ident == primitive)
}
