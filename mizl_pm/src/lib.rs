use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Field, Fields, Type, parse_macro_input};

const DEBUG: bool = false;

#[proc_macro_derive(FfiSerialize, attributes(ffi_serialize_enum, ffi_inline_vec))]
pub fn ffi_serialize_derive(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    match make_ffi_serialize(&ast) {
        Ok(ts) => {
            if DEBUG {
                _ = std::fs::create_dir_all("mizl_pm/_debug");
                _ = std::fs::write(format!("mizl_pm/_debug/test_{}.rs", ast.ident), ts.to_string()).ok();
            }
            ts.into()
        }
        Err(e) => e.to_compile_error().into(),
    }
}

enum FieldKind {
    Primitive {
        align_expr: TokenStream2,
        size_expr: TokenStream2,
        type_expr: TokenStream2,
    },
    Enum,
    String,
    Vec(Box<Type>, bool),
    // VecOfString(bool),
    VecOfPrimitive {
        align_expr: TokenStream2,
        size_expr: TokenStream2,
        type_expr: TokenStream2,
        inline: bool,
    },
    ChildStruct(Box<Type>),
}

fn is_serializable_enum_field(field: &Field) -> bool {
    field
        .attrs
        .iter()
        .any(|attr| attr.path().is_ident("ffi_serialize_enum"))
}

fn get_primitive_field_info(ty: &Type) -> Option<(TokenStream2, TokenStream2, TokenStream2)> {
    let name = if let Type::Path(tp) = ty {
        tp.path.get_ident()?.to_string()
    } else {
        return None;
    };

    let (align_expr, size_expr, type_expr) = match name.as_str() {
        "u8" => (quote!(I8_SA), quote!(I8_SZ), quote!(u8)),
        "i8" => (quote!(I8_SA), quote!(I8_SZ), quote!(i8)),
        "u16" => (quote!(I16_SA), quote!(I16_SZ), quote!(u16)),
        "i16" => (quote!(I16_SA), quote!(I16_SZ), quote!(i16)),
        "u32" => (quote!(I32_SA), quote!(I32_SZ), quote!(u32)),
        "i32" => (quote!(I32_SA), quote!(I32_SZ), quote!(i32)),
        "f32" => (quote!(I32_SA), quote!(I32_SZ), quote!(f32)),
        "u64" => (quote!(I64_SA), quote!(I64_SZ), quote!(u64)),
        "i64" => (quote!(I64_SA), quote!(I64_SZ), quote!(i64)),
        "f64" => (quote!(I64_SA), quote!(I64_SZ), quote!(f64)),
        "isize" => (quote!(WORD_SA), quote!(WORD_SZ), quote!(isize)),
        "usize" => (quote!(WORD_SA), quote!(WORD_SZ), quote!(usize)),
        _ => return None,
    };
    Some((align_expr, size_expr, type_expr))
}

fn path_ident_eq(ty: &Type, name: &str) -> bool {
    if let Type::Path(tp) = ty {
        tp.path.segments.last().map_or(false, |s| s.ident == name)
    } else {
        false
    }
}

fn vec_inner(ty: &Type) -> Option<Type> {
    if let Type::Path(tp) = ty {
        let seg = tp.path.segments.last()?;
        if seg.ident != "Vec" {
            return None;
        }
        if let syn::PathArguments::AngleBracketed(ref ab) = seg.arguments {
            if let Some(syn::GenericArgument::Type(inner)) = ab.args.first() {
                return Some(inner.clone());
            }
        }
    }
    None
}

fn get_field_ffi_type(field: &Field, can_be_inlined: bool) -> FieldKind {
    let field_type = &field.ty;
    if let Some(prim_inf) = get_primitive_field_info(field_type) {
        FieldKind::Primitive {
            align_expr: prim_inf.0,
            size_expr: prim_inf.1,
            type_expr: prim_inf.2,
        }
    } else if is_serializable_enum_field(&field) {
        FieldKind::Enum
    } else if path_ident_eq(field_type, "String") {
        FieldKind::String
    } else if let Some(inner) = vec_inner(field_type) {
        // let inline_vec = can_be_inlined && is_inline_vec(&field);
        if let Some(prim_inf) = get_primitive_field_info(&inner) {
            FieldKind::VecOfPrimitive {
                align_expr: prim_inf.0,
                size_expr: prim_inf.1,
                type_expr: prim_inf.2,
                inline: false,
            }
        } else {
            FieldKind::Vec(Box::new(inner), false)
        }
    } else {
        FieldKind::ChildStruct(Box::new(field_type.clone()))
    }
}

