// SPDX-License-Identifier: MIT
// MIG type system — ipc_type_t and friends

use std::collections::HashMap;

/// IPC wire type numbers (subset carried by the MIG grammar)
pub mod ipc_type_num {
    pub const MACH_MSG_TYPE_UNSTRUCTURED: u32 = 0;
    pub const MACH_MSG_TYPE_BIT: u32 = 0;
    pub const MACH_MSG_TYPE_BOOLEAN: u32 = 0;
    pub const MACH_MSG_TYPE_INTEGER_8: u32 = 9;
    pub const MACH_MSG_TYPE_INTEGER_16: u32 = 1;
    pub const MACH_MSG_TYPE_INTEGER_32: u32 = 2;
    pub const MACH_MSG_TYPE_INTEGER_64: u32 = 3;
    pub const MACH_MSG_TYPE_CHAR: u32 = 8;
    pub const MACH_MSG_TYPE_BYTE: u32 = 9;
    pub const MACH_MSG_TYPE_REAL_32: u32 = 10;
    pub const MACH_MSG_TYPE_REAL_64: u32 = 11;
    pub const MACH_MSG_TYPE_STRING_C: u32 = 12;

    // Port types (from mach/message.h values used by MIG)
    pub const MACH_MSG_TYPE_MOVE_RECEIVE: u32 = 16;
    pub const MACH_MSG_TYPE_COPY_SEND: u32 = 19;
    pub const MACH_MSG_TYPE_MAKE_SEND: u32 = 20;
    pub const MACH_MSG_TYPE_MOVE_SEND: u32 = 17;
    pub const MACH_MSG_TYPE_MAKE_SEND_ONCE: u32 = 21;
    pub const MACH_MSG_TYPE_MOVE_SEND_ONCE: u32 = 18;
    pub const MACH_MSG_TYPE_PORT_NAME: u32 = 15;
    pub const MACH_MSG_TYPE_PORT_RECEIVE: u32 = 16;
    pub const MACH_MSG_TYPE_PORT_SEND: u32 = 17;
    pub const MACH_MSG_TYPE_PORT_SEND_ONCE: u32 = 18;
    pub const MACH_MSG_TYPE_POLYMORPHIC: u32 = 2005; // sentinel used by MIG

    pub const PORT_SIZE: u32 = 32; // sizeof(mach_port_t)*NBBY on ILP32; 64 on LP64
}

/// ipc_flags_t — per-argument IPC flags (replaces C bitfield macros)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IpcFlags(pub u32);

impl IpcFlags {
    pub const NONE: Self = Self(0x000);
    pub const PHYSICAL_COPY: Self = Self(0x001);
    pub const OVERWRITE: Self = Self(0x002);
    pub const DEALLOC: Self = Self(0x004);
    pub const NOT_DEALLOC: Self = Self(0x008);
    pub const MAYBE_DEALLOC: Self = Self(0x010);
    pub const SAME_COUNT: Self = Self(0x020);
    pub const COUNT_IN_OUT: Self = Self(0x040);
    pub const RET_CODE: Self = Self(0x080);
    pub const AUTO: Self = Self(0x100);
    pub const CONST: Self = Self(0x200);

    #[allow(dead_code)]
    pub fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

impl std::ops::BitOr for IpcFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for IpcFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Dealloc {
    No,
    Yes,
    Maybe,
}

/// The resolved IPC type for one argument (mirrors ipc_type_t)
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct IpcType {
    pub name: Option<String>,

    pub type_size: u32,
    pub pad_size: u32,
    pub min_type_size: u32,

    pub in_name: u32,
    pub out_name: u32,
    pub size: u32,
    pub number: u32,
    pub kpd_number: u32,

    pub in_line: bool,
    pub mig_in_line: bool,
    pub port_type: bool,

    pub in_name_str: Option<String>,
    pub out_name_str: Option<String>,

    pub is_struct: bool,
    pub is_string: bool,
    pub var_array: bool,
    pub no_opt_array: bool,
    pub native: bool,
    pub native_pointer: bool,

    pub element: Option<Box<IpcType>>,

    pub user_type: Option<String>,
    pub server_type: Option<String>,
    pub trans_type: Option<String>,
    pub kpd_type: Option<String>,

    pub in_trans: Option<String>,
    pub out_trans: Option<String>,
    pub destructor: Option<String>,
    pub bad_value: Option<String>,

    pub ool_number: u32,
}

impl IpcType {
    /// Build a short-form basic type declaration.
    pub fn short_decl(
        in_name: u32,
        in_str: Option<String>,
        out_name: u32,
        out_str: Option<String>,
        size: u32,
    ) -> Self {
        IpcType {
            name: None,
            type_size: 0,
            pad_size: 0,
            min_type_size: 0,
            in_name,
            out_name,
            size,
            number: 0,
            kpd_number: 0,
            in_line: true,
            mig_in_line: false,
            port_type: false,
            in_name_str: in_str,
            out_name_str: out_str,
            is_struct: false,
            is_string: false,
            var_array: false,
            no_opt_array: false,
            native: false,
            native_pointer: false,
            element: None,
            user_type: None,
            server_type: None,
            trans_type: None,
            kpd_type: None,
            in_trans: None,
            out_trans: None,
            destructor: None,
            bad_value: None,
            ool_number: 0,
        }
    }

