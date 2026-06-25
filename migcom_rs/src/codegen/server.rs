// SPDX-License-Identifier: MIT
// Server-side stub generation (mirrors server.c)

use std::io::{self, Write};

use super::utils::{write_identification, write_imports, write_mig_external};
use crate::ast::{Direction, ImportKind, RoutineKind, Statement};
use crate::global::Options;

pub fn write_server(w: &mut dyn Write, stmts: &[Statement], opts: &Options) -> io::Result<()> {
    let subsys = opts.subsystem_name.as_deref().unwrap_or("?");
    let demux = opts.server_demux.as_deref().unwrap_or(subsys);

    write_identification(w, opts)?;
    writeln!(w)?;
    writeln!(w, "/* Module {subsys} */")?;
    writeln!(w)?;
    writeln!(w, "#define\t__MIG_check__server_{subsys}_subsystem__")?;
    writeln!(w)?;
    writeln!(w, "#include <mach/mach_types.h>")?;
    writeln!(w, "#include <mach/message.h>")?;
    writeln!(w, "#include <mach/ndr.h>")?;
    writeln!(w, "#include <mach/mig.h>")?;
    writeln!(w, "#include <mach/mig_errors.h>")?;
    writeln!(w)?;
    writeln!(w, "/* Includes from Import / SImport directives */")?;
    write_imports(w, stmts, &[ImportKind::Import, ImportKind::SImport])?;
    writeln!(w)?;

    // Count routines
    let routines: Vec<&crate::ast::Routine> = stmts
        .iter()
        .filter_map(|s| {
            if let Statement::Routine(r) = s {
                Some(r)
            } else {
                None
            }
        })
        .collect();

    let base = opts.subsystem_base;

    // Write per-routine dispatch functions
    for (seq, rt) in routines.iter().enumerate() {
        write_server_routine(w, rt, base + seq as u32, opts)?;
    }

    // Write the demux function
    write_demux(w, &routines, base, demux, opts)?;

    // Write the MIG subsystem descriptor
    write_subsystem(w, &routines, base, subsys, opts)?;

    Ok(())
}

fn write_server_routine(
    w: &mut dyn Write,
    rt: &crate::ast::Routine,
    _msg_id: u32,
    opts: &Options,
) -> io::Result<()> {
    let is_simple = rt.kind == RoutineKind::SimpleRoutine;
    let srv_fn = format!("{}{}", opts.server_prefix, rt.name);
    let dispatch_fn = format!("_X{}", rt.name);

    writeln!(w)?;
    writeln!(w, "/* {dispatch_fn} — dispatch wrapper for {srv_fn} */")?;

    write_mig_external(w)?;
    writeln!(w, "kern_return_t {dispatch_fn}(")?;
    writeln!(w, "\tmach_msg_header_t *InHeadP,")?;
    writeln!(w, "\tmach_msg_header_t *OutHeadP)")?;
    writeln!(w, "{{")?;

    // Request struct
    writeln!(w, "\ttypedef struct {{")?;
    writeln!(w, "\t\tmach_msg_header_t Head;")?;
    writeln!(w, "\t\tNDR_record_t NDR;")?;
    for arg in rt.args.iter().filter(|a| {
        matches!(
            a.direction,
            Direction::In | Direction::InOut | Direction::None
        )
    }) {
        let ty = arg.ty.server_type.as_deref().unwrap_or("int");
        writeln!(w, "\t\t{ty} {};\t/* in */", arg.name)?;
    }
    writeln!(w, "\t}} Request;")?;
    writeln!(w)?;

    if !is_simple {
        // Reply struct
        writeln!(w, "\ttypedef struct {{")?;
        writeln!(w, "\t\tmach_msg_header_t Head;")?;
        writeln!(w, "\t\tNDR_record_t NDR;")?;
        writeln!(w, "\t\tmach_msg_type_name_t RetCodeType;")?;
        writeln!(w, "\t\tkern_return_t RetCode;")?;
        for arg in rt
            .args
            .iter()
            .filter(|a| matches!(a.direction, Direction::Out | Direction::InOut))
        {
            let ty = arg.ty.server_type.as_deref().unwrap_or("int");
            writeln!(w, "\t\t{ty} {};\t/* out */", arg.name)?;
        }
        writeln!(w, "\t}} Reply;")?;
        writeln!(w)?;
    }

    writeln!(w, "\tRequest *In0P = (Request *)InHeadP;")?;
    if !is_simple {
        writeln!(w, "\tReply *OutP = (Reply *)OutHeadP;")?;
    }
    writeln!(w)?;

    // Call the server function
    let in_args: Vec<String> = rt
        .args
        .iter()
        .filter(|a| matches!(a.direction, Direction::In | Direction::None))
        .map(|a| format!("In0P->{}", a.name))
        .collect();
    let out_args: Vec<String> = rt
        .args
        .iter()
        .filter(|a| matches!(a.direction, Direction::Out | Direction::InOut))
        .map(|a| format!("&OutP->{}", a.name))
        .collect();

    let all_args: Vec<String> = in_args.into_iter().chain(out_args).collect();

    if is_simple {
        writeln!(w, "\t(void){srv_fn}({});", all_args.join(", "))?;
    } else {
        writeln!(w, "\tOutP->RetCode = {srv_fn}({});", all_args.join(", "))?;
    }

    if !is_simple {
        writeln!(w)?;
        writeln!(
            w,
            "\tOutP->Head.msgh_size = (mach_msg_size_t)sizeof(Reply);"
        )?;
    }

    writeln!(w, "\treturn MACH_MSG_SUCCESS;")?;
    writeln!(w, "}}")
}

