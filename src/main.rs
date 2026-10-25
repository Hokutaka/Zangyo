use std::{env, error::Error, fs, path::PathBuf};
use sashimi_assembler::Target;

fn run() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let Some(source) = args.next() else {
        return Err("usage: zangyo <file.zgy> --target <triple> -o <file.o> [--asm file.s] [--origins file.txt]".into());
    };
    if source == "--help" || source == "-h" {
        println!("usage: zangyo <file.zgy> --target <triple> -o <file.o> [--asm file.s] [--origins file.txt]");
        return Ok(());
    }
    let mut target = None;
    let mut out = None;
    let mut asm = None;
    let mut origins = None;
    while let Some(flag) = args.next() {
        let value = args.next().ok_or(format!("{flag} の値が必要です"))?;
        match flag.as_str() {
            "--target" => target = Some(Target::parse(&value).ok_or("unsupported target; use x86_64-unknown-linux-gnu or x86_64-pc-windows-msvc")?),
            "-o" => out = Some(PathBuf::from(value)),
            "--asm" => asm = Some(PathBuf::from(value)),
            "--origins" => origins = Some(PathBuf::from(value)),
            _ => return Err(format!("unknown option: {flag}").into()),
        }
    }
    let target = target.ok_or("--target is required")?;
    let out = out.ok_or("-o is required")?;
    let text = fs::read_to_string(&source)?;
    let generated = zangyo::compile(&text, target)?;
    for warning in generated.warnings { eprintln!("{warning}"); }
    fn write(path: PathBuf, content: impl AsRef<[u8]>) -> Result<(), Box<dyn Error>> {
        if let Some(parent) = path.parent().filter(|x| !x.as_os_str().is_empty()) { fs::create_dir_all(parent)?; }
        fs::write(path, content)?;
        Ok(())
    }
    write(out, &generated.object)?;
    if let Some(path) = asm { write(path, generated.asm.as_bytes())?; }
    if let Some(path) = origins { write(path, generated.origins.as_bytes())?; }
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
