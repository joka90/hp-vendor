use convert_case::{Case, Casing};
use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;
use serde_json::Value;
use std::{
    env,
    fs::{self, File},
    io::Write,
    path::PathBuf,
};

fn gen_diff(root: &Value, type_: &str) -> TokenStream {
    let properties = root
        .pointer(&format!("/definitions/{}/properties", type_))
        .unwrap();
    let properties_obj = properties.as_object().unwrap();

    let required: Vec<_> = root
        .pointer(&format!("/definitions/{}/required", type_))
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap())
        .collect();

    let mut props = Vec::new();
    for (k, _) in properties_obj.iter() {
        if k == "state" {
            continue;
        }

        let prop = if k == "type" {
            Ident::new("type_", Span::call_site())
        } else {
            Ident::new(k, Span::call_site())
        };

        props.push(if required.contains(&k.as_str()) {
            quote! {
                if old.#prop != new.#prop {
                    changed = true;
                }
            }
        } else {
            quote! {
                if old.#prop != new.#prop {
                    changed = true;
                } else {
                    new.#prop = None;
                }
            }
        });
    }

    quote! {
        {
            #[allow(unused_mut)]
            let mut changed = false;
            #(#props)*
            changed
        }
    }
}

fn main() {
    println!("cargo:rerun-if-changed=UploadEventPackageRequestModel.json");

    let json_str = fs::read_to_string("UploadEventPackageRequestModel.json").unwrap();
    let root: Value = serde_json::from_str(&json_str).unwrap();
    let mut variants = Vec::new();
    let mut structs = Vec::new();
    let mut states = Vec::new();
    let mut mut_states = Vec::new();
    let mut primaries = Vec::new();
    let mut diffs = Vec::new();
    for (k, v) in root
        .pointer("/definitions/AnyTelemetryEvent/properties")
        .unwrap()
        .as_object()
        .unwrap()
    {
        let variant = k.to_case(Case::UpperCamel);
        variants.push(Ident::new(&variant, Span::call_site()));

        let ref_ = v.pointer("/$ref").unwrap().as_str().unwrap();
        let type_ = ref_.strip_prefix("#/definitions/").unwrap();
        let struct_ = type_.replace("NVMES", "Nvmes"); // XXX Why? Better?
        structs.push(Ident::new(&struct_, Span::call_site()));

        let properties = root
            .pointer(&format!("/definitions/{}/properties", type_))
            .unwrap();
        let properties_obj = properties.as_object().unwrap();

        if let Some(ref_) = properties.pointer("/state/$ref") {
            let ref_ = ref_.as_str().unwrap();
            if ref_ == "#/definitions/SWState" {
                states.push(quote! { Some(State::Sw(x.state)) });
                mut_states.push(quote! { Some(MutState::Sw(&mut x.state)) });
            } else if ref_ == "#/definitions/HWState" {
                states.push(quote! { Some(State::Hw(x.state)) });
                mut_states.push(quote! { Some(MutState::Hw(&mut x.state)) });
            } else {
                unreachable!();
            }
        } else {
            states.push(quote! { None });
            mut_states.push(quote! { None });
        };

        let mut primary_keys: Vec<_> = properties_obj
            .iter()
            .filter_map(|(k, v)| {
                if let Some(desc) = v.pointer("/description") {
                    if desc.as_str().unwrap().contains("PRIMARY KEY") {
                        return Some(Ident::new(k, Span::call_site()));
                    }
                }
                None
            })
            .collect();
        primary_keys.sort();

        primaries.push(quote! {
            vec![#(x.#primary_keys.to_string()),*]
        });

        diffs.push(gen_diff(&root, &type_));
    }

    let tokens = quote! {
        // Generate a `TelemetryEvent` enum
        #[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
        #[serde(rename_all = "snake_case")]
        pub enum TelemetryEvent {
            #(#variants(#structs)),*
        }

        // Implement `From<T> for TelemetryEvent` for every type wrapper by enum
        #(
            impl From<#structs> for TelemetryEvent {
                fn from(value: #structs) -> Self {
                    TelemetryEvent::#variants(value)
                }
            }
        )*

        // Define `TelemetryEventType` enum
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum TelemetryEventType {
            #(#variants),*
        }

        // Generate `TelemetryEventType::iter()` method to iterate over variants
        impl TelemetryEventType {
            pub fn iter() -> impl Iterator<Item=Self> {
                static VARIANTS: &[TelemetryEventType] = &[
                    #(TelemetryEventType::#variants),*
                ];
                VARIANTS.iter().copied()
            }
        }

        // Generate function from `TelemetryEvent` to `TelemetryEvent`
        impl TelemetryEvent {
            fn type_(&self) -> TelemetryEventType {
                match self {
                    #(TelemetryEvent::#variants(_) => TelemetryEventType::#variants),*
                }
            }

            #[allow(unused_variables)]
            fn state_mut(&mut self) -> Option<MutState> {
                match self {
                    #(TelemetryEvent::#variants(x) => #mut_states),*
                }
            }

            #[allow(unused_variables)]
            fn primaries(&self) -> Vec<String> {
                match self {
                    #(TelemetryEvent::#variants(x) => #primaries),*
                }
            }

            // Panics if objects are of different variants
            #[allow(unused_variables)]
            fn diff(&mut self, old: &Self) -> bool {
                match (self, old) {
                    #((TelemetryEvent::#variants(new), TelemetryEvent::#variants(old)) => #diffs),*
                    _ => { unreachable!() }
                }
            }
        }
    };

    let out_dir = env::var_os("OUT_DIR").unwrap();
    let mut path = PathBuf::from(out_dir);
    path.push("event_enum.rs");
    let mut file = File::create(path).unwrap();
    writeln!(file, "{}", tokens).unwrap();
}
