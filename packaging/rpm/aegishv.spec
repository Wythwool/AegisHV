%global debug_package %{nil}
%global _build_id_links none

Name:           aegishv
Version:        0.4.0
Release:        1%{?dist}
Summary:        host-side KVM telemetry sensor
License:        MIT
URL:            https://github.com/Nullbit1/AegisHV
Source0:        %{name}-%{version}.tar.gz
BuildRequires:  cargo
BuildRequires:  rust
Requires(pre):  shadow-utils
Requires(post): systemd
Requires(postun): systemd
Requires:       systemd

%description
AegisHV reads Linux KVM tracefs text, emits JSONL telemetry, exposes
Prometheus-style metrics, correlates W^X patterns, and can call configured QMP
actions. This package installs the current host-side sensor and operator files.
It does not provide type-1 hypervisor support, full VMI, EPT/NPT enforcement,
syscall-path integrity, live libvirt integration, or hardware PMU sampling.

%prep
%autosetup -n %{name}-%{version}

%build
%if 0%{?aegishv_cargo_target:1}
cargo build --locked --release --target %{aegishv_cargo_target}
%else
cargo build --locked --release
%endif

%install
rm -rf %{buildroot}
%if 0%{?aegishv_cargo_target:1}
install -D -m 0755 target/%{aegishv_cargo_target}/release/aegishv %{buildroot}%{_bindir}/aegishv
%else
install -D -m 0755 target/release/aegishv %{buildroot}%{_bindir}/aegishv
%endif
install -D -m 0644 config.example.toml %{buildroot}%{_sysconfdir}/aegishv/config.toml
install -D -m 0644 schema/event.schema.json %{buildroot}%{_datadir}/aegishv/schema/event.schema.json
install -D -m 0644 schema/snapshot.schema.json %{buildroot}%{_datadir}/aegishv/schema/snapshot.schema.json
install -D -m 0644 packaging/seccomp/aegishv-seccomp.json %{buildroot}%{_datadir}/aegishv/seccomp/aegishv-seccomp.json
install -D -m 0644 packaging/apparmor/usr.bin.aegishv %{buildroot}%{_datadir}/aegishv/apparmor/usr.bin.aegishv
install -D -m 0644 packaging/selinux/aegishv.te %{buildroot}%{_datadir}/aegishv/selinux/aegishv.te
install -D -m 0644 packaging/selinux/aegishv.fc %{buildroot}%{_datadir}/aegishv/selinux/aegishv.fc
install -D -m 0644 packaging/selinux/aegishv.if %{buildroot}%{_datadir}/aegishv/selinux/aegishv.if
install -D -m 0644 packaging/selinux/README.md %{buildroot}%{_datadir}/aegishv/selinux/README.md
install -D -m 0644 packaging/rpm/aegishv.service %{buildroot}/usr/lib/systemd/system/aegishv.service
install -D -m 0644 packaging/rpm/aegishv.tmpfiles %{buildroot}/usr/lib/tmpfiles.d/aegishv.conf
install -D -m 0755 scripts/live-tracefs-smoke.sh %{buildroot}%{_datadir}/aegishv/scripts/live-tracefs-smoke.sh
install -D -m 0755 scripts/smoke-replay.sh %{buildroot}%{_datadir}/aegishv/scripts/smoke-replay.sh
install -D -m 0755 scripts/validate-jsonl-schema.py %{buildroot}%{_datadir}/aegishv/scripts/validate-jsonl-schema.py
install -D -m 0644 README.md %{buildroot}%{_docdir}/aegishv/README.md
install -D -m 0644 CHANGELOG.md %{buildroot}%{_docdir}/aegishv/CHANGELOG.md
install -D -m 0644 RELEASE.md %{buildroot}%{_docdir}/aegishv/RELEASE.md
install -D -m 0644 LICENSE %{buildroot}%{_docdir}/aegishv/LICENSE
install -D -m 0644 packaging/rpm/README.md %{buildroot}%{_docdir}/aegishv/RPM_PACKAGING.md
for doc in docs/*.md; do
    install -D -m 0644 "$doc" "%{buildroot}%{_docdir}/aegishv/$(basename "$doc")"
done
install -d -m 0750 %{buildroot}/var/lib/aegishv
install -d -m 0750 %{buildroot}/var/lib/aegishv/dumps
install -d -m 0750 %{buildroot}/var/lib/aegishv/spool
install -d -m 0750 %{buildroot}/var/log/aegishv

%pre
getent group aegishv >/dev/null || groupadd -r aegishv
getent passwd aegishv >/dev/null || useradd -r -g aegishv -d /var/lib/aegishv -s /sbin/nologin -c "AegisHV sensor" aegishv
exit 0

%post
install -d -o aegishv -g aegishv -m 0750 /var/lib/aegishv
install -d -o aegishv -g aegishv -m 0750 /var/lib/aegishv/dumps
install -d -o aegishv -g aegishv -m 0750 /var/lib/aegishv/spool
install -d -o aegishv -g aegishv -m 0750 /var/log/aegishv
install -d -o aegishv -g aegishv -m 0750 /run/aegishv
if command -v systemd-tmpfiles >/dev/null 2>&1; then
    systemd-tmpfiles --create /usr/lib/tmpfiles.d/aegishv.conf || :
fi
if command -v systemctl >/dev/null 2>&1; then
    systemctl daemon-reload || :
fi
exit 0

%postun
if command -v systemctl >/dev/null 2>&1; then
    systemctl daemon-reload || :
fi
exit 0

%files
%license LICENSE
%config(noreplace) %{_sysconfdir}/aegishv/config.toml
%{_bindir}/aegishv
/usr/lib/systemd/system/aegishv.service
/usr/lib/tmpfiles.d/aegishv.conf
%{_datadir}/aegishv/schema/event.schema.json
%{_datadir}/aegishv/schema/snapshot.schema.json
%{_datadir}/aegishv/seccomp/aegishv-seccomp.json
%{_datadir}/aegishv/apparmor/usr.bin.aegishv
%{_datadir}/aegishv/selinux/aegishv.te
%{_datadir}/aegishv/selinux/aegishv.fc
%{_datadir}/aegishv/selinux/aegishv.if
%{_datadir}/aegishv/selinux/README.md
%{_datadir}/aegishv/scripts/live-tracefs-smoke.sh
%{_datadir}/aegishv/scripts/smoke-replay.sh
%{_datadir}/aegishv/scripts/validate-jsonl-schema.py
%{_docdir}/aegishv
%dir %attr(0750,aegishv,aegishv) /var/lib/aegishv
%dir %attr(0750,aegishv,aegishv) /var/lib/aegishv/dumps
%dir %attr(0750,aegishv,aegishv) /var/lib/aegishv/spool
%dir %attr(0750,aegishv,aegishv) /var/log/aegishv
