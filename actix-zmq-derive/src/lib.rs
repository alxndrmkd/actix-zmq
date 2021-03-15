#![feature(extend_one)]

use proc_macro::TokenStream;
use proc_macro2::{Ident, Span};
use quote::quote;
use syn::{
    Data, DataStruct, DeriveInput, GenericArgument, GenericParam, Generics, PathArguments, Type, TypeParam, TypePath,
};

#[proc_macro_derive(ActorContextStuff)]
pub fn derive_actor_context(input: TokenStream) -> TokenStream {
    let ast = syn::parse_macro_input!(input as DeriveInput);

    let name = &ast.ident;
    let (actor_type, parts) = get_parts_field_and_param(&ast);

    expand(name, &parts, &actor_type, &ast.generics)
}

fn expand(name: &syn::Ident, parts: &Ident, actor_type: &Type, generics: &Generics) -> TokenStream {
    let actor_context = expand_actor_context(name, parts, generics);
    let async_context = expand_async_context(name, parts, actor_type, generics);
    let context_parts = expand_async_context_parts(name, parts, actor_type, generics);
    let to_envelope = expand_to_envelope(name, actor_type, generics);

    let mut gen = TokenStream::default();
    gen.extend(vec![actor_context, async_context, context_parts, to_envelope]);

    gen
}

fn expand_actor_context(name: &syn::Ident, parts: &Ident, generics: &Generics) -> TokenStream {
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let gen = quote! {
        impl #impl_generics ::actix::ActorContext for #name #ty_generics #where_clause {
            fn stop(&mut self) { self.#parts.stop() }
            fn terminate(&mut self) { self.#parts.terminate() }
            fn state(&self) -> ::actix::ActorState { self.#parts.state() }
        }
    };

    gen.into()
}

fn expand_async_context(name: &syn::Ident, parts: &Ident, actor_type: &Type, generics: &Generics) -> TokenStream {
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let gen = quote! {
        impl #impl_generics ::actix::AsyncContext<#actor_type> for #name #ty_generics #where_clause {
            fn address(&self) -> ::actix::Addr<A> { self.#parts.address() }

            fn spawn<F>(&mut self, fut: F) -> ::actix::SpawnHandle
            where
                F: ::actix::fut::ActorFuture<A, Output = ()> + 'static,
            { self.#parts.spawn(fut) }

            fn wait<F>(&mut self, fut: F)
            where
                F: ::actix::fut::ActorFuture<A, Output = ()> + 'static,
            { self.#parts.wait(fut) }

            fn waiting(&self) -> bool { self.#parts.waiting() }

            fn cancel_future(&mut self, handle: ::actix::SpawnHandle) -> bool { self.#parts.cancel_future(handle) }
        }
    };

    gen.into()
}

fn expand_async_context_parts(name: &syn::Ident, parts: &Ident, actor_type: &Type, generics: &Generics) -> TokenStream {
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let gen = quote! {
        impl #impl_generics ::actix::dev::AsyncContextParts<#actor_type> for #name #ty_generics #where_clause {
            fn parts(&mut self) -> &mut ::actix::dev::ContextParts<A> { &mut self.#parts }
        }
    };

    gen.into()
}

fn expand_to_envelope(name: &syn::Ident, actor_type: &Type, generics: &Generics) -> TokenStream {
    let mut message_param = TypeParam::from(Ident::new("ACTIX_MESSAGE", Span::call_site()));
    message_param.bounds.push(syn::parse_quote!(::actix::Message));
    message_param.bounds.push(syn::parse_quote!(Send));
    message_param.bounds.push(syn::parse_quote!('static));

    let mut message_param_generics = generics.clone();
    message_param_generics.params.push(GenericParam::Type(message_param));

    add_handler_trait_bound(&mut message_param_generics);

    let wc = message_param_generics.make_where_clause();
    wc.predicates.push(syn::parse_quote!(ACTIX_MESSAGE::Result: Send));

    let (impl_generics, _, where_clause) = message_param_generics.split_for_impl();
    let (_, ty_generics, _) = generics.split_for_impl();

    let gen = quote! {
        impl #impl_generics ::actix::dev::ToEnvelope<#actor_type, ACTIX_MESSAGE> for #name #ty_generics #where_clause {
            fn pack(msg: ACTIX_MESSAGE, tx: ::std::option::Option<::actix::dev::OneshotSender<ACTIX_MESSAGE::Result>>) -> ::actix::dev::Envelope<#actor_type> {
                ::actix::dev::Envelope::new(msg, tx)
            }
        }
    };

    gen.into()
}

fn get_parts_field_and_param(input: &DeriveInput) -> (Type, Ident) {
    let fields = match input.data {
        Data::Struct(DataStruct { ref fields, .. }) => fields,
        _ => panic!("expected a struct"),
    };

    fields
        .iter()
        .enumerate()
        .find_map(|(ix, field)| match field.ty {
            Type::Path(ref tp) if type_is_context_parts(tp) => {
                let segment = tp.path.segments.last().unwrap();

                match &segment.arguments {
                    PathArguments::AngleBracketed(params) if params.args.len() == 1 => {
                        let param = match params.args.first().unwrap() {
                            GenericArgument::Type(tpe) => tpe.clone(),
                            _ => return None,
                        };

                        let field = match field.ident {
                            Some(ref i) => i.clone(),
                            _ => Ident::new(&ix.to_string(), Span::call_site()),
                        };

                        Some((param, field))
                    },

                    _ => None,
                }
            },
            _ => None,
        })
        .expect("expected a struct with field of type ::actix::ContextParts<A>")
}

fn type_is_context_parts(typepath: &TypePath) -> bool {
    typepath.qself.is_none()
        && typepath
            .path
            .segments
            .iter()
            .last()
            .map(|s| s.ident == "ContextParts" || s.ident == "Context")
            .unwrap_or(false)
}

fn add_handler_trait_bound(generics: &mut Generics) {
    for gen in &mut generics.params {
        if let syn::GenericParam::Type(ty) = gen {
            ty.bounds.push(syn::parse_quote!(::actix::Handler<ACTIX_MESSAGE>));
            break;
        }
    }
}
