use clap::{Parser, Subcommand};
use spinhdl_core::{BuildCfg, BuildStage};
use std::{fs, path::PathBuf};

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

    Emit {
        #[arg(default_value = "spinhdl.toml")]
        config: PathBuf,
        #[arg(long, default_value = "build/main")]
        dir: String,
    },
    Dryrun {
        #[arg(default_value = "spinhdl.toml")]
        config: PathBuf,
    },

    Clean {
        #[arg(default_value = "spinhdl.toml")]
        config: PathBuf,
    },

    Revert {
        design: String,
        stage: String,
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
        Commands::Emit { config, dir } => {
            let mut cfg = load_config(&config);
            println!("Project name: {}", cfg.projectcfg.name);
            println!("Project version: {}", cfg.projectcfg.version);
            if let Err(e) = cfg.create_zynq_driver_tcl(&dir) {
                panic!("Error creating zynq driver TCL {}", e);
            }
        }

        Commands::Dryrun { config } => {
            let mut cfg = load_config(&config);
            // let fg = FlowGraph::from_toml_file("build/flow.lock.toml")?;
            // fg.print_hierarchy();
            // cfg.create_build_tasks();
            cfg.build_flow_graph();
            cfg.flow_graph.print_hierarchy();
            println!(
                "Topo order:\n  {}",
                cfg.flow_graph.topo_order().join("\n  ")
            );
            //println!("{:#?}", cfg.tasks);
            let dot = cfg.flow_graph.to_dot();
            std::fs::write("flow.dot", dot);
        }

        Commands::Revert {
            config,
            design,
            stage,
        } => {
            let mut cfg = load_config(&config);
            cfg.build_flow_graph();

            let Some(stage_enum) = BuildStage::from_str(&stage) else {
                    panic!(
                        "Unknown stage '{}'. Expected one of: verify_files, create_project, synth, route, bitgen.",
                        stage
                    );
            };

            cfg.revert_stage(&design, stage_enum);
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