    /// `*T` — pointer to element
    pub fn ptr_decl(inner: IpcType) -> Self {
        let mut t = inner.clone();
        t.in_line = false;
        t.is_struct = true; // pointers can be assigned with =
        t.element = Some(Box::new(inner));
        t
    }

    /// Fixed-size array
    pub fn array_decl(number: u32, elem: IpcType) -> Self {
        let mut t = elem.clone();
        t.number = number;
        t.var_array = false;
        t.element = Some(Box::new(elem));
        t
    }

    /// Variable-length array (max capped at `max`)
    pub fn var_array_decl(max: u32, elem: IpcType) -> Self {
        let mut t = elem.clone();
        t.number = max;
        t.var_array = true;
        t.element = Some(Box::new(elem));
        t
    }

    /// Struct (inline aggregate)
    pub fn struct_decl(number: u32, elem: IpcType) -> Self {
        let mut t = elem.clone();
        t.number = number;
        t.is_struct = true;
        t.element = Some(Box::new(elem));
        t
    }

    /// C-string of fixed/variable length
    pub fn cstring_decl(count: u32, varying: bool) -> Self {
        IpcType {
            name: None,
            type_size: count,
            pad_size: 0,
            min_type_size: if varying { 0 } else { count },
            in_name: ipc_type_num::MACH_MSG_TYPE_STRING_C,
            out_name: ipc_type_num::MACH_MSG_TYPE_STRING_C,
            size: 8,
            number: count,
            kpd_number: 0,
            in_line: true,
            mig_in_line: false,
            port_type: false,
            in_name_str: Some("MACH_MSG_TYPE_STRING_C".into()),
            out_name_str: Some("MACH_MSG_TYPE_STRING_C".into()),
            is_struct: false,
            is_string: true,
            var_array: varying,
            no_opt_array: false,
            native: false,
            native_pointer: false,
            element: None,
            user_type: Some("char".into()),
            server_type: Some("char".into()),
            trans_type: None,
            kpd_type: None,
            in_trans: None,
            out_trans: None,
            destructor: None,
            bad_value: None,
            ool_number: 0,
        }
    }

    /// Native (PointerTo / ValueOf) C type
    pub fn native_type(c_type: String, is_pointer: bool, not_val: Option<String>) -> Self {
        IpcType {
            name: None,
            type_size: 0,
            pad_size: 0,
            min_type_size: 0,
            in_name: ipc_type_num::MACH_MSG_TYPE_INTEGER_32,
            out_name: ipc_type_num::MACH_MSG_TYPE_INTEGER_32,
            size: 32,
            number: 1,
            kpd_number: 0,
            in_line: true,
            mig_in_line: false,
            port_type: false,
            in_name_str: None,
            out_name_str: None,
            is_struct: false,
            is_string: false,
            var_array: false,
            no_opt_array: false,
            native: true,
            native_pointer: is_pointer,
            element: None,
            user_type: Some(c_type.clone()),
            server_type: Some(c_type),
            trans_type: None,
            kpd_type: None,
            in_trans: None,
            out_trans: None,
            destructor: None,
            bad_value: not_val,
            ool_number: 0,
        }
    }

    /// Clear translation-specific fields so the type can be used as a fresh base.
    #[allow(dead_code)]
    pub fn reset(&mut self) {
        self.in_trans = None;
        self.out_trans = None;
        self.destructor = None;
    }
}

/// Symbol table mapping identifier → IpcType
#[derive(Default)]
pub struct TypeTable {
    table: HashMap<String, IpcType>,
}

impl TypeTable {
    pub fn lookup(&self, name: &str) -> Option<&IpcType> {
        self.table.get(name)
    }

    pub fn insert(&mut self, name: String, mut ty: IpcType) {
        ty.name = Some(name.clone());
        self.table.insert(name, ty);
    }

    /// Seed the table with the truly primitive types — those that appear as
    /// RHS of `type X = Y` in std_types.defs / machine_types.defs *before*
    /// being declared themselves, i.e. bare C spellings that have no `type`
    /// declaration of their own in those files.
    ///
    /// Everything else (int8_t, uint32_t, mach_port_t, …) is defined by the
    /// .defs files and flows through TypeDecl registration at parse time.
    pub fn init_builtins(&mut self) {
        use ipc_type_num::*;
        // (name, in_wire_type, out_wire_type, bit_size)
        // Only bare C spellings that are used as type aliases before being
        // declared: `int`, `unsigned`, `char`, `boolean_t` appear in
        // machine_types.defs / std_types.defs on the RHS of other `type`
        // decls but are *also* declared there — so in practice the .defs
        // files define them in dependency order and we don't need to seed
        // anything.  However `kern_return_t = int` relies on `int` being
        // known, and `int` itself is `type int = int32_t` which depends on
        // `int32_t` — all defined in order.  The one true primitive we must
        // provide is nothing: MACH_MSG_TYPE_* constants are handled by the
        // lexer as Token::SymbolicType; `polymorphic` likewise.
        //
        // We do seed one alias that bootstrap_cmds historically treated as
        // primitive and that may appear before any .defs include: none.
        // (Left intentionally empty — the .defs files are self-sufficient.)
        let _ = (MACH_MSG_TYPE_INTEGER_32, PORT_SIZE); // suppress unused warnings
    }
}
