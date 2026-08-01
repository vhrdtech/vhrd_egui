use quote::quote;

#[proc_macro]
pub fn svt(_item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    quote! {
        #[allow(unused)]
        let Self {
            seed: s,
            visual: v,
            transient,
        } = self;
        #[allow(unused)]
        let Some(t) = transient else {
            return;
        };
    }
    .into()
}
