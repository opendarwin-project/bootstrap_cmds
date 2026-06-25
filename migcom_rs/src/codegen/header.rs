// SPDX-License-Identifier: MIT
// User-side header generation (mirrors header.c)

use std::io::{self, Write};

use super::utils::{write_identification, write_imports, write_mig_external};
use crate::ast::{Argument, Direction, ImportKind, RoutineKind, Statement};
use crate::global::Options;

/// Pick the C type to use for an argument in a function prototype.
///
/// In the MIG type system the `name` is the .defs-level identifier
/// (e.g. `task_t`).  For port types the kernel expects the typedef name,
/// not the underlying `mach_port_t`, because `<mach/mach_types.h>` maps
/// these to `struct task *` / `struct thread *` etc. inside the kernel.
/// Return the C type name for a routine argument in the **user** header.
/// Prefers `user_type` (set by `cusertype:`) over the declared alias name,
/// so that e.g. `semaphore_consume_ref_t cusertype: semaphore_t` produces
/// `semaphore_t` in the prototype — a type that is visible in kernel context.
fn arg_user_proto_type(arg: &Argument) -> &str {
    arg.ty
        .user_type
        .as_deref()
        .or(arg.ty.name.as_deref())
        .or(arg.ty.server_type.as_deref())
        .unwrap_or("int")
}

/// Return the C type name for a routine argument in the **server** header.
/// Prefers the declared alias name (which is the intran result type for
/// port args) so that the server stub signature matches the implementation.
fn arg_server_proto_type(arg: &Argument) -> &str {
    arg.ty
        .name
        .as_deref()
        .or(arg.ty.server_type.as_deref())
        .or(arg.ty.user_type.as_deref())
        .unwrap_or("int")
}

/// Emit typedefs for port-type aliases whose user_type differs from
/// the declared name (e.g. `typedef mach_port_t mach_port_copy_send_t`).
///
/// Types that declare translations (intran/outtran/destructor) — like
/// `task_t`, `thread_t` — are guarded with `#ifndef KERNEL` because
/// `<mach/mach_types.h>` already maps them to struct pointers inside
/// the kernel.  Consume-ref aliases (`mach_port_copy_send_t` …) have no
/// translations and are emitted unconditionally.
fn write_type_aliases(w: &mut dyn Write, stmts: &[Statement]) -> io::Result<()> {
    // Suppressed entirely: mach/port.h redefines with incompatible types, and
    // consume-ref aliases (semaphore_consume_ref_t etc.) are accessed through their
    // cusertype in prototypes, so we don't need a standalone typedef for the alias.
    let suppress_entirely: &[&str] = &[
        "exception_handler_array_t",
        // consume-ref aliases: cusertype (e.g. semaphore_t) is used in prototypes instead
        "semaphore_consume_ref_t",
        "task_suspension_token_t",
    ];

    let mut unconditional: Vec<&String> = Vec::new();
    let mut kernel_guarded: Vec<&String> = Vec::new();
    let mut emitted = std::collections::HashSet::new();
    for stmt in stmts {
        if let Statement::TypeDecl { name, ty } = stmt {
            // Filter rules:
            //  - `ut == name`: no cusertype set; parse_type_decl left user_type=name.
            //    These are intran-only kernel-struct types — mach_types.h defines them.
            //  - `var_array`: OOL port arrays — skip (mach_types.h defines them).
            //  - `suppress_entirely`: explicitly excluded above.
            //  - `from_mach_port`: named `= mach_port_t` alias → #ifndef KERNEL guard.
            //  - `has_trans` (intran/outtran): kernel redefines as struct → guard.
            //  - otherwise: wire-type alias (mach_port_copy_send_t etc.) → unconditional.
            if let Some(ref ut) = ty.user_type {
                if ut != name && ty.port_type && !ty.var_array && emitted.insert(name.as_str()) {
                    if suppress_entirely.contains(&name.as_str()) {
                        continue;
                    }
                    let has_trans =
                        ty.in_trans.is_some() || ty.out_trans.is_some() || ty.destructor.is_some();
                    let from_mach_port = ty.name.as_deref() == Some("mach_port_t");
                    if has_trans || from_mach_port {
                        kernel_guarded.push(name);
                    } else {
                        unconditional.push(name);
                    }
                }
            }
        }
    }
    for name in &unconditional {
        writeln!(w, "typedef mach_port_t {name};")?;
    }
    if !kernel_guarded.is_empty() {
        writeln!(w, "#ifndef KERNEL")?;
        for name in &kernel_guarded {
            writeln!(w, "typedef mach_port_t {name};")?;
        }
        writeln!(w, "#endif /* !KERNEL */")?;
    }
    Ok(())
}

