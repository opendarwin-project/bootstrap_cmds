// SPDX-License-Identifier: MIT
// migcom — Mach Interface Generator compiler (Rust rewrite)
//
// This binary reads a pre-processed .defs file from stdin (the C preprocessor
// is invoked by the `mig` shell wrapper, exactly as in the original) and
// writes the user-side header, user stub, and server stub to the paths
// specified via command-line flags.

mod ast;
mod codegen;
mod diag;
mod global;
mod lexer;
mod lower;
mod parser;
mod types;

use std::fs::File;
use std::io::{self, BufWriter, Read, Write};
use std::process;

use diag::Diag;
use global::{MIG_VERSION, Options};

// ---------------------------------------------------------------------------
// Argument parsing
// ---------------------------------------------------------------------------

fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut opts = Options::default();
    let mut i = 0usize;

    // Handle the special single-argument `-version` case first
    if args == ["-version"] {
        opts.print_version = true;
        return Ok(opts);
    }

    while i < args.len() {
        macro_rules! next_arg {
            ($flag:expr) => {{
                i += 1;
                args.get(i)
                    .ok_or_else(|| format!("missing argument for {}", $flag))?
                    .as_str()
            }};
        }

        match args[i].as_str() {
            "-q" => opts.quiet = true,
            "-Q" => opts.quiet = false,
            "-v" => opts.verbose = true,
            "-V" => opts.verbose = false,
            "-r" => opts.use_msg_rpc = true,
            "-R" => opts.use_msg_rpc = false,
            "-l" => opts.use_event_logger = false,
            "-L" => opts.use_event_logger = true,
            "-k" => opts.be_ansi_c = true,
            "-K" => opts.be_ansi_c = false,
            "-n" => opts.check_ndr = true,
            "-N" => opts.check_ndr = false,
            "-s" => opts.gen_sym_tab = true,
            "-S" => opts.gen_sym_tab = false,
            "-t" => {
                opts.test_rpc_trap = true;
                opts.use_rpc_trap = true;
            }
            "-T" => opts.use_rpc_trap = false,
            "-x" => opts.short_circuit = true,
            "-X" => opts.short_circuit = false,
            "-b" => opts.emit_count_annotations = true,
            "-B" => opts.emit_count_annotations = false,
            "-split" => opts.use_split_headers = true,
            "-novouchers" => opts.is_voucher_code_allowed = false,
            "-mach_msg2" => opts.use_mach_msg2 = true,
            "-user" => {
                opts.user_filename = Some(next_arg!("-user").to_string());
            }
            "-server" => {
                opts.server_filename = Some(next_arg!("-server").to_string());
            }
            "-header" => {
                opts.user_header_filename = Some(next_arg!("-header").to_string());
            }
            "-sheader" => {
                opts.server_header_filename = Some(next_arg!("-sheader").to_string());
            }
            "-iheader" => {
                opts.internal_header_filename = Some(next_arg!("-iheader").to_string());
            }
            "-dheader" => {
                opts.defines_header_filename = Some(next_arg!("-dheader").to_string());
            }
            "-i" => {
                opts.user_file_prefix = Some(next_arg!("-i").to_string());
            }
            "-maxonstack" => {
                let v = next_arg!("-maxonstack")
                    .parse::<i32>()
                    .map_err(|_| "-maxonstack requires an integer".to_string())?;
                opts.max_mess_size_on_stack = Some(v);
            }
            "-max_descrs" => {
                let v = next_arg!("-max_descrs")
                    .parse::<i32>()
                    .map_err(|_| "-max_descrs requires an integer".to_string())?;
                opts.max_server_descrs = Some(v);
            }
            "-max_reply_descrs" => {
                let v = next_arg!("-max_reply_descrs")
                    .parse::<i32>()
                    .map_err(|_| "-max_reply_descrs requires an integer".to_string())?;
                opts.max_server_reply_descrs = Some(v);
            }
            other => return Err(format!("unknown flag: '{other}'")),
        }
        i += 1;
    }

    Ok(opts)
}

// ---------------------------------------------------------------------------
// Output helper
// ---------------------------------------------------------------------------

