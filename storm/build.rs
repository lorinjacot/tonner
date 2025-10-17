use cfg_aliases::cfg_aliases;

fn main() {
    cfg_aliases! {
        web: { all(target_arch = "wasm32", not(target_os = "emscripten"), feature = "web") },
    }
}
