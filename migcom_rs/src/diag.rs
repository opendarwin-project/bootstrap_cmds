// SPDX-License-Identifier: MIT
// Diagnostic reporting using annotate-snippets 0.12

use annotate_snippets::{AnnotationKind, Group, Level, Renderer, Snippet};

/// Shared diagnostic context (source text + filename)
pub struct Diag {
    pub filename: String,
    pub source: String,
    pub error_count: usize,
    pub quiet: bool,
}

impl Diag {
    pub fn new(filename: impl Into<String>, source: impl Into<String>, quiet: bool) -> Self {
        Self {
            filename: filename.into(),
            source: source.into(),
            error_count: 0,
            quiet,
        }
    }

    /// Emit an error at a byte-offset span; increments `error_count`.
    pub fn error(&mut self, span: std::ops::Range<usize>, msg: &str) {
        let renderer = Renderer::styled();
        let groups = &[Group::with_title(Level::ERROR.primary_title(msg)).element(
            Snippet::source(&self.source)
                .path(&self.filename)
                .annotation(AnnotationKind::Primary.span(span).label(msg)),
        )];
        eprintln!("{}", renderer.render(groups));
        self.error_count += 1;
    }

    /// Emit a warning (suppressed when `--quiet`).
    #[allow(dead_code)]
    pub fn warn(&mut self, span: std::ops::Range<usize>, msg: &str) {
        if self.quiet {
            return;
        }
        let renderer = Renderer::styled();
        let groups = &[
            Group::with_title(Level::WARNING.primary_title(msg)).element(
                Snippet::source(&self.source)
                    .path(&self.filename)
                    .annotation(AnnotationKind::Context.span(span).label(msg)),
            ),
        ];
        eprintln!("{}", renderer.render(groups));
    }

    /// Emit a fatal error: print and abort.
    #[allow(dead_code)]
    pub fn fatal(&mut self, span: std::ops::Range<usize>, msg: &str) -> ! {
        self.error(span, msg);
        std::process::exit(1);
    }

    /// Emit an error with no source location.
    pub fn error_noloc(&mut self, msg: &str) {
        eprintln!("error: {msg}");
        self.error_count += 1;
    }

    #[allow(dead_code)]
    pub fn fatal_noloc(&self, msg: &str) -> ! {
        eprintln!("fatal: {msg}");
        std::process::exit(1);
    }
}
