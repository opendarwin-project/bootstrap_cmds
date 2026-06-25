// SPDX-License-Identifier: MIT
// Semantic lowering: walk the parsed Statement list and apply global options.

use crate::ast::*;
use crate::diag::Diag;
use crate::global::Options;

/// Walk parsed statements and fill in `Options` from the semantic declarations.
/// Returns the list of routines and import statements in order.
pub fn lower(stmts: Vec<Statement>, opts: &mut Options, diag: &mut Diag) {
    for stmt in stmts {
        match stmt {
            Statement::Subsystem {
                name,
                base,
                is_kernel_user,
                is_kernel_server,
            } => {
                if opts.subsystem_name.is_some() {
                    diag.error_noloc(&format!(
                        "previous Subsystem declaration ({}) will be ignored",
                        opts.subsystem_name.as_deref().unwrap_or("?")
                    ));
                }
                opts.subsystem_name = Some(name);
                opts.subsystem_base = base;
                if is_kernel_user {
                    if opts.is_kernel_user {
                        diag.error_noloc("duplicate KernelUser keyword");
                    }
                    if !opts.use_msg_rpc {
                        diag.error_noloc("with KernelUser the -R option is meaningless");
                        opts.use_msg_rpc = true;
                    }
                    opts.is_kernel_user = true;
                }
                if is_kernel_server {
                    if opts.is_kernel_server {
                        diag.error_noloc("duplicate KernelServer keyword");
                    }
                    opts.is_kernel_server = true;
                }
            }
            Statement::WaitTime(s) => {
                opts.wait_time = Some(s);
            }
            Statement::NoWaitTime => {
                opts.wait_time = None;
            }
            Statement::SendTime(s) => {
                opts.send_time = Some(s);
            }
            Statement::NoSendTime => {
                opts.send_time = None;
            }
            Statement::MsgOption(o) => {
                opts.msg_option = o;
            }
            Statement::UseSpecialReplyPort(v) => {
                opts.use_special_reply_port = v;
                if v {
                    opts.has_use_special_reply_port = true;
                }
            }
            Statement::ConsumeOnSendError(v) => {
                opts.consume_on_send_error = v;
                if v != ConsumeOnSendError::None {
                    opts.has_consume_on_send_error = true;
                }
            }
            Statement::UserTypeLimit(n) => {
                opts.user_type_limit = Some(n as i32);
            }
            Statement::OnStackLimit(n) => {
                opts.max_mess_size_on_stack = Some(n as i32);
            }
            Statement::ErrorProc(s) => {
                opts.error_proc = s;
            }
            Statement::ServerPrefix(s) => {
                opts.server_prefix = s;
            }
            Statement::UserPrefix(s) => {
                opts.user_prefix = s;
            }
            Statement::ServerDemux(s) => {
                opts.server_demux = Some(s);
            }
            Statement::RCSDecl(s) => {
                opts.rcs_id = Some(s);
            }
            // TypeDecl and Routine are handled by the parser/type-table already
            Statement::TypeDecl { .. }
            | Statement::Routine(_)
            | Statement::Skip
            | Statement::Import { .. } => {}
        }
    }
}
