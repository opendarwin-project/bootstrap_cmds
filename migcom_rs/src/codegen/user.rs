// SPDX-License-Identifier: MIT
// User-side stub generation (mirrors user.c)

use std::io::{self, Write};

use super::utils::{write_identification, write_imports, write_mig_external};
use crate::ast::{Direction, ImportKind, RoutineKind, Statement};
use crate::global::Options;

pub fn write_user(w: &mut dyn Write, stmts: &[Statement], opts: &Options) -> io::Result<()> {
    write_identification(w, opts)?;
    writeln!(w)?;
    writeln!(
        w,
        "/* Module {} */",
        opts.subsystem_name.as_deref().unwrap_or("?")
    )?;
    writeln!(w)?;
    writeln!(
        w,
        "#define\t__MIG_check__user_{}_subsystem__",
        opts.subsystem_name.as_deref().unwrap_or("?")
    )?;
    writeln!(w)?;
    writeln!(w, "#include <mach/mach_types.h>")?;
    writeln!(w, "#include <mach/message.h>")?;
    writeln!(w, "#include <mach/ndr.h>")?;
    writeln!(w, "#include <mach/mig.h>")?;
    writeln!(w, "#include <mach/mig_errors.h>")?;
    writeln!(w)?;
    writeln!(w, "/* Includes from Import / UImport directives */")?;
    write_imports(w, stmts, &[ImportKind::Import, ImportKind::UImport])?;
    writeln!(w)?;

    let base = opts.subsystem_base;
    let mut seq = 0u32;

    for stmt in stmts {
        if let Statement::Routine(rt) = stmt {
            let msg_id = base + seq;
            write_user_routine(w, rt, msg_id, opts)?;
            seq += 1;
        }
    }

    Ok(())
}

fn write_user_routine(
    w: &mut dyn Write,
    rt: &crate::ast::Routine,
    msg_id: u32,
    opts: &Options,
) -> io::Result<()> {
    let user_name = format!("{}{}", opts.user_prefix, rt.name);
    let is_simple = rt.kind == RoutineKind::SimpleRoutine;

    writeln!(w)?;
    writeln!(
        w,
        "/* {} {} */",
        if is_simple {
            "SimpleRoutine"
        } else {
            "Routine"
        },
        rt.name
    )?;

    write_mig_external(w)?;
    let ret_type = if is_simple {
        "mach_msg_return_t"
    } else {
        "kern_return_t"
    };
    writeln!(w, "{ret_type}")?;
    write!(w, "{user_name}(")?;

    let all_user_args: Vec<&crate::ast::Argument> = rt
        .args
        .iter()
        .filter(|a| {
            !matches!(
                a.direction,
                Direction::ServerImpl
                    | Direction::ServerSecToken
                    | Direction::ServerAuditToken
                    | Direction::ServerContextToken
            )
        })
        .collect();

    if all_user_args.is_empty() {
        write!(w, "mach_port_t request_port")?;
    } else {
        for (i, arg) in all_user_args.iter().enumerate() {
            if i > 0 {
                write!(w, ",\n\t")?;
            }
            let ty = arg.ty.user_type.as_deref().unwrap_or("int");
            let is_out = matches!(arg.direction, Direction::Out | Direction::InOut);
            if is_out {
                write!(w, "\t{ty} *{}", arg.name)?;
            } else {
                write!(w, "\t{ty} {}", arg.name)?;
            }
        }
    }
    writeln!(w, ")")?;
    writeln!(w, "{{")?;

    // Request message struct
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
        // Reply message struct
        writeln!(w, "\ttypedef struct {{")?;
        writeln!(w, "\t\tmach_msg_header_t Head;")?;
        writeln!(w, "\t\tNDR_record_t NDR;")?;
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

    writeln!(w, "\tunion {{")?;
    writeln!(w, "\t\tRequest In;")?;
    if !is_simple {
        writeln!(w, "\t\tReply Out;")?;
    }
    writeln!(w, "\t}} Mess;")?;
    writeln!(w)?;
    writeln!(w, "\tRequest *InP = &Mess.In;")?;
    if !is_simple {
        writeln!(w, "\tReply *OutP = &Mess.Out;")?;
    }
    writeln!(w)?;

    // Fill request header
    writeln!(w, "\tInP->Head.msgh_bits =")?;
    writeln!(
        w,
        "\t\tMACH_MSGH_BITS(MACH_MSG_TYPE_COPY_SEND, MACH_MSG_TYPE_MAKE_SEND_ONCE);"
    )?;
    writeln!(
        w,
        "\tInP->Head.msgh_size = (mach_msg_size_t)sizeof(Request);"
    )?;
    writeln!(w, "\tInP->Head.msgh_id = {msg_id};")?;
    writeln!(w, "\tInP->NDR = NDR_record;")?;
    writeln!(w)?;

    // Pack in-arguments
    for arg in rt.args.iter().filter(|a| {
        matches!(
            a.direction,
            Direction::In | Direction::InOut | Direction::None
        )
    }) {
        writeln!(w, "\tInP->{name} = {name};", name = arg.name)?;
    }
    writeln!(w)?;

    // Send (and receive if not simple)
    if is_simple {
        writeln!(
            w,
            "\treturn mach_msg(&InP->Head, MACH_SEND_MSG | MACH_MSG_OPTION_NONE,"
        )?;
        writeln!(
            w,
            "\t\t(mach_msg_size_t)sizeof(Request), 0, MACH_PORT_NULL,"
        )?;
        writeln!(w, "\t\tMACH_MSG_TIMEOUT_NONE, MACH_PORT_NULL);")?;
    } else {
        writeln!(w, "\t{{")?;
        writeln!(w, "\t\tkern_return_t ret = mach_msg(&InP->Head,")?;
        writeln!(
            w,
            "\t\t\tMACH_SEND_MSG | MACH_RCV_MSG | MACH_MSG_OPTION_NONE,"
        )?;
        writeln!(w, "\t\t\t(mach_msg_size_t)sizeof(Request),")?;
        writeln!(w, "\t\t\t(mach_msg_size_t)sizeof(Reply),")?;
        writeln!(w, "\t\t\tInP->Head.msgh_local_port,")?;
        writeln!(w, "\t\t\tMACH_MSG_TIMEOUT_NONE, MACH_PORT_NULL);")?;
        writeln!(w, "\t\tif (ret != MACH_MSG_SUCCESS) return ret;")?;
        writeln!(w, "\t}}")?;
        writeln!(w)?;
        // Unpack out-arguments
        for arg in rt
            .args
            .iter()
            .filter(|a| matches!(a.direction, Direction::Out | Direction::InOut))
        {
            writeln!(w, "\tif ({name}) *{name} = OutP->{name};", name = arg.name)?;
        }
        writeln!(w)?;
        writeln!(w, "\treturn OutP->RetCode;")?;
    }

    writeln!(w, "}}")
}
