Name:           revisor
Version:        %{revisor_version}
Release:        1%{?dist}
Summary:        Code review app for GitHub repositories
License:        LicenseRef-Not-Declared
Requires:       vulkan-loader

%description
Revisor is a desktop app for reviewing GitHub pull requests.

%prep

%build

%install
install -Dm755 %{revisor_root}/target/fedora-x86_64/release/revisor %{buildroot}%{_bindir}/revisor
install -Dm644 %{revisor_root}/scripts/revisor.desktop %{buildroot}%{_datadir}/applications/revisor.desktop

%files
%{_bindir}/revisor
%{_datadir}/applications/revisor.desktop