pub fn write_user_header(w: &mut dyn Write, stmts: &[Statement], opts: &Options) -> io::Result<()> {
    let subsys = opts.subsystem_name.as_deref().unwrap_or("unknown");
    let guard = format!("_{}_user_", subsys.to_uppercase());

    write_identification(w, opts)?;
    writeln!(w)?;
    writeln!(w, "#ifndef\t{guard}")?;
    writeln!(w, "#define\t{guard}")?;
    writeln!(w)?;
    writeln!(w, "/* Module {subsys} */")?;
    writeln!(w)?;
    writeln!(w, "#include <mach/message.h>")?;
    writeln!(w, "#include <mach/ndr.h>")?;
    writeln!(w, "#include <mach/mig.h>")?;
    writeln!(w)?;

    write_imports(w, stmts, &[ImportKind::Import, ImportKind::UImport])?;
    writeln!(w)?;

    write_type_aliases(w, stmts)?;
    writeln!(w)?;

    if opts.be_ansi_c {
        writeln!(w, "#ifdef\t__MigTypeCheck")?;
        writeln!(w, "#define\t__MIG_check__user_{subsys}_subsystem__")?;
        writeln!(w, "#endif")?;
        writeln!(w)?;
    }

    // Emit extern declarations for each routine
    for stmt in stmts {
        if let Statement::Routine(rt) = stmt {
            write_mig_external(w)?;
            let user_name = format!("{}{}", opts.user_prefix, rt.name);
            let ret = if rt.kind == RoutineKind::Routine {
                "kern_return_t"
            } else {
                "mach_msg_return_t"
            };
            write!(w, "{ret}\n{user_name}(")?;
            // Emit argument list, injecting implicit count parameters after
            // every variable-length array argument (CountInOut / var_array).
            // Out/InOut scalar args get a * pointer (the caller passes &var to receive).
            let mut args: Vec<String> = Vec::new();
            for a in &rt.args {
                let ty = arg_user_proto_type(a);
                let is_out = matches!(a.direction, Direction::Out | Direction::InOut);
                if is_out && !a.ty.var_array {
                    args.push(format!("{ty} *{}", a.name));
                } else {
                    args.push(format!("{ty} {}", a.name));
                }
                // Inject *nameCnt / nameCnt for variable-length arrays
                if a.ty.var_array {
                    if is_out {
                        args.push(format!("mach_msg_type_number_t *{}Cnt", a.name));
                    } else {
                        args.push(format!("mach_msg_type_number_t {}Cnt", a.name));
                    }
                }
            }
            write!(w, "{}", args.join(",\n\t"))?;
            writeln!(w, ");")?;
            writeln!(w)?;
        }
    }

    writeln!(w, "#endif\t/* not defined({guard}) */")?;
    Ok(())
}

pub fn write_server_header(
    w: &mut dyn Write,
    stmts: &[Statement],
    opts: &Options,
) -> io::Result<()> {
    let subsys = opts.subsystem_name.as_deref().unwrap_or("unknown");
    let guard = format!("_{}_server_", subsys.to_uppercase());

    write_identification(w, opts)?;
    writeln!(w)?;
    writeln!(w, "#ifndef\t{guard}")?;
    writeln!(w, "#define\t{guard}")?;
    writeln!(w)?;
    writeln!(w, "/* Module {subsys} */")?;
    writeln!(w)?;
    writeln!(w, "#include <mach/mach_types.h>")?;
    writeln!(w, "#include <mach/message.h>")?;
    writeln!(w, "#include <mach/mig.h>")?;
    writeln!(w)?;

    write_imports(w, stmts, &[ImportKind::Import, ImportKind::SImport])?;
    writeln!(w)?;

    for stmt in stmts {
        if let Statement::Routine(rt) = stmt {
            let srv_name = format!("{}{}", opts.server_prefix, rt.name);
            let ret = if rt.kind == RoutineKind::Routine {
                "kern_return_t"
            } else {
                "void"
            };
            write!(w, "extern {ret} {srv_name}(")?;
            // Build the arg list, injecting an implicit count parameter after
            // every variable-length array argument (CountInOut / var_array).
            // Out/InOut non-array args also get a * pointer (kernel impl convention).
            let mut args: Vec<String> = Vec::new();
            for a in &rt.args {
                let ty = arg_server_proto_type(a);
                let is_out = matches!(a.direction, Direction::Out | Direction::InOut);
                if is_out && !a.ty.var_array {
                    // Out/InOut scalar/port args are passed by pointer in the server impl
                    args.push(format!("{ty} *{}", a.name));
                } else {
                    args.push(format!("{ty} {}", a.name));
                }
                // Inject *nameCnt / nameCnt for variable-length arrays
                if a.ty.var_array {
                    if is_out {
                        args.push(format!("mach_msg_type_number_t *{}Cnt", a.name));
                    } else {
                        args.push(format!("mach_msg_type_number_t {}Cnt", a.name));
                    }
                }
            }
            write!(w, "{}", args.join(",\n\t"))?;
            writeln!(w, ");")?;
            writeln!(w)?;
        }
    }

    writeln!(w, "#endif\t/* not defined({guard}) */")?;
    Ok(())
}

pub fn write_defines_header(
    w: &mut dyn Write,
    stmts: &[Statement],
    opts: &Options,
) -> io::Result<()> {
    let subsys = opts.subsystem_name.as_deref().unwrap_or("unknown");
    write_identification(w, opts)?;
    writeln!(w)?;
    writeln!(w, "/* Subsystem message-id defines for {subsys} */")?;
    writeln!(w)?;
    let base = opts.subsystem_base;
    let mut seq = 0u32;
    for stmt in stmts {
        if let Statement::Routine(rt) = stmt {
            writeln!(
                w,
                "#define MACH_MSG_ID_{} ({})",
                rt.name.to_uppercase(),
                base + seq
            )?;
            seq += 1;
        }
    }
    Ok(())
}
