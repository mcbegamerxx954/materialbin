use std::{
    fs::{self, File},
    io::BufWriter,
    path::Path,
};

use mt_updater::{CompiledMaterialDefinition, MinecraftVersion};
use scroll::Pread;
fn main() -> anyhow::Result<()> {
    let file = fs::read("RenderChunk.material.bin")?;
    let material: CompiledMaterialDefinition = file.pread_with(0, MinecraftVersion::V1_19_60)?;
    //    println!("{material:#?}");
    //    println!("Material parsing result: {material:#?}");
    let mut output = File::create("RenderChunk.material.bin.rustbased")?;
    let mut output = BufWriter::new(output);
    for file in fs::read_dir("input")? {
        let file = file?;
        let data = fs::read(file.path())?;
        let material: CompiledMaterialDefinition =
            data.pread_with(0, MinecraftVersion::V1_18_30)?;
        let out_path = Path::new("output/").join(file.path().file_name().unwrap());
        let mut output = File::create(out_path)?;
        let mut output = BufWriter::new(output);
        material.write(&mut output)?;
    }
    material.write(&mut output)?;
    Ok(())
}