fn open_output(path: Option<&str>) -> io::Result<Box<dyn Write>> {
    match path {
        None => Ok(Box::new(io::sink())),
        Some(p) => {
            let f = File::create(p)
                .map_err(|e| io::Error::new(e.kind(), format!("cannot open '{p}': {e}")))?;
            Ok(Box::new(BufWriter::new(f)))
        }
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    let raw_args: Vec<String> = std::env::args().skip(1).collect();

    let mut opts = match parse_args(&raw_args) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("migcom: {e}");
            process::exit(1);
        }
    };

    if opts.print_version {
        println!("{MIG_VERSION}");
        return;
    }

    // Read pre-processed source from stdin
    let mut src = String::new();
    if let Err(e) = io::stdin().read_to_string(&mut src) {
        eprintln!("migcom: reading stdin: {e}");
        process::exit(1);
    }

    // Lex
    let tokens = lexer::tokenise(&src);

    // Parse
    let mut diag = Diag::new("<stdin>", &src, opts.quiet);
    let mut p = parser::Parser::new(&tokens, &mut diag);
    let stmts = p.parse_statements();
    // Move the populated type table back out (we don't need it further here)
    drop(p);

    if diag.error_count > 0 {
        eprintln!("migcom: {} error(s) found — aborting", diag.error_count);
        process::exit(1);
    }

    // Semantic lowering — fill in Options from parsed statements
    lower::lower(stmts.clone(), &mut opts, &mut diag);

    if diag.error_count > 0 {
        process::exit(1);
    }

    // Finalize derived file names and validate option combinations
    if let Err(e) = opts.finalize() {
        eprintln!("migcom: {e}");
        process::exit(1);
    }

    if opts.verbose {
        eprintln!(
            "Subsystem {}: base = {}{}{}",
            opts.subsystem_name.as_deref().unwrap_or("?"),
            opts.subsystem_base,
            if opts.is_kernel_user {
                ", KernelUser"
            } else {
                ""
            },
            if opts.is_kernel_server {
                ", KernelServer"
            } else {
                ""
            },
        );
    }

    // -----------------------------------------------------------------------
    // Code generation
    // -----------------------------------------------------------------------

    // User header
    let uheader_path = opts.user_header_filename.as_deref();
    if opts.verbose {
        eprintln!("Writing {} …", uheader_path.unwrap_or("/dev/null"));
    }
    let mut uheader = open_output(uheader_path).unwrap_or_else(|e| {
        eprintln!("migcom: {e}");
        process::exit(1);
    });
    codegen::header::write_user_header(&mut *uheader, &stmts, &opts).unwrap_or_else(|e| {
        eprintln!("migcom: writing user header: {e}");
        process::exit(1);
    });

    // Server header (optional)
    if let Some(path) = opts.server_header_filename.as_deref() {
        if opts.verbose {
            eprintln!("Writing {path} …");
        }
        let mut sh = open_output(Some(path)).unwrap_or_else(|e| {
            eprintln!("migcom: {e}");
            process::exit(1);
        });
        codegen::header::write_server_header(&mut *sh, &stmts, &opts).unwrap_or_else(|e| {
            eprintln!("migcom: writing server header: {e}");
            process::exit(1);
        });
    }

    // Defines header (optional)
    if let Some(path) = opts.defines_header_filename.as_deref() {
        if opts.verbose {
            eprintln!("Writing {path} …");
        }
        let mut dh = open_output(Some(path)).unwrap_or_else(|e| {
            eprintln!("migcom: {e}");
            process::exit(1);
        });
        codegen::header::write_defines_header(&mut *dh, &stmts, &opts).unwrap_or_else(|e| {
            eprintln!("migcom: writing defines header: {e}");
            process::exit(1);
        });
    }

    // User stub
    if let Some(prefix) = opts.user_file_prefix.as_deref() {
        // -i mode: one file per routine
        if opts.verbose {
            eprintln!("Writing individual user files …");
        }
        for stmt in &stmts {
            if let ast::Statement::Routine(rt) = stmt {
                let path = format!("{prefix}{}.c", rt.name);
                if opts.verbose {
                    eprintln!("  {path}");
                }
                let mut f = open_output(Some(&path)).unwrap_or_else(|e| {
                    eprintln!("migcom: {e}");
                    process::exit(1);
                });
                // Write a single-routine user stub
                codegen::user::write_user(&mut *f, std::slice::from_ref(stmt), &opts)
                    .unwrap_or_else(|e| {
                        eprintln!("migcom: {e}");
                        process::exit(1);
                    });
            }
        }
    } else {
        let user_path = opts.user_filename.as_deref();
        if opts.verbose {
            eprintln!("Writing {} …", user_path.unwrap_or("/dev/null"));
        }
        let mut user = open_output(user_path).unwrap_or_else(|e| {
            eprintln!("migcom: {e}");
            process::exit(1);
        });
        codegen::user::write_user(&mut *user, &stmts, &opts).unwrap_or_else(|e| {
            eprintln!("migcom: writing user stub: {e}");
            process::exit(1);
        });
    }

    // Server stub
    let server_path = opts.server_filename.as_deref();
    if opts.verbose {
        eprintln!("Writing {} …", server_path.unwrap_or("/dev/null"));
    }
    let mut server = open_output(server_path).unwrap_or_else(|e| {
        eprintln!("migcom: {e}");
        process::exit(1);
    });
    codegen::server::write_server(&mut *server, &stmts, &opts).unwrap_or_else(|e| {
        eprintln!("migcom: writing server stub: {e}");
        process::exit(1);
    });

    if opts.verbose {
        eprintln!("done.");
    }
}
