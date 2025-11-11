use proc_macro::TokenStream;

#[proc_macro]
pub fn mangled_symbol(ident:TokenStream) -> TokenStream {
    use proc_macro::{TokenTree,Literal};
    let mut base = String::new();
    for ident in ident {
        match ident {
            TokenTree::Ident(id) => {
                base = id.to_string();
                break;
            },
            _ => {
                panic!("expecting a single identifier")
            }
        }
    }
    base.push_str("_mangled_");
    for _ in 0..64 {
        base.push(fastrand::alphanumeric());
    }
    TokenTree::Literal(Literal::string(&base)).into()
}