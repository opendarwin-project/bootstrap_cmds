// SPDX-License-Identifier: MIT
// Recursive-descent parser for .defs files

use crate::ast::*;
use crate::diag::Diag;
use crate::lexer::{Spanned, Token};
use crate::types::{IpcFlags, IpcType, TypeTable};

pub struct Parser<'src> {
    tokens: &'src [Spanned],
    pos: usize,
    pub diag: &'src mut Diag,
    pub types: TypeTable,
}

impl<'src> Parser<'src> {
    pub fn new(tokens: &'src [Spanned], diag: &'src mut Diag) -> Self {
        let mut types = TypeTable::default();
        types.init_builtins();
        Self {
            tokens,
            pos: 0,
            diag,
            types,
        }
    }

    // -----------------------------------------------------------------------
    // Token stream helpers
    // -----------------------------------------------------------------------

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos).map(|s| &s.token)
    }

    fn peek_span(&self) -> std::ops::Range<usize> {
        self.tokens
            .get(self.pos)
            .map(|s| s.span.clone())
            .unwrap_or(0..0)
    }

    #[allow(dead_code)]
    fn advance(&mut self) -> Option<&Spanned> {
        let t = self.tokens.get(self.pos);
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    fn at_end(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    fn expect(&mut self, expected: &Token) -> bool {
        if self.peek() == Some(expected) {
            self.pos += 1;
            true
        } else {
            let sp = self.peek_span();
            let got = self
                .peek()
                .map(|t| format!("{t:?}"))
                .unwrap_or_else(|| "EOF".into());
            self.diag
                .error(sp, &format!("expected {expected:?}, got {got}"));
            false
        }
    }

    fn expect_identifier(&mut self) -> Option<String> {
        match self.peek() {
            Some(Token::Identifier(s)) => {
                let s = s.clone();
                self.pos += 1;
                Some(s)
            }
            _ => {
                let sp = self.peek_span();
                self.diag.error(sp, "expected identifier");
                None
            }
        }
    }

    fn expect_number(&mut self) -> Option<u32> {
        match self.peek() {
            Some(Token::Number(n)) => {
                let n = *n;
                self.pos += 1;
                Some(n)
            }
            _ => {
                let sp = self.peek_span();
                self.diag.error(sp, "expected number");
                None
            }
        }
    }

    fn expect_string(&mut self) -> Option<String> {
        match self.peek() {
            Some(Token::StringLit(s) | Token::QStringLit(s)) => {
                let s = s.clone();
                self.pos += 1;
                Some(s)
            }
            Some(Token::Identifier(s)) => {
                let s = s.clone();
                self.pos += 1;
                Some(s)
            }
            _ => {
                let sp = self.peek_span();
                self.diag.error(sp, "expected string");
                None
            }
        }
    }

    fn expect_filename(&mut self) -> Option<String> {
        match self.peek() {
            Some(Token::FileName(s) | Token::QStringLit(s)) => {
                let s = s.clone();
                self.pos += 1;
                Some(s)
            }
            _ => {
                let sp = self.peek_span();
                self.diag.error(sp, "expected filename");
                None
            }
        }
    }

    fn expect_qstring(&mut self) -> Option<String> {
        match self.peek() {
            Some(Token::QStringLit(s)) => {
                let s = s.clone();
                self.pos += 1;
                Some(s)
            }
            _ => {
                let sp = self.peek_span();
                self.diag.error(sp, "expected quoted string");
                None
            }
        }
    }

    // Skip to next semicolon for error recovery
    fn sync_to_semi(&mut self) {
        while !self.at_end() {
            if self.peek() == Some(&Token::Semi) {
                self.pos += 1;
                break;
            }
            self.pos += 1;
        }
    }

    // -----------------------------------------------------------------------
    // Top-level parse
    // -----------------------------------------------------------------------

    pub fn parse_statements(&mut self) -> Vec<Statement> {
        let mut stmts = Vec::new();
        while !self.at_end() {
            if self.peek() == Some(&Token::Semi) {
                self.pos += 1;
                continue;
            }
            match self.parse_statement() {
                Some(s) => stmts.push(s),
                None => self.sync_to_semi(),
            }
        }
        stmts
    }

    fn parse_statement(&mut self) -> Option<Statement> {
        let sp = self.peek_span();
        let stmt = match self.peek()? {
            Token::Subsystem => self.parse_subsystem(),
            Token::WaitTime => self.parse_waittime(),
            Token::SendTime => self.parse_sendtime(),
            Token::NoWaitTime => {
                self.pos += 1;
                Some(Statement::NoWaitTime)
            }
            Token::NoSendTime => {
                self.pos += 1;
                Some(Statement::NoSendTime)
            }
            Token::MsgOption => self.parse_msgoption(),
            Token::UseSpecialReplyPort => self.parse_use_special_reply(),
            Token::ConsumeOnSendError => self.parse_consume_on_send_error(),
            Token::UserTypeLimit => {
                self.pos += 1;
                let n = self.expect_number()?;
                Some(Statement::UserTypeLimit(n))
            }
            Token::OnStackLimit => {
                self.pos += 1;
                let n = self.expect_number()?;
                Some(Statement::OnStackLimit(n))
            }
            Token::ErrorProc => {
                self.pos += 1;
                let id = self.expect_identifier()?;
                Some(Statement::ErrorProc(id))
            }
            Token::ServerPrefix => {
                self.pos += 1;
                let id = self.expect_identifier()?;
                Some(Statement::ServerPrefix(id))
            }
            Token::UserPrefix => {
                self.pos += 1;
                let id = self.expect_identifier()?;
                Some(Statement::UserPrefix(id))
            }
            Token::ServerDemux => {
                self.pos += 1;
                let id = self.expect_identifier()?;
                Some(Statement::ServerDemux(id))
            }
            Token::Type => self.parse_type_decl(),
            Token::Routine | Token::SimpleRoutine => self.parse_routine_decl(),
            Token::Skip => {
                self.pos += 1;
                Some(Statement::Skip)
            }
            Token::Import | Token::UImport | Token::SImport | Token::IImport | Token::DImport => {
                self.parse_import()
            }
            Token::RCSId => self.parse_rcsdecl(),
            other => {
                let msg = format!("unexpected token {other:?}");
                self.diag.error(sp, &msg);
                return None;
            }
        }?;
        self.expect(&Token::Semi);
        Some(stmt)
    }

    // -----------------------------------------------------------------------
    // Individual statement parsers
    // -----------------------------------------------------------------------

    fn parse_subsystem(&mut self) -> Option<Statement> {
        self.pos += 1; // consume `subsystem`
        let mut is_kernel_user = false;
        let mut is_kernel_server = false;
        // optional modifiers
        loop {
            match self.peek() {
                Some(Token::KernelUser) => {
                    is_kernel_user = true;
                    self.pos += 1;
                }
                Some(Token::KernelServer) => {
                    is_kernel_server = true;
                    self.pos += 1;
                }
                _ => break,
            }
        }
        let name = self.expect_identifier()?;
        let base = self.expect_number()?;
        Some(Statement::Subsystem {
            name,
            base,
            is_kernel_user,
            is_kernel_server,
        })
    }

    fn parse_waittime(&mut self) -> Option<Statement> {
        self.pos += 1;
        let s = self.expect_string()?;
        Some(Statement::WaitTime(s))
    }

    fn parse_sendtime(&mut self) -> Option<Statement> {
        self.pos += 1;
        let s = self.expect_string()?;
        Some(Statement::SendTime(s))
    }

    fn parse_msgoption(&mut self) -> Option<Statement> {
        self.pos += 1;
        let s = self.expect_string()?;
        if s == "MACH_MSG_OPTION_NONE" {
            Some(Statement::MsgOption(None))
        } else {
            Some(Statement::MsgOption(Some(s)))
        }
    }

    fn parse_use_special_reply(&mut self) -> Option<Statement> {
        self.pos += 1;
        let n = self.expect_number()?;
        Some(Statement::UseSpecialReplyPort(n != 0))
    }

    fn parse_consume_on_send_error(&mut self) -> Option<Statement> {
        self.pos += 1;
        let s = self.expect_string()?;
        let val = match s.to_lowercase().as_str() {
            "none" => ConsumeOnSendError::None,
            "timeout" => ConsumeOnSendError::Timeout,
            "any" => ConsumeOnSendError::Any,
            _ => {
                let sp = self.peek_span();
                self.diag.error(sp, "invalid ConsumeOnSendError value");
                return None;
            }
        };
        Some(Statement::ConsumeOnSendError(val))
    }

    fn parse_type_decl(&mut self) -> Option<Statement> {
        self.pos += 1; // consume `type`
        let name = self.expect_identifier()?;
        self.expect(&Token::Equal);
        let mut ty = self.parse_trans_type_spec()?;
        // Only set user_type to the alias name when it hasn't already been
        // explicitly provided by a `cusertype:` modifier. This preserves e.g.
        // `semaphore_consume_ref_t = mach_port_move_send_t cusertype: semaphore_t`
        // so that prototypes use `semaphore_t` (visible in kernel context) instead of
        // the consume-ref alias name which is not defined in kernel headers.
        if ty.user_type.is_none() {
            ty.user_type = Some(name.clone());
        }
        if ty.server_type.is_none() {
            ty.server_type = Some(name.clone());
        }
        // Register immediately so forward uses within the same file work.
        let mut named = ty.clone();
        named.name = Some(name.clone());
        self.types.insert(name.clone(), named);
        Some(Statement::TypeDecl { name, ty })
    }

    fn parse_import(&mut self) -> Option<Statement> {
        let kind = match self.peek()? {
            Token::Import => ImportKind::Import,
            Token::UImport => ImportKind::UImport,
            Token::SImport => ImportKind::SImport,
            Token::IImport => ImportKind::IImport,
            Token::DImport => ImportKind::DImport,
            _ => unreachable!(),
        };
        self.pos += 1;
        let filename = self.expect_filename()?;
        Some(Statement::Import { kind, filename })
    }

    fn parse_rcsdecl(&mut self) -> Option<Statement> {
        self.pos += 1; // consume `RCSId`
        let s = self.expect_qstring()?;
        Some(Statement::RCSDecl(s))
    }

    // -----------------------------------------------------------------------
    // Routine parsing
    // -----------------------------------------------------------------------

    fn parse_routine_decl(&mut self) -> Option<Statement> {
        let kind = match self.peek()? {
            Token::Routine => RoutineKind::Routine,
            Token::SimpleRoutine => RoutineKind::SimpleRoutine,
            _ => unreachable!(),
        };
        self.pos += 1;
        let name = self.expect_identifier()?;
        let args = self.parse_arguments()?;
        Some(Statement::Routine(Routine { kind, name, args }))
    }

    fn parse_arguments(&mut self) -> Option<Vec<Argument>> {
        self.expect(&Token::LParen);
        if self.peek() == Some(&Token::RParen) {
            self.pos += 1;
            return Some(vec![]);
        }
        let list = self.parse_argument_list()?;
        self.expect(&Token::RParen);
        Some(list)
    }

    fn parse_argument_list(&mut self) -> Option<Vec<Argument>> {
        let mut args = Vec::new();
        loop {
            // On error inside an argument, skip to the next `;` or `)` and keep going
            // so one bad type reference doesn't kill the whole routine.
            match self.parse_single_argument() {
                Some(arg) => args.push(arg),
                None => {
                    // consume up to `;` or `)`
                    while !self.at_end() {
                        match self.peek() {
                            Some(Token::Semi) | Some(Token::RParen) => break,
                            _ => {
                                self.pos += 1;
                            }
                        }
                    }
                }
            }
            if self.peek() == Some(&Token::Semi) {
                self.pos += 1;
                if self.peek() == Some(&Token::RParen) {
                    break;
                }
            } else {
                break;
            }
        }
        Some(args)
    }

    fn parse_single_argument(&mut self) -> Option<Argument> {
        // In the MIG grammar an argument is either:
        //   Direction Identifier : Type [Flags]    — normal
        //   DirectionKeyword : Type                — keyword doubles as name (e.g. requestPort)
        //   TrImplKeyword Identifier : Type        — trailer (ServerImpl / UserImpl)
        //
        // We disambiguate by peeking at token[pos+1]: if it is `:` the direction
        // keyword is being used as both direction AND name.
        let (direction, name) = self.parse_direction_and_name()?;
        self.expect(&Token::Colon);
        let ty = self.parse_argument_type()?;
        let flags = self.parse_ipc_flags();
        Some(Argument {
            name,
            direction,
            ty,
            flags,
        })
    }

    /// Returns (direction, name) for one argument, handling the dual-role keywords.
    fn parse_direction_and_name(&mut self) -> Option<(Direction, String)> {
        // Keyword directions that can double as the argument name (no separate identifier follows):
        // requestPort, replyPort, sReplyPort, uReplyPort, waitTime, sendTime, msgOption,
        // secToken, serverSecToken, userSecToken, auditToken, serverAuditToken,
        // userAuditToken, serverContextToken, msgSeqno, serverImpl, userImpl.
        //
        // `in`, `out`, `inout` always take a separate identifier after them.
        let next_is_colon = self
            .tokens
            .get(self.pos + 1)
            .map(|s| s.token == Token::Colon)
            .unwrap_or(false);

        match self.peek() {
            // Pure direction prefixes — always followed by a separate identifier
            Some(Token::In) => {
                self.pos += 1;
                let n = self.expect_identifier()?;
                Some((Direction::In, n))
            }
            Some(Token::Out) => {
                self.pos += 1;
                let n = self.expect_identifier()?;
                Some((Direction::Out, n))
            }
            Some(Token::InOut) => {
                self.pos += 1;
                let n = self.expect_identifier()?;
                Some((Direction::InOut, n))
            }

            // Dual-role direction keywords: if next token is `:`, the keyword IS the name
            Some(Token::RequestPort) => {
                self.pos += 1;
                if next_is_colon {
                    Some((Direction::RequestPort, "requestPort".into()))
                } else {
                    let n = self.expect_identifier()?;
                    Some((Direction::RequestPort, n))
                }
            }
            Some(Token::ReplyPort) => {
                self.pos += 1;
                if next_is_colon {
                    Some((Direction::ReplyPort, "replyPort".into()))
                } else {
                    let n = self.expect_identifier()?;
                    Some((Direction::ReplyPort, n))
                }
            }
            Some(Token::SReplyPort) => {
                self.pos += 1;
                if next_is_colon {
                    Some((Direction::SReplyPort, "sReplyPort".into()))
                } else {
                    let n = self.expect_identifier()?;
                    Some((Direction::SReplyPort, n))
                }
            }
            Some(Token::UReplyPort) => {
                self.pos += 1;
                if next_is_colon {
                    Some((Direction::UReplyPort, "uReplyPort".into()))
                } else {
                    let n = self.expect_identifier()?;
                    Some((Direction::UReplyPort, n))
                }
            }
            Some(Token::WaitTime) => {
                self.pos += 1;
                if next_is_colon {
                    Some((Direction::WaitTime, "waitTime".into()))
                } else {
                    let n = self.expect_identifier()?;
                    Some((Direction::WaitTime, n))
                }
            }
            Some(Token::SendTime) => {
                self.pos += 1;
                if next_is_colon {
                    Some((Direction::SendTime, "sendTime".into()))
                } else {
                    let n = self.expect_identifier()?;
                    Some((Direction::SendTime, n))
                }
            }
            Some(Token::MsgOption) => {
                self.pos += 1;
                if next_is_colon {
                    Some((Direction::MsgOption, "msgOption".into()))
                } else {
                    let n = self.expect_identifier()?;
                    Some((Direction::MsgOption, n))
                }
            }
            Some(Token::SecToken) => {
                self.pos += 1;
                if next_is_colon {
                    Some((Direction::SecToken, "secToken".into()))
                } else {
                    let n = self.expect_identifier()?;
                    Some((Direction::SecToken, n))
                }
            }
            Some(Token::ServerSecToken) => {
                self.pos += 1;
                if next_is_colon {
                    Some((Direction::ServerSecToken, "serverSecToken".into()))
                } else {
                    let n = self.expect_identifier()?;
                    Some((Direction::ServerSecToken, n))
                }
            }
            Some(Token::UserSecToken) => {
                self.pos += 1;
                if next_is_colon {
                    Some((Direction::UserSecToken, "userSecToken".into()))
                } else {
                    let n = self.expect_identifier()?;
                    Some((Direction::UserSecToken, n))
                }
            }
            Some(Token::AuditToken) => {
                self.pos += 1;
                if next_is_colon {
                    Some((Direction::AuditToken, "auditToken".into()))
                } else {
                    let n = self.expect_identifier()?;
                    Some((Direction::AuditToken, n))
                }
            }
            Some(Token::ServerAuditToken) => {
                self.pos += 1;
                if next_is_colon {
                    Some((Direction::ServerAuditToken, "serverAuditToken".into()))
                } else {
                    let n = self.expect_identifier()?;
                    Some((Direction::ServerAuditToken, n))
                }
            }
            Some(Token::UserAuditToken) => {
                self.pos += 1;
                if next_is_colon {
                    Some((Direction::UserAuditToken, "userAuditToken".into()))
                } else {
                    let n = self.expect_identifier()?;
                    Some((Direction::UserAuditToken, n))
                }
            }
            Some(Token::ServerContextToken) => {
                self.pos += 1;
                if next_is_colon {
                    Some((Direction::ServerContextToken, "serverContextToken".into()))
                } else {
                    let n = self.expect_identifier()?;
                    Some((Direction::ServerContextToken, n))
                }
            }
            Some(Token::MsgSeqno) => {
                self.pos += 1;
                if next_is_colon {
                    Some((Direction::MsgSeqno, "msgSeqno".into()))
                } else {
                    let n = self.expect_identifier()?;
                    Some((Direction::MsgSeqno, n))
                }
            }
            Some(Token::ServerImpl) => {
                self.pos += 1;
                if next_is_colon {
                    Some((Direction::ServerImpl, "serverImpl".into()))
                } else {
                    let n = self.expect_identifier()?;
                    Some((Direction::ServerImpl, n))
                }
            }
            Some(Token::UserImpl) => {
                self.pos += 1;
                if next_is_colon {
                    Some((Direction::UserImpl, "userImpl".into()))
                } else {
                    let n = self.expect_identifier()?;
                    Some((Direction::UserImpl, n))
                }
            }
            // No direction keyword — plain identifier
            _ => {
                let n = self.expect_identifier()?;
                Some((Direction::None, n))
            }
        }
    }

    fn parse_argument_type(&mut self) -> Option<IpcType> {
        // Grammar (from original parser.y):
        //   ArgumentType : ':' Identifier
        //                | ':' NamedTypeSpec          (Identifier '=' TransTypeSpec)
        //                | ':' NativeTypeSpec
        //                | ':' BasicTypeSpec           (SymbolicType or number)
        //
        // The colon has already been consumed by the caller.
        match self.peek() {
            Some(Token::Identifier(_)) => {
                // Peek ahead: if next-next is `=` it's an inline NamedTypeSpec
                // (e.g.  arg : existing_type_name = MACH_MSG_TYPE_MOVE_SEND ctype: mach_port_t)
                let is_named_spec = self
                    .tokens
                    .get(self.pos + 1)
                    .map(|s| s.token == Token::Equal)
                    .unwrap_or(false);
                if is_named_spec {
                    // Parse as NamedTypeSpec: consume the name, then `=`, then TransTypeSpec
                    return self.parse_trans_type_spec();
                }
                let name = self.expect_identifier()?;
                if let Some(ty) = self.types.lookup(&name) {
                    Some(ty.clone())
                } else {
                    let sp = self.peek_span();
                    self.diag.warn(
                        sp,
                        &format!("undefined type '{name}' — using integer_32 placeholder"),
                    );
                    // Return a placeholder so we keep parsing the rest of the routine.
                    Some(IpcType::short_decl(
                        crate::types::ipc_type_num::MACH_MSG_TYPE_INTEGER_32,
                        Some(name.clone()),
                        crate::types::ipc_type_num::MACH_MSG_TYPE_INTEGER_32,
                        Some(name),
                        32,
                    ))
                }
            }
            _ => self.parse_named_type_spec(),
        }
    }

    fn parse_ipc_flags(&mut self) -> IpcFlags {
        let mut flags = IpcFlags::NONE;
        while self.peek() == Some(&Token::Comma) {
            self.pos += 1;
            let f = match self.peek() {
                Some(Token::SameCount) => IpcFlags::SAME_COUNT,
                Some(Token::RetCode) => IpcFlags::RET_CODE,
                Some(Token::PhysicalCopy) => IpcFlags::PHYSICAL_COPY,
                Some(Token::Overwrite) => IpcFlags::OVERWRITE,
                Some(Token::Dealloc) => {
                    self.pos += 1;
                    // Dealloc[] → MAYBE_DEALLOC
                    if self.peek() == Some(&Token::LBrack) {
                        self.pos += 1;
                        self.expect(&Token::RBrack);
                        flags |= IpcFlags::MAYBE_DEALLOC;
                        continue;
                    }
                    flags |= IpcFlags::DEALLOC;
                    continue;
                }
                Some(Token::NotDealloc) => IpcFlags::NOT_DEALLOC,
                Some(Token::CountInOut) => IpcFlags::COUNT_IN_OUT,
                Some(Token::Auto) => IpcFlags::AUTO,
                Some(Token::Const) => IpcFlags::CONST,
                _ => break,
            };
            self.pos += 1;
            flags |= f;
        }
        flags
    }

    // -----------------------------------------------------------------------
    // Type spec parsing
    // -----------------------------------------------------------------------

    fn parse_trans_type_spec(&mut self) -> Option<IpcType> {
        let mut ty = self.parse_type_spec()?;

        loop {
            match self.peek() {
                Some(Token::InTran) => {
                    self.pos += 1;
                    self.expect(&Token::Colon);
                    let trans = self.expect_identifier()?;
                    let func = self.expect_identifier()?;
                    self.expect(&Token::LParen);
                    let stype = self.expect_identifier()?;
                    self.expect(&Token::RParen);
                    ty.trans_type = Some(trans);
                    ty.in_trans = Some(func);
                    ty.server_type = Some(stype);
                }
                Some(Token::OutTran) => {
                    self.pos += 1;
                    self.expect(&Token::Colon);
                    let stype = self.expect_identifier()?;
                    let func = self.expect_identifier()?;
                    self.expect(&Token::LParen);
                    let trans = self.expect_identifier()?;
                    self.expect(&Token::RParen);
                    ty.server_type = Some(stype);
                    ty.out_trans = Some(func);
                    ty.trans_type = Some(trans);
                }
                Some(Token::Destructor) => {
                    self.pos += 1;
                    self.expect(&Token::Colon);
                    let func = self.expect_identifier()?;
                    self.expect(&Token::LParen);
                    let trans = self.expect_identifier()?;
                    self.expect(&Token::RParen);
                    ty.destructor = Some(func);
                    ty.trans_type = Some(trans);
                }
                Some(Token::CType) => {
                    self.pos += 1;
                    self.expect(&Token::Colon);
                    let name = self.expect_identifier()?;
                    ty.user_type = Some(name.clone());
                    ty.server_type = Some(name);
                }
                Some(Token::CUserType) => {
                    self.pos += 1;
                    self.expect(&Token::Colon);
                    let name = self.expect_identifier()?;
                    ty.user_type = Some(name);
                }
                Some(Token::CServerType) => {
                    self.pos += 1;
                    self.expect(&Token::Colon);
                    let name = self.expect_identifier()?;
                    ty.server_type = Some(name);
                }
                _ => break,
            }
        }
        Some(ty)
    }

    fn parse_type_spec(&mut self) -> Option<IpcType> {
        match self.peek()? {
            Token::Array => self.parse_array_spec(),
            Token::Caret => {
                self.pos += 1;
                let inner = self.parse_type_spec()?;
                Some(IpcType::ptr_decl(inner))
            }
            Token::Struct => self.parse_struct_spec(),
            Token::CString => self.parse_cstring_spec(),
            Token::PointerTo | Token::PointerToIfNot | Token::ValueOf => {
                self.parse_native_type_spec()
            }
            Token::LParen => {
                // Long-form type — no longer supported; consume and error
                let sp = self.peek_span();
                self.diag
                    .error(sp, "long-form type declarations are no longer supported");
                self.sync_to_semi();
                None
            }
            Token::Identifier(_) => {
                // NamedTypeSpec: Identifier '=' TransTypeSpec
                // PrevTypeSpec:  Identifier (look up in table)
                let is_named = self
                    .tokens
                    .get(self.pos + 1)
                    .map(|s| s.token == Token::Equal)
                    .unwrap_or(false);
                if is_named {
                    let name = self.expect_identifier()?;
                    self.expect(&Token::Equal);
                    let mut ty = self.parse_trans_type_spec()?;
                    ty.name = Some(name.clone());
                    self.types.insert(name, ty.clone());
                    return Some(ty);
                }
                let name = self.expect_identifier()?;
                if let Some(ty) = self.types.lookup(&name) {
                    Some(ty.clone())
                } else {
                    let sp = self.peek_span();
                    self.diag.warn(
                        sp,
                        &format!("undefined type '{name}' — using integer_32 placeholder"),
                    );
                    Some(IpcType::short_decl(
                        crate::types::ipc_type_num::MACH_MSG_TYPE_INTEGER_32,
                        Some(name.clone()),
                        crate::types::ipc_type_num::MACH_MSG_TYPE_INTEGER_32,
                        Some(name),
                        32,
                    ))
                }
            }
            Token::SymbolicType { .. } | Token::Number(_) => self.parse_basic_type_spec(),
            _ => {
                let sp = self.peek_span();
                self.diag.error(sp, "expected type spec");
                None
            }
        }
    }

    fn parse_basic_type_spec(&mut self) -> Option<IpcType> {
        let (inn, in_s, outn, out_s, size) = self.parse_ipc_type()?;
        let mut ty = IpcType::short_decl(inn, in_s, outn, out_s, size);
        // Set the real C type based on the IPC wire type.
        // MACH_MSG_TYPE_PORT_NAME is a port *name* (unsigned integer), not a
        // port object — it maps to mach_port_name_t, not mach_port_t, and must
        // NOT be flagged as port_type so that write_type_aliases doesn't emit a
        // conflicting `typedef mach_port_t mach_voucher_name_t` (etc.) that
        // collides with the `typedef mach_port_name_t mach_voucher_name_t` in
        // <mach/mach_types.h>.
        // All other port-flavour wire types map to mach_port_t.
        if inn == crate::types::ipc_type_num::MACH_MSG_TYPE_PORT_NAME {
            ty.user_type = Some("mach_port_name_t".into());
            ty.server_type = Some("mach_port_name_t".into());
            // port_type stays false — it is an integer name, not a port object.
        } else if is_port_type(inn) {
            ty.user_type = Some("mach_port_t".into());
            ty.server_type = Some("mach_port_t".into());
            ty.port_type = true;
        }
        Some(ty)
    }

    // Returns (in_num, in_str, out_num, out_str, size)
    fn parse_ipc_type(&mut self) -> Option<(u32, Option<String>, u32, Option<String>, u32)> {
        let (inn, in_s, outn, out_s, sz1) = self.parse_prim_ipc_type()?;
        if self.peek() == Some(&Token::Bar) {
            self.pos += 1;
            let (_, _, outn2, out_s2, sz2) = self.parse_prim_ipc_type()?;
            let size = match (sz1, sz2) {
                (0, s) | (s, 0) => s,
                (a, b) if a == b => a,
                _ => {
                    let sp = self.peek_span();
                    self.diag.error(sp, "sizes in IPCTypes don't match");
                    0
                }
            };
            Some((inn, in_s, outn2, out_s2, size))
        } else {
            Some((inn, in_s, outn, out_s, sz1))
        }
    }

    fn parse_prim_ipc_type(&mut self) -> Option<(u32, Option<String>, u32, Option<String>, u32)> {
        match self.peek()? {
            Token::Number(n) => {
                let n = *n;
                self.pos += 1;
                Some((n, None, n, None, 0))
            }
            Token::SymbolicType {
                in_number,
                in_str,
                out_number,
                out_str,
                size,
            } => {
                let v = (
                    *in_number,
                    Some(in_str.clone()),
                    *out_number,
                    Some(out_str.clone()),
                    *size,
                );
                self.pos += 1;
                Some(v)
            }
            _ => {
                let sp = self.peek_span();
                self.diag.error(sp, "expected IPC type");
                None
            }
        }
    }

    fn parse_array_spec(&mut self) -> Option<IpcType> {
        self.pos += 1; // consume `array`
        self.expect(&Token::LBrack);
        // variants: [], [*], [* : expr], [expr]
        let variant = if self.peek() == Some(&Token::RBrack) {
            // []
            self.pos += 1;
            self.expect(&Token::Of);
            ('v', 0u32)
        } else if self.peek() == Some(&Token::Star) {
            self.pos += 1;
            if self.peek() == Some(&Token::RBrack) {
                // [*]
                self.pos += 1;
                self.expect(&Token::Of);
                ('v', 0)
            } else {
                // [* : expr]
                self.expect(&Token::Colon);
                let max = self.parse_int_expr()?;
                self.expect(&Token::RBrack);
                self.expect(&Token::Of);
                ('v', max)
            }
        } else {
            let n = self.parse_int_expr()?;
            self.expect(&Token::RBrack);
            self.expect(&Token::Of);
            ('f', n)
        };
        let elem = self.parse_type_spec()?;
        Some(match variant.0 {
            'v' => IpcType::var_array_decl(variant.1, elem),
            _ => IpcType::array_decl(variant.1, elem),
        })
    }

    fn parse_struct_spec(&mut self) -> Option<IpcType> {
        self.pos += 1; // consume `struct`
        self.expect(&Token::LBrack);
        let n = self.parse_int_expr()?;
        self.expect(&Token::RBrack);
        self.expect(&Token::Of);
        let elem = self.parse_type_spec()?;
        Some(IpcType::struct_decl(n, elem))
    }

    fn parse_cstring_spec(&mut self) -> Option<IpcType> {
        self.pos += 1; // consume `c_string`
        self.expect(&Token::LBrack);
        let varying = if self.peek() == Some(&Token::Star) {
            self.pos += 1;
            self.expect(&Token::Colon);
            true
        } else {
            false
        };
        let count = self.parse_int_expr()?;
        self.expect(&Token::RBrack);
        Some(IpcType::cstring_decl(count, varying))
    }

    fn parse_native_type_spec(&mut self) -> Option<IpcType> {
        let variant = self.peek()?.clone();
        self.pos += 1;
        self.expect(&Token::LParen);
        let c_type = self.parse_type_phrase()?;
        match variant {
            Token::PointerTo => {
                self.expect(&Token::RParen);
                Some(IpcType::native_type(c_type, true, None))
            }
            Token::PointerToIfNot => {
                self.expect(&Token::Comma);
                let not_val = self.parse_type_phrase()?;
                self.expect(&Token::RParen);
                Some(IpcType::native_type(c_type, true, Some(not_val)))
            }
            Token::ValueOf => {
                self.expect(&Token::RParen);
                Some(IpcType::native_type(c_type, false, None))
            }
            _ => unreachable!(),
        }
    }

    fn parse_type_phrase(&mut self) -> Option<String> {
        let mut parts = vec![self.expect_identifier()?];
        while let Some(Token::Identifier(_)) = self.peek() {
            parts.push(self.expect_identifier()?);
        }
        Some(parts.join(" "))
    }

    fn parse_named_type_spec(&mut self) -> Option<IpcType> {
        self.parse_trans_type_spec()
    }

    // -----------------------------------------------------------------------
    // Integer expression (supports + - * /)
    // -----------------------------------------------------------------------

    fn parse_int_expr(&mut self) -> Option<u32> {
        self.parse_additive()
    }

    fn parse_additive(&mut self) -> Option<u32> {
        let mut lhs = self.parse_multiplicative()?;
        loop {
            match self.peek() {
                Some(Token::Plus) => {
                    self.pos += 1;
                    lhs += self.parse_multiplicative()?;
                }
                Some(Token::Minus) => {
                    self.pos += 1;
                    lhs -= self.parse_multiplicative()?;
                }
                _ => break,
            }
        }
        Some(lhs)
    }

    fn parse_multiplicative(&mut self) -> Option<u32> {
        let mut lhs = self.parse_primary_int()?;
        loop {
            match self.peek() {
                Some(Token::Star) => {
                    self.pos += 1;
                    lhs *= self.parse_primary_int()?;
                }
                Some(Token::Div) => {
                    self.pos += 1;
                    lhs /= self.parse_primary_int()?;
                }
                _ => break,
            }
        }
        Some(lhs)
    }

    fn parse_primary_int(&mut self) -> Option<u32> {
        match self.peek()? {
            Token::Number(n) => {
                let n = *n;
                self.pos += 1;
                Some(n)
            }
            Token::LParen => {
                self.pos += 1;
                let v = self.parse_int_expr()?;
                self.expect(&Token::RParen);
                Some(v)
            }
            _ => {
                let sp = self.peek_span();
                self.diag.error(sp, "expected integer expression");
                None
            }
        }
    }
}

/// Returns true for IPC type numbers that represent actual Mach port objects.
/// NOTE: MACH_MSG_TYPE_PORT_NAME is intentionally excluded — it represents a
/// port *name* (unsigned integer), not a port object, and maps to mach_port_name_t.
fn is_port_type(n: u32) -> bool {
    use crate::types::ipc_type_num::*;
    matches!(
        n,
        MACH_MSG_TYPE_MOVE_RECEIVE // = PORT_RECEIVE
            | MACH_MSG_TYPE_PORT_SEND // = MOVE_SEND
            | MACH_MSG_TYPE_PORT_SEND_ONCE // = MOVE_SEND_ONCE
            | MACH_MSG_TYPE_COPY_SEND
            | MACH_MSG_TYPE_MAKE_SEND
            | MACH_MSG_TYPE_MAKE_SEND_ONCE
            | MACH_MSG_TYPE_POLYMORPHIC
    )
}
