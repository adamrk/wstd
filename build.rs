use cfg_aliases::cfg_aliases;

fn main() {
    cfg_aliases! {
        // TODO https://github.com/bytecodealliance/wstd/issues/147: Swap these
        // to use `target_env` instead.
        p2: { feature = "p2" },
        p3: { feature = "p3" },
    }
}
