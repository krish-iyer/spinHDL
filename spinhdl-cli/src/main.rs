use std::{env, fs, path::PathBuf};
use spinhdl::BuildCfg;

fn main() {
    let cli_args: Vec<String> = env::args().collect();
    if cli_args.len() != 2 {
        println!("Usage: {} <config-file>", cli_args[0]);
        panic!("not enough args!");
    }

    let path = PathBuf::from(&cli_args[1]);
    let data = fs::read_to_string(&path).expect("failed to read file");
    let mut cfg: BuildCfg = toml::from_str(&data).expect("failed to parse toml");

    println!("Project name: {}", cfg.projectcfg.name);
    println!("Project version: {}", cfg.projectcfg.version);
    println!("Design build: {:?}", cfg.designcfg[0].build);

    println!("generating project");
    cfg.verify_build_setup();

    // if let Some(hier) = &cfg.hier {
    //     println!("Verilog hierarchy:");
    //     for (path, node) in &hier.modules {

    //         if let Some(rm) = &node.rm {
    //             println!("{} -> {:?}", path, rm);
    //         }
    //     }
    // }

    cfg.build_designs();
}