fn get_ffi_token_from_base(base_type: &Type) -> TokenStream2 {
    if let Type::Path(tp) = base_type {
        let seg = tp.path.segments.last().unwrap();
        let ffi_type_ident = format_ident!("{}Ffi", seg.ident);
        quote! { #ffi_type_ident }
    } else {
        // shouldn't happen
        quote! { InvalidFfi }
    }
}

// /////

fn make_ffi_serialize(ast: &DeriveInput) -> syn::Result<TokenStream2> {
    let name = &ast.ident;
    let ffi_name = format_ident!("{name}Ffi");

    let fields = match &ast.data {
        Data::Struct(ds) => match &ds.fields {
            Fields::Named(nf) => nf.named.iter().collect::<Vec<_>>(),
            _ => return Err(syn::Error::new_spanned(ast, "field must be named")),
        },
        _ => return Err(syn::Error::new_spanned(ast, "field must be struct type")),
    };

    let calc_align_body = make_calc_align_body(&fields);
    let calc_base_size_body = make_calc_base_size_body(&fields);
    let calc_full_size_body = make_calc_full_size_body(&fields);
    let has_dynamic_size_body = make_has_dynamic_size_body(&fields);
    let has_var_length_field_body = make_has_var_length_field_body(&fields);
    let serialize_body = make_serialize_body(&fields);

    Ok(quote! {
        pub struct #ffi_name;

        // necessary so we can use const functions.
        // this feels kind of wrong...
        impl #ffi_name {
            pub const fn calculate_alignment() -> usize { #calc_align_body }
            pub const fn calculate_base_size() -> usize { #calc_base_size_body }
            pub fn calculate_full_size(obj: &#name) -> usize { #calc_full_size_body }
            pub const fn has_dynamic_size() -> bool { #has_dynamic_size_body }
            pub const fn has_var_length_field() -> bool { #has_var_length_field_body }
            pub unsafe fn serialize(ptrd: *mut u8, obj: &#name) -> *mut u8 { #serialize_body }
        }

        impl FfiSerializer for #ffi_name {
            type Target = #name;

            fn calculate_alignment() -> usize { #ffi_name::calculate_alignment() }
            fn calculate_base_size() -> usize { #ffi_name::calculate_base_size() }
            fn calculate_full_size(obj: &#name) -> usize { #ffi_name::calculate_full_size(obj) }
            fn has_dynamic_size() -> bool { #ffi_name::has_dynamic_size() }
            fn has_var_length_field() -> bool { #ffi_name::has_var_length_field() }
            unsafe fn serialize(ptrd: *mut u8, obj: &#name) -> *mut u8 { #ffi_name::serialize(ptrd, obj) }
        }

        impl FfiSerializeTrait for #name {
            type Ffi = #ffi_name;
        }
    })
}

fn make_calc_align_body(fields: &[&Field]) -> TokenStream2 {
    // currently, we only have primitives (up to 64-bit) or pointers,
    // so we don't have to worry about anything above 8 byte right now.
    let mut align_exprs: Vec<TokenStream2> = Vec::new();
    for (i, field) in fields.iter().enumerate() {
        align_exprs.push(match get_field_ffi_type(field, i == 0) {
            FieldKind::String | FieldKind::Vec(_, _) | FieldKind::VecOfPrimitive { .. } | FieldKind::ChildStruct(_) => {
                quote! { WORD_SA }
            }
            FieldKind::Primitive { align_expr, .. } => align_expr,
            FieldKind::Enum => quote! { I32_SA },
        });
    }

    quote! {
        let mut align = 1usize;
        #( align = max_const_usize(align, #align_exprs); )*
        align
    }
}

fn make_calc_base_size_body(fields: &[&Field]) -> TokenStream2 {
    // calculate the static fields of a given struct.
    let mut size_stmts: Vec<TokenStream2> = Vec::new();

    for (i, field) in fields.iter().enumerate() {
        size_stmts.push(match get_field_ffi_type(field, i == 0) {
            FieldKind::String | FieldKind::ChildStruct(_) => {
                quote! {
                    // pointer only
                    size = align_usize_fast_const::<WORD_SA>(size);
                    size += WORD_SZ;
                }
            }
            FieldKind::Vec(_, inline) | FieldKind::VecOfPrimitive { inline, .. } => {
                if inline {
                    quote! {
                        // no size
                        size += 0;
                    }
                } else {
                    // normal vec
                    quote! {
                        // pointer only
                        size = align_usize_fast_const::<WORD_SA>(size);
                        size += WORD_SZ;
                    }
                }
            }
            FieldKind::Primitive {
                align_expr, size_expr, ..
            } => {
                quote! {
                    size = align_usize_fast_const::<#align_expr>(size);
                    size += #size_expr;
                }
            }
            FieldKind::Enum => {
                quote! {
                    size = align_usize_fast_const::<I32_SA>(size);
                    size += I32_SZ;
                }
            }
        })
    }

    quote! {
        let mut size = 0usize;
        #( #size_stmts )*
        size
    }
}

fn make_calc_full_size_body(fields: &[&Field]) -> TokenStream2 {
    // calculate the dynamic fields of a given struct.
    // the static field sizes will be summed up in the base size.
    let mut size_stmts: Vec<TokenStream2> = Vec::new();

    for (i, field) in fields.iter().enumerate() {
        let name = field.ident.as_ref().unwrap();

        match get_field_ffi_type(field, i == 0) {
            FieldKind::String => {
                size_stmts.push(quote! {
                    size = align_usize_fast_const::<I32_SZ>(size + I32_SZ);
                    size += obj.#name.len() + 1;
                });
            }
            FieldKind::Vec(ele_type, inline) => {
                let ele_ffi = get_ffi_token_from_base(&ele_type);
                if inline {
                    size_stmts.push(quote! {
                        // pointer array
                        size = obj.#name.len() * WORD_SZ;
                    });
                } else {
                    size_stmts.push(quote! {
                        // pointer array
                        size = align_usize_fast_const::<WORD_SA>(size + I32_SZ);
                        size += obj.#name.len() * WORD_SZ;
                    });
                }
                size_stmts.push(quote! {
                    // element data
                    if #ele_ffi::has_dynamic_size() {
                        // slow length calculation if element size is dynamic
                        for elem in &obj.#name {
                            size = align_usize_fast_const::<{ #ele_ffi::calculate_alignment() }>(size);
                            size += #ele_ffi::calculate_full_size(elem);
                        }
                    } else {
                        // optimized length calculation if element size is constant
                        size = align_usize_fast_const::<{ #ele_ffi::calculate_alignment() }>(size);
                        size += #ele_ffi::calculate_base_size() * obj.#name.len();
                    }
                });
            }
            FieldKind::VecOfPrimitive {
                align_expr,
                size_expr,
                inline,
                ..
            } => {
                if inline {
                    size_stmts.push(quote! {
                        size = obj.#name.len() * #size_expr;
                    });
                } else {
                    size_stmts.push(quote! {
                        size = align_usize_fast_const::<{ max_const_usize(I32_SA, #align_expr) }>(size + I32_SZ);
                        size += obj.#name.len() * #size_expr;
                    });
                }
            }
            FieldKind::ChildStruct(child_type) => {
                let child_ffi = get_ffi_token_from_base(&child_type);
                size_stmts.push(quote! {
                    size = align_usize_fast_const::<{ #child_ffi::calculate_alignment() }>(size);
                    size += #child_ffi::calculate_full_size(&obj.#name);
                });
            }
            FieldKind::Primitive { .. } | FieldKind::Enum => {
                // do nothing, these have no dynamic size
            }
        };
    }

    quote! {
        let mut size = Self::calculate_base_size();
        #( #size_stmts )*
        size
    }
}

fn make_has_dynamic_size_body(fields: &[&Field]) -> TokenStream2 {
    // does object use dynamic size?
    // only used for Vec's full size calculation optimization.
    let any_dynamic = fields.iter().any(|f| {
        matches!(
            get_field_ffi_type(f, false),
            FieldKind::String | FieldKind::Vec(_, _) | FieldKind::ChildStruct(_)
        )
    });
    quote! { #any_dynamic }
}

fn make_has_var_length_field_body(_fields: &[&Field]) -> TokenStream2 {
    // does field need four byte length prefix?
    // this is currently unused but may be used in the future.
    let any_length = quote! { false };
    quote! { #any_length }
}

fn make_serialize_body(fields: &[&Field]) -> TokenStream2 {
    // serialize data given our allocated buffer is large enough
    // static data is put in the fixed_stmts vec, while
    // dynamic data is put in the dynamic_stmts vec.

    let mut fixed_stmts: Vec<TokenStream2> = Vec::new();
    let mut dynamic_stmts: Vec<TokenStream2> = Vec::new();

    let mut str_ptr_idx = 0;
    let mut vec_ptr_idx = 0;
    let mut chd_ptr_idx = 0;

    for (i, field) in fields.iter().enumerate() {
        let name = field.ident.as_ref().unwrap();
        let ffi_type = get_field_ffi_type(field, i == 0);

        match ffi_type {
            FieldKind::String => {
                let data_ptr = format_ident!("str_ptr_{}", str_ptr_idx.to_string());
                str_ptr_idx += 1;

                dynamic_stmts.push(quote! {
                    // align to start position, write length, and remember start position
                    ptrd_dyn = align_ptr_fast::<I32_SA>(ptrd_dyn.add(I32_SZ));
                    *(ptrd_dyn.sub(I32_SZ) as *mut u32) = obj.#name.len() as u32;
                    let #data_ptr = ptrd_dyn;

                    // copy string data and add null term
                    std::ptr::copy_nonoverlapping(obj.#name.as_ptr(), ptrd_dyn, obj.#name.len());
                    *ptrd_dyn.add(obj.#name.len()) = 0u8;

                    // seek forward string length + null term
                    ptrd_dyn = ptrd_dyn.add(obj.#name.len() + 1);
                });

                fixed_stmts.push(quote! {
                    ptrd = align_ptr_fast::<WORD_SA>(ptrd);
                    *(ptrd as *mut *mut u8) = #data_ptr;
                    ptrd = ptrd.add(WORD_SZ);
                });
            }
            FieldKind::Vec(ele_type, inline) => {
                if inline {
                    // todo: DELETE INLINE, WE HANDLE IT NOW!!!!
                    let ele_ffi = get_ffi_token_from_base(&ele_type);
                    let ptr_array = format_ident!("vec_array_{}", vec_ptr_idx.to_string());
                    vec_ptr_idx += 1;

                    dynamic_stmts.push(quote! {
                        // write length
                        *(ptrd_dyn.sub(I32_SZ) as *mut u32) = obj.#name.len() as u32;

                        // also make a copy pointer for writing the pointer array and move to end
                        let mut #ptr_array = ptrd_dyn;
                        ptrd_dyn = ptrd_dyn.add(obj.#name.len() * WORD_SZ);

                        // align and write each element
                        ptrd_dyn = align_ptr_fast::<{ #ele_ffi::calculate_alignment() }>(ptrd_dyn);
                        for elem in &obj.#name {
                            ptrd_dyn = align_ptr_fast::<{ #ele_ffi::calculate_alignment() }>(ptrd_dyn);
                            *(#ptr_array as *mut *mut u8) = ptrd_dyn;
                            #ptr_array = #ptr_array.add(WORD_SZ);
                            ptrd_dyn = #ele_ffi::serialize(ptrd_dyn, elem);
                        }
                    });
                } else {
                    let ele_ffi = get_ffi_token_from_base(&ele_type);
                    let data_ptr = format_ident!("vec_ptr_{}", vec_ptr_idx.to_string());
                    let ptr_array = format_ident!("vec_array_{}", vec_ptr_idx.to_string());
                    vec_ptr_idx += 1;

                    dynamic_stmts.push(quote! {
                        // align to start position, write length, and remember start position
                        ptrd_dyn = align_ptr_fast::<WORD_SA>(ptrd_dyn.add(I32_SZ));
                        *(ptrd_dyn.sub(I32_SZ) as *mut u32) = obj.#name.len() as u32;
                        let #data_ptr = ptrd_dyn;

                        // also make a copy pointer for writing the pointer array and move to end
                        let mut #ptr_array = ptrd_dyn;
                        ptrd_dyn = ptrd_dyn.add(obj.#name.len() * WORD_SZ);

                        // align and write each element
                        for elem in &obj.#name {
                            ptrd_dyn = align_ptr_fast::<{ #ele_ffi::calculate_alignment() }>(ptrd_dyn);
                            *(#ptr_array as *mut *mut u8) = ptrd_dyn;
                            #ptr_array = #ptr_array.add(WORD_SZ);
                            ptrd_dyn = #ele_ffi::serialize(ptrd_dyn, elem);
                        }
                    });

                    fixed_stmts.push(quote! {
                        ptrd = align_ptr_fast::<WORD_SA>(ptrd);
                        *(ptrd as *mut *mut u8) = #data_ptr;
                        ptrd = ptrd.add(WORD_SZ);
                    });
                }
            }
            FieldKind::VecOfPrimitive {
                align_expr,
                size_expr,
                type_expr,
                inline,
            } => {
                if inline {
                    dynamic_stmts.push(quote! {
                        // write length
                        *(ptrd_dyn.sub(I32_SZ) as *mut u32) = obj.#name.len() as u32;

                        for elem in &obj.#name {
                            *(ptrd_dyn as *mut #type_expr) = *elem;
                            ptrd_dyn = ptrd_dyn.add(#size_expr);
                        }
                    });
                } else {
                    let data_ptr = format_ident!("pvec_ptr_{}", vec_ptr_idx.to_string());
                    vec_ptr_idx += 1;

                    dynamic_stmts.push(quote! {
                        // align to start position, write length, and remember start position
                        ptrd_dyn = align_ptr_fast::<{ max_const_usize(I32_SA, #align_expr) }>(ptrd_dyn.add(I32_SZ));
                        *(ptrd_dyn.sub(I32_SZ) as *mut u32) = obj.#name.len() as u32;
                        let #data_ptr = ptrd_dyn;

                        for elem in &obj.#name {
                            *(ptrd_dyn as *mut #type_expr) = *elem;
                            ptrd_dyn = ptrd_dyn.add(#size_expr);
                        }
                    });

                    fixed_stmts.push(quote! {
                        ptrd = align_ptr_fast::<WORD_SA>(ptrd);
                        *(ptrd as *mut *mut u8) = #data_ptr;
                        ptrd = ptrd.add(WORD_SZ);
                    });
                }
            }
            FieldKind::ChildStruct(child_type) => {
                let child_ffi = get_ffi_token_from_base(&child_type);
                let data_ptr = format_ident!("chd_ptr_{}", chd_ptr_idx.to_string());
                chd_ptr_idx += 1;

                dynamic_stmts.push(quote! {
                    ptrd_dyn = align_ptr_fast::<{ #child_ffi::calculate_alignment() }>(ptrd_dyn);
                    let #data_ptr = ptrd_dyn;
                    ptrd_dyn = #child_ffi::serialize(ptrd_dyn, &obj.#name);
                });

                fixed_stmts.push(quote! {
                    ptrd = align_ptr_fast::<WORD_SA>(ptrd);
                    *(ptrd as *mut *mut u8) = #data_ptr;
                    ptrd = ptrd.add(WORD_SZ);
                });
            }
            FieldKind::Primitive {
                align_expr,
                size_expr,
                type_expr,
            } => {
                fixed_stmts.push(quote! {
                    ptrd = align_ptr_fast::<#align_expr>(ptrd);
                    *(ptrd as *mut #type_expr) = obj.#name;
                    ptrd = ptrd.add(#size_expr);
                });
            }
            FieldKind::Enum => {
                fixed_stmts.push(quote! {
                    ptrd = align_ptr_fast::<I32_SA>(ptrd);
                    *(ptrd as *mut u32) = { use num::ToPrimitive as _; obj.#name.to_u32().unwrap() };
                    ptrd = ptrd.add(I32_SZ);
                });
            }
        }
    }

    quote! {
        // move dynamic data pointer to base data pointer after base size
        let mut ptrd = ptrd;
        let mut ptrd_dyn: *mut u8 = ptrd.add(Self::calculate_base_size());
        #( #dynamic_stmts )*
        #( #fixed_stmts )*

        // next static data starts at end of current dynamic data
        ptrd_dyn
    }
}
