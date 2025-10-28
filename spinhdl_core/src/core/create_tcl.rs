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

        let tcl_path = "run_route.tcl";

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
                    let tcl_path = format!("run_bitgen_{}.tcl", name);

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

    pub fn create_zynq_driver_tcl(&self, dir: &String) -> io::Result<()> {
        let tcl_path = "zynq_driver.tcl";
        let mut tcl = File::create(tcl_path)?;

        // Header and source imports
        writeln!(tcl, "source ~/.tools/Xilinx/Vitis/2022.2/scripts/vitis/util/zynqmp_utils.tcl")?;
        writeln!(tcl)?;
        writeln!(tcl, "# Auto-generated TCL for ZynqMP Vitis driver flow")?;
        writeln!(tcl)?;

        // Build procedure
        writeln!(tcl, "proc build {{dir}} {{")?;
        writeln!(tcl, "    set xsa [file join $dir logic_1.xsa]")?;
        writeln!(tcl, "    puts \"dir: $dir\"")?;
        writeln!(tcl, "    setws driver_build/")?;
        writeln!(tcl, "    puts \"xsa: $xsa\"")?;
        writeln!(tcl, "    app create -name baremetal_driver -hw $xsa -proc psu_cortexa53_0 -os standalone -lang C -template {{Empty Application(C)}}")?;
        writeln!(tcl, "    importsources -name baremetal_driver -path drivers/baremetal/src/")?;
        writeln!(tcl, "    app build -name baremetal_driver hw_server")?;
        writeln!(tcl, "}}")?;
        writeln!(tcl)?;

        // boot_jtag procedure
        writeln!(tcl, "proc boot_jtag {{}} {{")?;
        writeln!(tcl, "    targets -set -filter {{name =~ \"PSU\"}}")?;
        writeln!(tcl, "    mwr 0xffca0010 0x0    ;# multiboot = 0")?;
        writeln!(tcl, "    mwr 0xff5e0200 0x0100 ;# boot mode = JTAG")?;
        writeln!(tcl, "    rst -system")?;
        writeln!(tcl, "}}")?;
        writeln!(tcl)?;

        // flash procedure
        writeln!(tcl, "proc flash {{dir}} {{")?;
        writeln!(tcl, "    connect")?;
        writeln!(tcl, "    boot_jtag")?;
        writeln!(tcl)?;
        writeln!(tcl, "    source driver_build/logic_1/hw/psu_init.tcl")?;
        writeln!(tcl)?;
        writeln!(tcl, "    targets -set -nocase -filter {{name =~\"APU*\"}}")?;
        writeln!(tcl, "    rst -system")?;
        writeln!(tcl, "    after 3000")?;
        writeln!(tcl, "    targets -set -nocase -filter {{name =~\"APU*\"}}")?;
        writeln!(tcl, "    reset_apu")?;
        writeln!(tcl)?;
        writeln!(tcl, "    set bitfile [file join $dir logic_1.bit]")?;
        writeln!(tcl, "    puts \"Programming bitstream: $bitfile\"")?;
        writeln!(tcl, "    fpga -file $bitfile")?;
        writeln!(tcl)?;
        writeln!(tcl, "    targets -set -nocase -filter {{name =~\"APU*\"}}")?;
        writeln!(tcl, "    loadhw -hw driver_build/logic_1/hw/logic_1.xsa -mem-ranges [list {{0x80000000 0xbfffffff}} {{0x400000000 0x5ffffffff}} {{0x1000000000 0x7fffffffff}}] -regs")?;
        writeln!(tcl, "    configparams force-mem-access 1")?;
        writeln!(tcl)?;
        writeln!(tcl, "    set mode [expr {{[mrd -value 0xFF5E0200] & 0xf}}]")?;
        writeln!(tcl)?;
        writeln!(tcl, "    targets -set -nocase -filter {{name =~ \"*A53*#0\"}}")?;
        writeln!(tcl, "    rst -processor")?;
        writeln!(tcl, "    dow driver_build/logic_1/export/logic_1/sw/logic_1/boot/fsbl.elf")?;
        writeln!(tcl, "    set bp_16_2_fsbl_bp [bpadd -addr &XFsbl_Exit]")?;
        writeln!(tcl, "    con -block -timeout 60")?;
        writeln!(tcl, "    bpremove $bp_16_2_fsbl_bp")?;
        writeln!(tcl)?;
        writeln!(tcl, "    set part0 [file join $dir logic_1_part.bin]")?;
        writeln!(tcl, "    set part1 [file join $dir logic_2_part.bin]")?;
        writeln!(tcl, "    set size [file size $part0]")?;
        writeln!(tcl, "    mwr -bin -file $part0 0x0800000000 $size")?;
        writeln!(tcl, "    mwr -bin -file $part1 0x08000C0000 $size")?;
        writeln!(tcl)?;
        writeln!(tcl, "    rst -processor")?;
        writeln!(tcl, "    dow driver_build/baremetal_driver/Debug/baremetal_driver.elf")?;
        writeln!(tcl, "    configparams force-mem-access 0")?;
        writeln!(tcl, "    bpadd -addr &main")?;
        writeln!(tcl, "    con -block -timeout 500")?;
        writeln!(tcl, "    con")?;
        writeln!(tcl, "}}")?;
        writeln!(tcl)?;

        // CLI-like command handling
        writeln!(tcl, "if {{[llength $argv] == 0}} {{")?;
        writeln!(tcl, "    puts \"Usage: xsct zynq_driver.tcl <build|flash|all>\"")?;
        writeln!(tcl, "    exit")?;
        writeln!(tcl, "}}")?;
        writeln!(tcl)?;
        writeln!(tcl, "set dir \"{}\"", dir)?;
        writeln!(tcl)?;
        writeln!(tcl, "set cmd $argv")?;
        writeln!(tcl, "switch -- $cmd {{")?;
        writeln!(tcl, "    build {{ build $dir }}")?;
        writeln!(tcl, "    flash {{ flash $dir }}")?;
        writeln!(tcl, "    all   {{ build $dir; flash $dir }}")?;
        writeln!(tcl, "    default {{ puts \"Unknown argument: $cmd\"; exit 1 }}")?;
        writeln!(tcl, "}}")?;

        println!("Generated zynq_driver.tcl successfully");
        Ok(())
    }

}
