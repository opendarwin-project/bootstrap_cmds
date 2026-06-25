// SPDX-License-Identifier: MIT
// Lexer for .defs files using winnow

// winnow is used for its AsChar trait to drive the hand-written tokeniser.
use winnow::stream::AsChar;

use crate::types::ipc_type_num::*;

/// Byte offset of a token in the original source
pub type Span = std::ops::Range<usize>;

#[derive(Clone, Debug, PartialEq)]
pub enum Token {
    // keywords
    Routine,
    SimpleRoutine,
    Subsystem,
    MsgOption,
    UseSpecialReplyPort,
    ConsumeOnSendError,
    MsgSeqno,
    WaitTime,
    SendTime,
    NoWaitTime,
    NoSendTime,
    In,
    Out,
    UserImpl,
    ServerImpl,
    SecToken,
    ServerSecToken,
    UserSecToken,
    AuditToken,
    ServerAuditToken,
    UserAuditToken,
    ServerContextToken,
    InOut,
    RequestPort,
    ReplyPort,
    UReplyPort,
    SReplyPort,
    Array,
    Of,
    ErrorProc,
    ServerPrefix,
    UserPrefix,
    ServerDemux,
    RCSId,
    Import,
    UImport,
    SImport,
    DImport,
    IImport,
    Type,
    KernelServer,
    KernelUser,
    Skip,
    Struct,
    InTran,
    OutTran,
    Destructor,
    CType,
    CUserType,
    CServerType,
    CString,
    UserTypeLimit,
    OnStackLimit,
    PointerTo,
    PointerToIfNot,
    ValueOf,
    Novouchers,
    #[allow(dead_code)]
    MachMsg2,
    // IPC flags
    SameCount,
    RetCode,
    PhysicalCopy,
    Overwrite,
    Dealloc,
    NotDealloc,
    CountInOut,
    Auto,
    Const,
    #[allow(dead_code)]
    Polymorphic,
    // symbolic mach types
    SymbolicType {
        in_number: u32,
        in_str: String,
        out_number: u32,
        out_str: String,
        size: u32,
    },
    // punctuation
    Colon,
    Semi,
    Comma,
    Plus,
    Minus,
    Star,
    Div,
    LParen,
    RParen,
    Equal,
    Caret,
    Tilde,
    LAngle,
    RAngle,
    LBrack,
    RBrack,
    Bar,
    // literals
    Number(u32),
    Identifier(String),
    #[allow(dead_code)]
    StringLit(String), // bare unquoted string (path component)
    QStringLit(String), // "quoted"
    FileName(String),   // "quoted" or <angle>
    // errors
    LexError,
}

#[derive(Clone, Debug)]
pub struct Spanned {
    pub token: Token,
    pub span: Span,
}

// ---------------------------------------------------------------------------
// Low-level character helpers
// ---------------------------------------------------------------------------

fn is_ident_start(c: u8) -> bool {
    c.is_alpha() || c == b'_'
}

fn is_ident_continue(c: u8) -> bool {
    c.is_alphanum() || c == b'_'
}

#[allow(dead_code)]
fn is_string_char(c: u8) -> bool {
    // bare String token: [-/._$A-Za-z0-9]+
    c.is_alphanum() || matches!(c, b'-' | b'/' | b'.' | b'_' | b'$')
}

// ---------------------------------------------------------------------------
// Keyword / symbolic-type lookup (case-insensitive)
// ---------------------------------------------------------------------------

