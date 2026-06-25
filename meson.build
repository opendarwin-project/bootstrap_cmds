project(
    'bootstrap_cmds',
    'rust',
    version: '0.1.0',
    license: 'MIT',
    meson_version: '>= 1.9.0',
    default_options: [
        'rust_std=2024',
        'buildtype=release',
    ],
)

# ---------------------------------------------------------------------------
# Crate dependencies via subproject wraps
# ---------------------------------------------------------------------------

# Meson's cargo subproject integration auto-names deps as '<crate>-<api>-rs'.
# winnow 1.0.3 → api '1'  → 'winnow-1-rs'
winnow_dep = dependency(
    'winnow-1-rs',
    version: '>=1.0.3',
    fallback: ['winnow', 'winnow_dep'],
)

# annotate-snippets 0.12.16 → api '0.12' → 'annotate-snippets-0.12-rs'
annotate_snippets_dep = dependency(
    'annotate-snippets-0.12-rs',
    version: '>=0.12.16',
    fallback: ['annotate-snippets', 'annotate_snippets_dep'],
)

# ---------------------------------------------------------------------------
# migcom executable
# ---------------------------------------------------------------------------

migcom_exe = executable(
    'migcom',
    'migcom_rs/src/main.rs',
    dependencies: [
        winnow_dep,
        annotate_snippets_dep,
    ],
    install: true,
    install_dir: get_option('libexecdir'),
)

# ---------------------------------------------------------------------------
# mig wrapper script
# ---------------------------------------------------------------------------

install_data(
    'migcom.tproj/mig.sh',
    install_dir: get_option('bindir'),
    install_mode: 'rwxr-xr-x',
    rename: 'mig',
)

# ---------------------------------------------------------------------------
# Man pages
# ---------------------------------------------------------------------------

install_man('migcom.tproj/mig.1')
install_man('migcom.tproj/migcom.1')

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------

summary(
    {
        'prefix': get_option('prefix'),
        'bindir': get_option('bindir'),
        'libexecdir': get_option('libexecdir'),
        'winnow': winnow_dep.version(),
        'annotate-snippets': annotate_snippets_dep.version(),
    },
    section: 'Paths / Versions',
)