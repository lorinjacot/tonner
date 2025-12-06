pub use vec3::*;
pub use vec4::*;

mod vec3;
mod vec4;

#[doc(hidden)]
#[macro_export]
macro_rules! wrapper {
    (
        $doc:literal,
        $wrapper:ident,
        $inner:ty,
        fields: [$($field:ident: $field_type:ty),*],
        consts: [$(
            $const_doc:literal
            $const:ident
        ),*]
    ) => {
        #[doc = $doc]
        #[wasm_bindgen]
        #[derive(Clone, Copy)]
        pub struct $wrapper(pub(crate) $inner);

        paste::paste! {
            #[wasm_bindgen]
            impl $wrapper {
                $(
                    #[wasm_bindgen(getter)]
                    pub fn $field(&self) -> $field_type {
                        self.0.$field
                    }

                    #[wasm_bindgen(setter)]
                    pub fn [<set_ $field>](&mut self, $field: $field_type) {
                        self.0.$field = $field
                    }
                )*

                $(
                    #[doc = $const_doc]
                    #[allow(non_snake_case)]
                    pub fn $const() -> Self {
                        Self($inner::$const)
                    }
                )*
            }
        }
    };
}