fn keyword_or_ident(s: &str) -> Token {
    match s.to_lowercase().as_str() {
        "routine" => Token::Routine,
        "simpleroutine" => Token::SimpleRoutine,
        "subsystem" => Token::Subsystem,
        "msgoption" => Token::MsgOption,
        "usespecialreplyport" => Token::UseSpecialReplyPort,
        "consumeonsenderror" => Token::ConsumeOnSendError,
        "msgseqno" => Token::MsgSeqno,
        "waittime" => Token::WaitTime,
        "sendtime" => Token::SendTime,
        "nowaittime" => Token::NoWaitTime,
        "nosendtime" => Token::NoSendTime,
        "in" => Token::In,
        "out" => Token::Out,
        "userimpl" => Token::UserImpl,
        "serverimpl" => Token::ServerImpl,
        "sectoken" => Token::SecToken,
        "serversectoken" => Token::ServerSecToken,
        "usersectoken" => Token::UserSecToken,
        "audittoken" => Token::AuditToken,
        "serveraudittoken" => Token::ServerAuditToken,
        "useraudittoken" => Token::UserAuditToken,
        "servercontexttoken" => Token::ServerContextToken,
        "inout" => Token::InOut,
        "requestport" => Token::RequestPort,
        "replyport" => Token::ReplyPort,
        "ureplyport" => Token::UReplyPort,
        "sreplyport" => Token::SReplyPort,
        "array" => Token::Array,
        "of" => Token::Of,
        "error" => Token::ErrorProc,
        "serverprefix" => Token::ServerPrefix,
        "userprefix" => Token::UserPrefix,
        "serverdemux" => Token::ServerDemux,
        "rcsid" => Token::RCSId,
        "import" => Token::Import,
        "uimport" => Token::UImport,
        "simport" => Token::SImport,
        "dimport" => Token::DImport,
        "iimport" => Token::IImport,
        "type" => Token::Type,
        "kernelserver" => Token::KernelServer,
        "kerneluser" => Token::KernelUser,
        "skip" => Token::Skip,
        "struct" => Token::Struct,
        "intran" => Token::InTran,
        "outtran" => Token::OutTran,
        "destructor" => Token::Destructor,
        "ctype" => Token::CType,
        "cusertype" => Token::CUserType,
        "cservertype" => Token::CServerType,
        "c_string" => Token::CString,
        "usertypelimit" => Token::UserTypeLimit,
        "onstacklimit" => Token::OnStackLimit,
        "novouchers" => Token::Novouchers,
        // IPC flags (case-insensitive in original)
        "samecount" => Token::SameCount,
        "retcode" => Token::RetCode,
        "physicalcopy" => Token::PhysicalCopy,
        "overwrite" => Token::Overwrite,
        "dealloc" => Token::Dealloc,
        "notdealloc" => Token::NotDealloc,
        "countinout" => Token::CountInOut,
        "polymorphic" => symbolic(
            MACH_MSG_TYPE_POLYMORPHIC,
            MACH_MSG_TYPE_POLYMORPHIC,
            PORT_SIZE,
        ),
        "auto" => Token::Auto,
        "const" => Token::Const,
        // case-SENSITIVE
        _ => Token::Identifier(s.to_owned()),
    }
}

fn symbolic(inn: u32, out: u32, size: u32) -> Token {
    Token::SymbolicType {
        in_number: inn,
        in_str: format!("{inn}"),
        out_number: out,
        out_str: format!("{out}"),
        size,
    }
}

