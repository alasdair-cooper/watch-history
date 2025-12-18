use crux_core::typegen::TypeGen;
use shared::{App, Rating};
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    println!("cargo:rerun-if-changed=../shared");

    let mut gen = TypeGen::new();

    gen.register_app::<App>()?;
    
    gen.register_type::<Rating>()?;
    
    let output_root = PathBuf::from("./generated");

    gen.java("com.alasdair_cooper.watch_history.types", output_root.join("java"))?;

    Ok(())
}
