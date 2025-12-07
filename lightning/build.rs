use cfg_aliases::cfg_aliases;

fn main() {
    cfg_aliases! {
        web: { all(target_arch = "wasm32", feature = "web") },
        desktop: { all(not(target_arch = "wasm32"), feature = "desktop") },
    }
}