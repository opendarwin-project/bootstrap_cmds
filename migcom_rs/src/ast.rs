// SPDX-License-Identifier: MIT
// AST node types produced by the parser

use crate::types::{IpcFlags, IpcType};

// ---------------------------------------------------------------------------
// Argument direction / kind
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    None,
    In,
    Out,
    InOut,
    RequestPort,
    ReplyPort,
    SReplyPort,
    UReplyPort,
    WaitTime,
    SendTime,
    MsgOption,
    SecToken,
    ServerSecToken,
    UserSecToken,
    AuditToken,
    ServerAuditToken,
    UserAuditToken,
    ServerContextToken,
    MsgSeqno,
    ServerImpl,
    UserImpl,
}

// ---------------------------------------------------------------------------
// Routine argument
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct Argument {
    pub name: String,
    pub direction: Direction,
    pub ty: IpcType,
    pub flags: IpcFlags,
}

// ---------------------------------------------------------------------------
// Routine / SimpleRoutine
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoutineKind {
    Routine,
    SimpleRoutine,
}

#[derive(Clone, Debug)]
pub struct Routine {
    pub kind: RoutineKind,
    pub name: String,
    pub args: Vec<Argument>,
}

// ---------------------------------------------------------------------------
// Import kind
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImportKind {
    Import,
    UImport,
    SImport,
    IImport,
    DImport,
}

// ---------------------------------------------------------------------------
// Top-level statements
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub enum Statement {
    Subsystem {
        name: String,
        base: u32,
        is_kernel_user: bool,
        is_kernel_server: bool,
    },
    WaitTime(String),
    NoWaitTime,
    SendTime(String),
    NoSendTime,
    MsgOption(Option<String>),
    UseSpecialReplyPort(bool),
    ConsumeOnSendError(ConsumeOnSendError),
    UserTypeLimit(u32),
    OnStackLimit(u32),
    ErrorProc(String),
    ServerPrefix(String),
    UserPrefix(String),
    ServerDemux(String),
    TypeDecl {
        name: String,
        ty: IpcType,
    },
    Routine(Routine),
    Skip,
    Import {
        kind: ImportKind,
        filename: String,
    },
    RCSDecl(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConsumeOnSendError {
    None,
    Timeout,
    Any,
}
