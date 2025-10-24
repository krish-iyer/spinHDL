use super::*;

use std::{fs::File};
use std::{io, io::Write};

impl BuildCfg {

    pub fn create_project_tcl(&self, design: &DesignCfg) -> io::Result<()> {
        let tcl_path = "create_project.tcl";
        let mut tcl_file = File::create(&tcl_path)?;

        writeln!(
            tcl_file,
            "create_project -force -part {} {}",
            self.projectcfg.part, design.name
        )?;

        if !design.rtl_files.is_empty() {
            writeln!(
                tcl_file,
                "add_files -fileset sources_1 {}",
                design.rtl_files.join(" ")
            )?;
        }

        writeln!(
            tcl_file,
            "set_property top {} [current_fileset]",
            design.top
        )?;

        if !design.xdc_files.is_empty() {
            writeln!(
                tcl_file,
                "add_files -fileset constrs_1 {}",
                design.xdc_files.join(" ")
            )?;
        }

        for file in &design.xci_files {
            writeln!(tcl_file, "import_ip {}", file)?;
        }

        for file in &design.ip_files {
            writeln!(tcl_file, "source {}", file)?;
        }

        println!("Created create_project.tcl for '{}'", design.name);
        Ok(())
    }

    pub fn create_synth_tcl(&self, design: &DesignCfg) -> io::Result<()> {
        let synth_tcl_path = "run_synth.tcl";
        let mut synth_tcl = File::create(&synth_tcl_path).expect("Failed to create run_synth.tcl");

        writeln!(synth_tcl, "open_project {}.xpr", design.name)?;

        match design.moduletype {
            ModuleType::Recon => {
                writeln!(synth_tcl, "synth_design -mode out_of_context")?;
                writeln!(
                    synth_tcl,
                    "write_checkpoint -force {}/runs/synth_1/{}.dcp",
                    design.name, design.name
                )?;
                writeln!(synth_tcl, "close_project")?;
            }

            ModuleType::Static => {
                writeln!(synth_tcl, "reset_run synth_1")?;
                writeln!(synth_tcl, "launch_runs -jobs 4 synth_1")?;
                writeln!(synth_tcl, "wait_on_run synth_1")?;
            }
        }

        Ok(())
    }

    pub fn create_pr_xdc_tcl(&self, constr: &PrXdc) -> io::Result<()> {
        // Ensure root design exists

        let tcl_path = "create_pr_xdc.tcl";

        let mut tcl = File::create(tcl_path)?;

        writeln!(tcl, "open_project {}.xpr", constr.project_name)?;
        writeln!(tcl, "open_run synth_1 -name synth_1")?;
        writeln!(
            tcl,
            "set_property target_constrs_file pr_{}.xdc [current_fileset -constrset]",
            constr.project_name
        )?;

        writeln!(tcl, "startgroup")?;
        writeln!(tcl, "create_pblock pblock_{}", constr.instance_name)?;
        writeln!(
            tcl,
            "resize_pblock pblock_{} -add {}",
            constr.instance_name, constr.region
        )?;
        writeln!(
            tcl,
            "add_cells_to_pblock pblock_{} [get_cells [list {}]] -clear_locs",
            constr.instance_name, constr.instance_name
        )?;
        writeln!(tcl, "endgroup")?;

        writeln!(
            tcl,
            "set_property SNAPPING_MODE ON [get_pblocks pblock_{}]",
            constr.instance_name
        )?;
        writeln!(
            tcl,
            "set_property RESET_AFTER_RECONFIG 1 [get_pblocks pblock_{}]",
            constr.instance_name
        )?;
        writeln!(
            tcl,
            "set_property HD.RECONFIGURABLE 1 [get_cells {}]",
            constr.instance_name
        )?;
        writeln!(tcl, "save_constraints -force")?;
        writeln!(tcl, "close_project")?;

        println!(
            "Generated partial reconfiguration XDC for '{}'",
            constr.project_name
        );
        Ok(())
    }

    pub fn create_route_tcl(&self, root_design: &String) -> io::Result<()> {
        let pr_node = self.design_graph.get_child_nodes(&root_design, true);

        // TODO: Fix this; you are being sloppy
        let pr_inst = match &pr_node[0] {
            design_hier::NodeKind::Module { name, region } => PrXdc {
                project_name: root_design.clone(),
                instance_name: name.clone(),
                region: region.clone().unwrap_or_default(),
            },
            _ => panic!("Expected Module but received Design node"),
        };

        let rm_designs = self
            .design_graph
            .get_child_nodes(&pr_inst.instance_name, false);

        let tcl_path = "route_pr.tcl";

        let mut tcl = File::create(tcl_path)?;

        writeln!(tcl, "open_project {}.xpr", root_design)?;
        writeln!(tcl, "open_run synth_1 -name synth_1")?;

        for (i, rm) in rm_designs.iter().enumerate() {
            match rm {
                design_hier::NodeKind::Design { name } => {
                    writeln!(
                        tcl,
                        "read_checkpoint -cell [get_cells {}] ../{}.dcp",
                        pr_inst.instance_name, name
                    )?;

                    writeln!(tcl, "opt_design")?;
                    writeln!(tcl, "place_design")?;
                    writeln!(tcl, "route_design")?;

                    writeln!(tcl, "write_checkpoint -force {}_routed.dcp", name)?;

                    writeln!(
                        tcl,
                        "update_design -cell [get_cells {}] -black_box",
                        pr_inst.instance_name
                    )?;

                    if i == 0 {
                        // only lock in the first iter
                        writeln!(tcl, "lock -level routing")?;
                    }
                }
                _ => panic!("Received a Module when Design was expected!"),
            }
        }
        writeln!(tcl, "close_project")?;

        Ok(())
    }

    pub fn create_bitstream_tcl(&self, root_design: &String) -> io::Result<()> {
        let pr_node = self.design_graph.get_child_nodes(&root_design, true);

        // TODO: Fix this; you are being sloppy
        let pr_inst = match &pr_node[0] {
            design_hier::NodeKind::Module { name, region } => PrXdc {
                project_name: root_design.clone(),
                instance_name: name.clone(),
                region: region.clone().unwrap_or_default(),
            },
            _ => panic!("Expected Module but received Design node"),
        };

        let rm_designs = self
            .design_graph
            .get_child_nodes(&pr_inst.instance_name, false);

        for rm in &rm_designs {
            match rm {
                design_hier::NodeKind::Design { name } => {
                    let tcl_path = format!("generate_bit_{}.tcl", name);

                    let mut tcl = File::create(&tcl_path)?;

                    // Write TCL commands
                    writeln!(tcl, "open_project {}.xpr", root_design)?;
                    writeln!(tcl, "open_checkpoint {}_routed.dcp", name)?;
                    writeln!(tcl, "write_bitstream -force -bin_file {}.bit", name)?;
                    writeln!(tcl, "write_debug_probes -force {}.ltx", name)?;
                    writeln!(tcl, "write_hw_platform -fixed -force {}.xsa", name)?;
                    writeln!(
                        tcl,
                        "write_cfgmem -force -format BIN -interface SMAPx32 \
                         -loadbit \"up 0x0 {}_pblock_{}_partial.bit\" \"{}_part.bin\"",
                        name, pr_inst.instance_name, name
                    )?;
                    writeln!(tcl, "close_design")?;
                    writeln!(tcl, "close_project")?;
                }
                _ => panic!("Failed to create bitstreams tcl"),
            }
        }

        Ok(())
    }
}
