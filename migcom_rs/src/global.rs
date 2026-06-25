// SPDX-License-Identifier: MIT
// Global compiler options (mirrors global.h / global.c)

use crate::ast::ConsumeOnSendError;

pub const MIG_VERSION: &str = "migcom (Rust rewrite) 0.1.0";

#[derive(Debug)]
pub struct Options {
    // Verbosity
    pub quiet: bool,
    pub verbose: bool,

    // Code generation flags
    pub use_msg_rpc: bool,
    pub gen_sym_tab: bool,
    pub use_event_logger: bool,
    pub be_ansi_c: bool,
    pub check_ndr: bool,
    pub use_split_headers: bool,
    pub short_circuit: bool,
    pub use_rpc_trap: bool,
    pub test_rpc_trap: bool,
    pub is_voucher_code_allowed: bool,
    pub emit_count_annotations: bool,
    pub use_mach_msg2: bool,

    // Subsystem modifiers
    pub is_kernel_user: bool,
    pub is_kernel_server: bool,

    // UseSpecialReplyPort
    pub use_special_reply_port: bool,
    pub has_use_special_reply_port: bool,

    // ConsumeOnSendError
    pub consume_on_send_error: ConsumeOnSendError,
    pub has_consume_on_send_error: bool,

    // Descriptor limits
    pub max_server_descrs: Option<i32>,
    pub max_server_reply_descrs: Option<i32>,

    // Stack/type limits
    pub max_mess_size_on_stack: Option<i32>,
    pub user_type_limit: Option<i32>,

    // Subsystem state (filled in after parsing)
    pub subsystem_name: Option<String>,
    pub subsystem_base: u32,
    pub rcs_id: Option<String>,
    pub msg_option: Option<String>,
    pub wait_time: Option<String>,
    pub send_time: Option<String>,
    pub error_proc: String,
    pub server_prefix: String,
    pub user_prefix: String,
    pub server_demux: Option<String>,

    // Output filenames (None → /dev/null)
    pub user_file_prefix: Option<String>,
    pub user_header_filename: Option<String>,
    pub server_header_filename: Option<String>,
    pub internal_header_filename: Option<String>,
    pub defines_header_filename: Option<String>,
    pub user_filename: Option<String>,
    pub server_filename: Option<String>,

    pub print_version: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            quiet: false,
            verbose: false,
            use_msg_rpc: true,
            gen_sym_tab: false,
            use_event_logger: false,
            be_ansi_c: true,
            check_ndr: false,
            use_split_headers: false,
            short_circuit: false,
            use_rpc_trap: false,
            test_rpc_trap: false,
            is_voucher_code_allowed: true,
            emit_count_annotations: false,
            use_mach_msg2: false,
            is_kernel_user: false,
            is_kernel_server: false,
            use_special_reply_port: false,
            has_use_special_reply_port: false,
            consume_on_send_error: ConsumeOnSendError::None,
            has_consume_on_send_error: false,
            max_server_descrs: None,
            max_server_reply_descrs: None,
            max_mess_size_on_stack: None,
            user_type_limit: None,
            subsystem_name: None,
            subsystem_base: 0,
            rcs_id: None,
            msg_option: None,
            wait_time: None,
            send_time: None,
            error_proc: "MsgError".into(),
            server_prefix: String::new(),
            user_prefix: String::new(),
            server_demux: None,
            user_file_prefix: None,
            user_header_filename: None,
            server_header_filename: None,
            internal_header_filename: None,
            defines_header_filename: None,
            user_filename: None,
            server_filename: None,
            print_version: false,
        }
    }
}

impl Options {
    /// Fill in derived file names after the subsystem name is known.
    pub fn finalize(&mut self) -> Result<(), String> {
        let name = self
            .subsystem_name
            .as_deref()
            .ok_or_else(|| "no SubSystem declaration".to_string())?;

        if self.user_header_filename.is_none() {
            self.user_header_filename = Some(format!("{name}.h"));
        } else if self.user_header_filename.as_deref() == Some("/dev/null") {
            self.user_header_filename = None;
        }

        if self.user_filename.is_none() {
            self.user_filename = Some(format!("{name}User.c"));
        } else if self.user_filename.as_deref() == Some("/dev/null") {
            self.user_filename = None;
        }

        if self.server_filename.is_none() {
            self.server_filename = Some(format!("{name}Server.c"));
        } else if self.server_filename.as_deref() == Some("/dev/null") {
            self.server_filename = None;
        }

        if self.server_demux.is_none() {
            self.server_demux = Some(format!("{name}_server"));
        }

        if self.use_mach_msg2 && (!self.be_ansi_c || self.use_rpc_trap || self.check_ndr) {
            return Err("KernelServer does not support the given options with -mach_msg2".into());
        }

        Ok(())
    }

    #[allow(dead_code)]
    pub fn output_path<'a>(&self, path: &'a Option<String>) -> Option<&'a str> {
        path.as_deref()
    }
}
