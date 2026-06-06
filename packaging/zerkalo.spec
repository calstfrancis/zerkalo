Name:           zerkalo
Version:        0.12.1
Release:        0
Summary:        Contemplative Typst editor with live preview and git sync
License:        MIT
URL:            https://github.com/calstfrancis/zerkalo
Source0:        %{name}-%{version}.tar.gz
Source1:        vendor.tar.gz

BuildRequires:  cargo
BuildRequires:  rust
BuildRequires:  pkgconfig(gtk4) >= 4.10
BuildRequires:  pkgconfig(libadwaita-1) >= 1.4
BuildRequires:  pkgconfig(gtksourceview-5)
BuildRequires:  pkgconfig(libgit2)
BuildRequires:  pkgconfig(openssl)
BuildRequires:  pkg-config

Requires:       pandoc
Requires:       hunspell
# tinymist LSP is bundled at /usr/lib/zerkalo/tinymist — no runtime dep needed

%description
Zerkalo is a Typst editor built with Rust, GTK4, and libadwaita.
It features live preview (embedded Typst compiler — no external binary needed),
multi-file tabs, LSP completions via tinymist, git sync, spell checking,
citation autocomplete, DOCX/LaTeX import and export via pandoc, and GOST
academic style support.

%prep
%autosetup -n %{name}-%{version} -a1

# Configure cargo to use the vendored dependency tree
mkdir -p .cargo
cat > .cargo/config.toml << 'CARGO_CONFIG'
[source.crates-io]
replace-with = "vendored-sources"

[source.vendored-sources]
directory = "vendor"
CARGO_CONFIG

%build
cargo build --release --frozen --offline

%install
# Binary
install -Dm755 target/release/zerkalo \
    %{buildroot}%{_bindir}/zerkalo

# Desktop entry
install -Dm644 packaging/io.github.calstfrancis.Zerkalo.desktop \
    %{buildroot}%{_datadir}/applications/io.github.calstfrancis.Zerkalo.desktop

# Icon
install -Dm644 packaging/zerkalo.svg \
    %{buildroot}%{_datadir}/icons/hicolor/scalable/apps/zerkalo.svg

# Bundled tinymist LSP binary
# (downloaded during GitHub release CI; not present in source tree)
# If packaging/tinymist exists (pre-fetched), install it
if [ -f packaging/tinymist ]; then
    install -Dm755 packaging/tinymist \
        %{buildroot}%{_libdir}/zerkalo/tinymist
fi

%files
%license LICENSE
%{_bindir}/zerkalo
%{_datadir}/applications/io.github.calstfrancis.Zerkalo.desktop
%{_datadir}/icons/hicolor/scalable/apps/zerkalo.svg
%{_libdir}/zerkalo/tinymist

%post
%icon_theme_cache_post

%postun
%icon_theme_cache_postun

%changelog
