use clap::{Parser, Subcommand};
use spinhdl_core::BuildCfg;
use std::{env, fs, path::PathBuf};

#[derive(Parser)]
#[command(name = "spinhdl", about = "HDL project build and generation tool")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    New {
        name: String,
    },

    Weave {
        #[arg(default_value = "spinhdl.toml")]
        config: PathBuf,
    },

    Spin {
        #[arg(default_value = "spinhdl.toml")]
        config: PathBuf,
    },

    Clean {
        #[arg(default_value = "spinhdl.toml")]
        config: PathBuf,
    },
}

fn main() {

   let cli = Cli::parse();

    match cli.command {
        Commands::Weave { config } => {
            let mut cfg = load_config(&config);
            println!("Project name: {}", cfg.projectcfg.name);
            println!("Project version: {}", cfg.projectcfg.version);
            cfg.verify_build_setup();
            cfg.build_designs();
        }
        _ => {
            panic!("Command currently not supported");
        }
    }
}

fn load_config(path: &PathBuf) -> BuildCfg {
    let data = fs::read_to_string(path)
        .unwrap_or_else(|_| panic!("failed to read file: {}", path.display()));
    toml::from_str(&data).expect("failed to parse toml")
}