fn write_demux(
    w: &mut dyn Write,
    routines: &[&crate::ast::Routine],
    base: u32,
    demux: &str,
    _opts: &Options,
) -> io::Result<()> {
    writeln!(w)?;
    writeln!(w, "/* Server demux */")?;
    write_mig_external(w)?;
    writeln!(w, "boolean_t {demux}(")?;
    writeln!(w, "\tmach_msg_header_t *InHeadP,")?;
    writeln!(w, "\tmach_msg_header_t *OutHeadP)")?;
    writeln!(w, "{{")?;
    writeln!(w, "\tmach_msg_id_t msgh_id = InHeadP->msgh_id;")?;
    writeln!(
        w,
        "\tif (msgh_id < {base} || msgh_id > {})",
        base + routines.len() as u32 - 1
    )?;
    writeln!(w, "\t\treturn FALSE;")?;
    writeln!(w)?;
    writeln!(w, "\ttypedef kern_return_t (*dispatch_fn_t)(")?;
    writeln!(w, "\t\tmach_msg_header_t *, mach_msg_header_t *);")?;
    writeln!(w)?;
    writeln!(w, "\tstatic const dispatch_fn_t dispatch_table[] = {{")?;
    for rt in routines.iter() {
        writeln!(w, "\t\t(dispatch_fn_t)_X{},", rt.name)?;
    }
    writeln!(w, "\t}};")?;
    writeln!(w)?;
    writeln!(
        w,
        "\treturn dispatch_table[msgh_id - {base}](InHeadP, OutHeadP) == MACH_MSG_SUCCESS;"
    )?;
    writeln!(w, "}}")
}

fn write_subsystem(
    w: &mut dyn Write,
    routines: &[&crate::ast::Routine],
    base: u32,
    subsys: &str,
    _opts: &Options,
) -> io::Result<()> {
    let subsys_sym = format!("{}_subsystem", subsys);
    writeln!(w)?;
    writeln!(w, "/* MIG subsystem descriptor */")?;
    writeln!(w, "const struct mig_subsystem {subsys_sym} = {{")?;
    writeln!(w, "\t{},\t/* start */", base)?;
    writeln!(w, "\t{},\t/* end */", base + routines.len() as u32)?;
    writeln!(w, "\tsizeof(mig_reply_error_t),")?;
    writeln!(w, "\t{{")?;
    for rt in routines.iter() {
        writeln!(
            w,
            "\t\t{{ (mig_impl_routine_t)0, (mig_stub_routine_t)_X{name}, 0, 0, _WALIGN(sizeof(mig_reply_error_t)) }},",
            name = rt.name
        )?;
    }
    writeln!(w, "\t}}")?;
    writeln!(w, "}};")
}
