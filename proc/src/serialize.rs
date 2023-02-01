use proc_macro2::TokenStream;

use crate::attrs::{parse_attributes, Args, Schema};

struct Config {
    schema: Option<Schema>,
    reference: Option<Schema>,

    variant: Option<syn::Ident>,

    /// Signals if fields should be checked to match on schema.
    /// `false` if `schema` is inferred to `Self`.
    check_fields: bool,
}

impl Config {
    fn for_struct(
        args: Args,
        data: &syn::DataStruct,
        ident: &syn::Ident,
        generics: &syn::Generics,
    ) -> Self {
        let (_, type_generics, _) = generics.split_for_impl();

        match (args.serialize.or(args.common), args.reference) {
            (None, None) if generics.params.is_empty() => Config {
                schema: Some(Schema {
                    ty: syn::parse_quote!(Self),
                    generics: Default::default(),
                }),
                reference: None,
                variant: None,
                check_fields: false,
            },
            (None, Some(None)) if generics.params.is_empty() => Config {
                schema: Some(Schema {
                    ty: syn::parse_quote!(Self),
                    generics: Default::default(),
                }),
                reference: Some(Schema {
                    ty: syn::parse_quote!(#ident #type_generics),
                    generics: syn::Generics {
                        lt_token: Some(Default::default()),
                        params: syn::parse_quote!('__ser),
                        gt_token: Some(Default::default()),
                        where_clause: None,
                    },
                }),
                variant: None,
                check_fields: false,
            },
            (None, reference @ (None | Some(None))) => {
                // Add predicates that fields implement
                // `T: Schema + Serialize<T>`
                let predicates = data
                    .fields
                    .iter()
                    .map(|field| -> syn::WherePredicate {
                        let ty = &field.ty;
                        syn::parse_quote! { #ty: ::alkahest::Schema + ::alkahest::Serialize<#ty> }
                    })
                    .collect();

                let generics = syn::Generics {
                    lt_token: None,
                    params: Default::default(),
                    gt_token: None,
                    where_clause: Some(syn::WhereClause {
                        where_token: Default::default(),
                        predicates,
                    }),
                };

                let schema = Schema {
                    ty: syn::parse_quote!(Self),
                    generics,
                };

                let reference = match &reference {
                    None => None,
                    Some(None) => {
                        // Add predicates that fields implement
                        // `T: Schema` and `&T: Serialize<T>`
                        let predicates = data
                            .fields
                            .iter()
                            .map(|field| -> syn::WherePredicate {
                                let ty = &field.ty;
                                syn::parse_quote! { #ty: ::alkahest::Schema }
                            })
                            .chain(data.fields.iter().map(|field| -> syn::WherePredicate {
                                let ty = &field.ty;
                                syn::parse_quote! { &'__ser #ty: ::alkahest::Serialize<#ty> }
                            }))
                            .collect();

                        let generics = syn::Generics {
                            lt_token: Some(Default::default()),
                            params: syn::parse_quote!('__ser),
                            gt_token: Some(Default::default()),
                            where_clause: Some(syn::WhereClause {
                                where_token: Default::default(),
                                predicates,
                            }),
                        };

                        Some(Schema {
                            ty: syn::parse_quote!(#ident #type_generics),
                            generics,
                        })
                    }
                    Some(Some(_)) => unreachable!(),
                };

                Config {
                    schema: Some(schema),
                    reference,
                    variant: args.variant,
                    check_fields: false,
                }
            }
            (Some(schema), Some(None)) => {
                let mut reference = schema.clone();
                reference
                    .generics
                    .lt_token
                    .get_or_insert(Default::default());
                reference
                    .generics
                    .gt_token
                    .get_or_insert(Default::default());
                reference.generics.params.push(syn::parse_quote!('__ser));

                Config {
                    schema: Some(schema),
                    reference: Some(reference),
                    variant: args.variant,
                    check_fields: true,
                }
            }
            (None, Some(Some(reference))) => Config {
                schema: None,
                reference: Some(reference),
                variant: args.variant,
                check_fields: true,
            },
            (Some(schema), None) => Config {
                schema: Some(schema),
                reference: None,
                variant: args.variant,
                check_fields: true,
            },
            (Some(schema), Some(Some(reference))) => Config {
                schema: Some(schema),
                reference: Some(reference),
                variant: args.variant,
                check_fields: true,
            },
        }
    }
}

pub fn derive(input: proc_macro::TokenStream) -> syn::Result<TokenStream> {
    let input = syn::parse::<syn::DeriveInput>(input)?;
    let args = parse_attributes(&input.attrs)?;

    let ident = &input.ident;
    let generics = &input.generics;
    let (_impl_generics, type_generics, _where_clause) = generics.split_for_impl();

    match input.data {
        syn::Data::Union(_) => Err(syn::Error::new_spanned(
            input,
            "Serialize cannot be derived for unions",
        )),
        syn::Data::Struct(data) => {
            let cfg = Config::for_struct(args, &data, ident, generics);

            if cfg.variant.is_some() {
                unimplemented!("Add support for enums");
            }

            let field_count = data.fields.len();

            let field_check_names = match (cfg.check_fields, &data.fields) {
                (true, syn::Fields::Named(_)) => data
                    .fields
                    .iter()
                    .map(|field| {
                        quote::format_ident!(
                            "__alkahest_schema_field_{}_idx_is",
                            field.ident.as_ref().unwrap(),
                        )
                    })
                    .collect(),
                _ => Vec::new(),
            };

            let field_check_ids = match (cfg.check_fields, &data.fields) {
                (true, syn::Fields::Named(_)) => (0..data.fields.len()).collect(),
                _ => Vec::new(),
            };

            let field_names = data
                .fields
                .iter()
                .enumerate()
                .map(|(index, field)| match &field.ident {
                    Some(ident) => syn::Member::from(ident.clone()),
                    None => syn::Member::from(index),
                })
                .collect::<Vec<_>>();

            match (cfg.schema, cfg.reference) {
                (None, None) => unreachable!(),
                (None, Some(schema)) => {
                    let schema_type = &schema.ty;
                    let check_field_count = if cfg.check_fields {
                        quote::quote! {
                            let _: [(); #field_count] = <#schema_type>::__alkahest_schema_field_count();
                        }
                    } else {
                        quote::quote! {}
                    };

                    let mut generics = input.generics.clone();

                    generics.lt_token = generics.lt_token.or(schema.generics.lt_token);
                    generics.gt_token = generics.gt_token.or(schema.generics.gt_token);
                    generics.params.extend(schema.generics.params.into_iter());

                    if let Some(where_clause) = schema.generics.where_clause {
                        generics
                            .make_where_clause()
                            .predicates
                            .extend(where_clause.predicates);
                    }

                    let (impl_generics, _type_generics, where_clause) = generics.split_for_impl();

                    return Ok(quote::quote! {
                        impl #impl_generics ::alkahest::Serialize<#schema_type> for &'__ser #ident #type_generics #where_clause {
                            fn serialize(self, offset: ::alkahest::private::usize, output: &mut [::alkahest::private::u8]) -> ::alkahest::private::Result<(::alkahest::private::usize, ::alkahest::private::usize), ::alkahest::private::usize> {
                                use ::alkahest::private::Result;

                                // Checks compilation of code in the block.
                                #[allow(unused)]
                                let _ = || {
                                    #(let _: [(); #field_check_ids] = <#schema_type>::#field_check_names();)*
                                    #check_field_count
                                };

                                let mut ser = ::alkahest::Serializer::new(offset, output);


                                #[allow(unused_mut)]
                                let mut err = Result::<(), usize>::Ok(());

                                #(
                                    let with_schema = ::alkahest::private::with_schema(|s: &#schema_type| &s.#field_names);
                                    if let Result::Err(size) = err {
                                        err = Result::Err(size + with_schema.size_value(self.#field_names));
                                    } else {
                                        if let Result::Err(size) = with_schema.serialize_value(&mut ser, &self.#field_names) {
                                            err = Result::Err(size);
                                        }
                                    }
                                )*

                                err?;
                                Result::Ok(ser.finish())
                            }

                            fn size(self) -> ::alkahest::private::usize {
                                #[allow(unused_mut)]
                                let mut size = 0;
                                #(
                                    let with_schema = ::alkahest::private::with_schema(|s: &#schema_type| &s.#field_names);
                                    size += with_schema.size_value(&self.#field_names);
                                )*
                                size
                            }
                        }

                        impl #impl_generics ::alkahest::Serialize<#schema_type> for #ident #type_generics #where_clause {
                            #[inline(always)]
                            fn serialize(self, offset: ::alkahest::private::usize, output: &mut [::alkahest::private::u8]) -> ::alkahest::private::Result<(::alkahest::private::usize, ::alkahest::private::usize), ::alkahest::private::usize> {
                                <&Self as ::alkahest::Serialize<#schema_type>>::serialize(&self, offset, output)
                            }

                            #[inline(always)]
                            fn size(self) -> ::alkahest::private::usize {
                                <&Self as ::alkahest::Serialize<#schema_type>>::size(&self)
                            }
                        }
                    });
                }
                (Some(schema), reference) => {
                    let schema_type = &schema.ty;
                    let check_field_count = if cfg.check_fields {
                        quote::quote! {
                            let _: [(); #field_count] = <#schema_type>::__alkahest_schema_field_count();
                        }
                    } else {
                        quote::quote! {}
                    };

                    let mut generics = input.generics.clone();

                    generics.lt_token = generics.lt_token.or(schema.generics.lt_token);
                    generics.gt_token = generics.gt_token.or(schema.generics.gt_token);
                    generics.params.extend(schema.generics.params.into_iter());

                    if let Some(where_clause) = schema.generics.where_clause {
                        generics
                            .make_where_clause()
                            .predicates
                            .extend(where_clause.predicates);
                    }

                    let (impl_generics, _type_generics, where_clause) = generics.split_for_impl();

                    let mut tokens = quote::quote! {
                        impl #impl_generics ::alkahest::Serialize<#schema_type> for #ident #type_generics #where_clause {
                            fn serialize(self, offset: ::alkahest::private::usize, output: &mut [::alkahest::private::u8]) -> ::alkahest::private::Result<(::alkahest::private::usize, ::alkahest::private::usize), ::alkahest::private::usize> {
                                use ::alkahest::private::Result;

                                // Checks compilation of code in the block.
                                #[allow(unused)]
                                let _ = || {
                                    #(let _: [(); #field_check_ids] = <#schema_type>::#field_check_names();)*
                                    #check_field_count
                                };

                                let mut ser = ::alkahest::Serializer::new(offset, output);


                                #[allow(unused_mut)]
                                let mut err = Result::<(), usize>::Ok(());

                                #(
                                    let with_schema = ::alkahest::private::with_schema(|s: &#schema_type| &s.#field_names);
                                    if let Result::Err(size) = err {
                                        err = Result::Err(size + with_schema.size_value(self.#field_names));
                                    } else {
                                        if let Result::Err(size) = with_schema.serialize_value(&mut ser, self.#field_names) {
                                            err = Result::Err(size);
                                        }
                                    }
                                )*

                                err?;
                                Result::Ok(ser.finish())
                            }

                            fn size(self) -> ::alkahest::private::usize {
                                #[allow(unused_mut)]
                                let mut size = 0;
                                #(
                                    let with_schema = ::alkahest::private::with_schema(|s: &#schema_type| &s.#field_names);
                                    size += with_schema.size_value(self.#field_names);
                                )*
                                size
                            }
                        }
                    };

                    if let Some(reference) = reference {
                        let schema_type = &reference.ty;
                        generics = input.generics.clone();

                        generics.lt_token = generics.lt_token.or(schema.generics.lt_token);
                        generics.gt_token = generics.gt_token.or(schema.generics.gt_token);
                        generics
                            .params
                            .extend(reference.generics.params.into_iter());

                        if let Some(where_clause) = reference.generics.where_clause {
                            generics
                                .make_where_clause()
                                .predicates
                                .extend(where_clause.predicates);
                        }

                        let (impl_generics, _type_generics, where_clause) =
                            generics.split_for_impl();

                        tokens.extend(quote::quote! {
                            impl #impl_generics ::alkahest::Serialize<#schema_type> for &'__ser #ident #type_generics #where_clause {
                                fn serialize(self, offset: ::alkahest::private::usize, output: &mut [::alkahest::private::u8]) -> ::alkahest::private::Result<(::alkahest::private::usize, ::alkahest::private::usize), ::alkahest::private::usize> {
                                    use ::alkahest::private::Result;

                                    let mut ser = ::alkahest::Serializer::new(offset, output);

                                    #[allow(unused_mut)]
                                    let mut err = Result::<(), usize>::Ok(());

                                    #(
                                        let with_schema = ::alkahest::private::with_schema(|s: &#schema_type| &s.#field_names);
                                        if let Result::Err(size) = err {
                                            err = Result::Err(size + with_schema.size_value(&self.#field_names));
                                        } else {
                                            if let Result::Err(size) = with_schema.serialize_value(&mut ser, &self.#field_names) {
                                                err = Result::Err(size);
                                            }
                                        }
                                    )*

                                    err?;
                                    Result::Ok(ser.finish())
                                }

                                fn size(self) -> ::alkahest::private::usize {
                                    #[allow(unused_mut)]
                                    let mut size = 0;
                                    #(
                                        let with_schema = ::alkahest::private::with_schema(|s: &#schema_type| &s.#field_names);
                                        size += with_schema.size_value(&self.#field_names);
                                    )*
                                    size
                                }
                            }
                        });
                    }

                    Ok(tokens)
                }
            }
        }
        syn::Data::Enum(_) => {
            todo!()
        }
    }
}