fn case_sensitive_keyword(s: &str) -> Option<Token> {
    Some(match s {
        "PointerTo" => Token::PointerTo,
        "PointerToIfNot" => Token::PointerToIfNot,
        "ValueOf" => Token::ValueOf,
        "MACH_MSG_TYPE_UNSTRUCTURED" => {
            symbolic(MACH_MSG_TYPE_UNSTRUCTURED, MACH_MSG_TYPE_UNSTRUCTURED, 0)
        }
        "MACH_MSG_TYPE_BIT" => symbolic(MACH_MSG_TYPE_BIT, MACH_MSG_TYPE_BIT, 1),
        "MACH_MSG_TYPE_BOOLEAN" => symbolic(MACH_MSG_TYPE_BOOLEAN, MACH_MSG_TYPE_BOOLEAN, 32),
        "MACH_MSG_TYPE_INTEGER_8" => symbolic(MACH_MSG_TYPE_INTEGER_8, MACH_MSG_TYPE_INTEGER_8, 8),
        "MACH_MSG_TYPE_INTEGER_16" => {
            symbolic(MACH_MSG_TYPE_INTEGER_16, MACH_MSG_TYPE_INTEGER_16, 16)
        }
        "MACH_MSG_TYPE_INTEGER_32" => {
            symbolic(MACH_MSG_TYPE_INTEGER_32, MACH_MSG_TYPE_INTEGER_32, 32)
        }
        "MACH_MSG_TYPE_INTEGER_64" => {
            symbolic(MACH_MSG_TYPE_INTEGER_64, MACH_MSG_TYPE_INTEGER_64, 64)
        }
        "MACH_MSG_TYPE_REAL_32" => symbolic(MACH_MSG_TYPE_REAL_32, MACH_MSG_TYPE_REAL_32, 32),
        "MACH_MSG_TYPE_REAL_64" => symbolic(MACH_MSG_TYPE_REAL_64, MACH_MSG_TYPE_REAL_64, 64),
        "MACH_MSG_TYPE_CHAR" => symbolic(MACH_MSG_TYPE_CHAR, MACH_MSG_TYPE_CHAR, 8),
        "MACH_MSG_TYPE_BYTE" => symbolic(MACH_MSG_TYPE_BYTE, MACH_MSG_TYPE_BYTE, 8),
        "MACH_MSG_TYPE_MOVE_RECEIVE" => symbolic(
            MACH_MSG_TYPE_MOVE_RECEIVE,
            MACH_MSG_TYPE_PORT_RECEIVE,
            PORT_SIZE,
        ),
        "MACH_MSG_TYPE_COPY_SEND" => {
            symbolic(MACH_MSG_TYPE_COPY_SEND, MACH_MSG_TYPE_PORT_SEND, PORT_SIZE)
        }
        "MACH_MSG_TYPE_MAKE_SEND" => {
            symbolic(MACH_MSG_TYPE_MAKE_SEND, MACH_MSG_TYPE_PORT_SEND, PORT_SIZE)
        }
        "MACH_MSG_TYPE_MOVE_SEND" => {
            symbolic(MACH_MSG_TYPE_MOVE_SEND, MACH_MSG_TYPE_PORT_SEND, PORT_SIZE)
        }
        "MACH_MSG_TYPE_MAKE_SEND_ONCE" => symbolic(
            MACH_MSG_TYPE_MAKE_SEND_ONCE,
            MACH_MSG_TYPE_PORT_SEND_ONCE,
            PORT_SIZE,
        ),
        "MACH_MSG_TYPE_MOVE_SEND_ONCE" => symbolic(
            MACH_MSG_TYPE_MOVE_SEND_ONCE,
            MACH_MSG_TYPE_PORT_SEND_ONCE,
            PORT_SIZE,
        ),
        "MACH_MSG_TYPE_PORT_NAME" => {
            symbolic(MACH_MSG_TYPE_PORT_NAME, MACH_MSG_TYPE_PORT_NAME, PORT_SIZE)
        }
        "MACH_MSG_TYPE_PORT_RECEIVE" => symbolic(
            MACH_MSG_TYPE_POLYMORPHIC,
            MACH_MSG_TYPE_PORT_RECEIVE,
            PORT_SIZE,
        ),
        "MACH_MSG_TYPE_PORT_SEND" => symbolic(
            MACH_MSG_TYPE_POLYMORPHIC,
            MACH_MSG_TYPE_PORT_SEND,
            PORT_SIZE,
        ),
        "MACH_MSG_TYPE_PORT_SEND_ONCE" => symbolic(
            MACH_MSG_TYPE_POLYMORPHIC,
            MACH_MSG_TYPE_PORT_SEND_ONCE,
            PORT_SIZE,
        ),
        "MACH_MSG_TYPE_POLYMORPHIC" => symbolic(
            MACH_MSG_TYPE_POLYMORPHIC,
            MACH_MSG_TYPE_POLYMORPHIC,
            PORT_SIZE,
        ),
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// Main tokeniser
// ---------------------------------------------------------------------------

/// Tokenise a complete pre-processed .defs source into a vec of Spanned tokens.
pub fn tokenise(src: &str) -> Vec<Spanned> {
    let bytes = src.as_bytes();
    let len = bytes.len();
    let mut pos = 0usize;
    let mut tokens = Vec::new();

    macro_rules! push {
        ($start:expr, $tok:expr) => {
            tokens.push(Spanned {
                token: $tok,
                span: $start..pos,
            });
        };
    }

    while pos < len {
        // Skip whitespace
        while pos < len
            && (bytes[pos] == b' '
                || bytes[pos] == b'\t'
                || bytes[pos] == b'\n'
                || bytes[pos] == b'\r')
        {
            pos += 1;
        }
        if pos >= len {
            break;
        }

        let start = pos;
        let ch = bytes[pos];

        // C-style block comment
        if ch == b'/' && pos + 1 < len && bytes[pos + 1] == b'*' {
            pos += 2;
            while pos + 1 < len && !(bytes[pos] == b'*' && bytes[pos + 1] == b'/') {
                pos += 1;
            }
            pos += 2; // consume */
            continue;
        }

        // C++ line comment
        if ch == b'/' && pos + 1 < len && bytes[pos + 1] == b'/' {
            while pos < len && bytes[pos] != b'\n' {
                pos += 1;
            }
            continue;
        }

        // cpp # directives — skip to end of line (pre-processor already ran)
        if ch == b'#' {
            while pos < len && bytes[pos] != b'\n' {
                pos += 1;
            }
            continue;
        }

        // Quoted string "..."
        if ch == b'"' {
            pos += 1;
            while pos < len && bytes[pos] != b'"' && bytes[pos] != b'\n' {
                pos += 1;
            }
            if pos < len {
                pos += 1;
            } // consume closing "
            let content = &src[start + 1..pos - 1];
            push!(start, Token::QStringLit(content.to_owned()));
            continue;
        }

        // Angle-bracket filename <...>
        if ch == b'<' {
            // Could be filename <foo/bar.h> or LAngle; peek ahead
            let mut j = pos + 1;
            let _valid = true;
            while j < len && bytes[j] != b'>' && bytes[j] != b'\n' {
                j += 1;
            }
            if j < len && bytes[j] == b'>' {
                // looks like a filename
                let content = &src[pos + 1..j];
                pos = j + 1;
                push!(start, Token::FileName(format!("<{content}>")));
                continue;
            }
            // Otherwise treat as LAngle below
        }

        // Numbers
        if ch.is_ascii_digit() {
            while pos < len && bytes[pos].is_ascii_digit() {
                pos += 1;
            }
            let n: u32 = src[start..pos].parse().unwrap_or(0);
            push!(start, Token::Number(n));
            continue;
        }

        // Identifiers / keywords
        if is_ident_start(ch) {
            while pos < len && is_ident_continue(bytes[pos]) {
                pos += 1;
            }
            let word = &src[start..pos];
            // case-sensitive MACH_ and Pointer*/ValueOf check first
            let tok = case_sensitive_keyword(word).unwrap_or_else(|| keyword_or_ident(word));
            push!(start, tok);
            continue;
        }

        // Single-char punctuation
        pos += 1;
        let tok = match ch {
            b':' => Token::Colon,
            b';' => Token::Semi,
            b',' => Token::Comma,
            b'+' => Token::Plus,
            b'-' => Token::Minus,
            b'*' => Token::Star,
            b'/' => Token::Div,
            b'(' => Token::LParen,
            b')' => Token::RParen,
            b'=' => Token::Equal,
            b'^' => Token::Caret,
            b'~' => Token::Tilde,
            b'<' => Token::LAngle,
            b'>' => Token::RAngle,
            b'[' => Token::LBrack,
            b']' => Token::RBrack,
            b'|' => Token::Bar,
            _ => Token::LexError,
        };
        push!(start, tok);
    }

    tokens
}
