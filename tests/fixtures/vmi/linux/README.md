# Linux VMI fixtures

These files are synthetic profile fixtures for parser and offline inspection tests.

They are not extracted from a real kernel and do not prove distro support. Memory contents used by hook, BPF, and detector tests are built inside the tests so the fixtures stay small and deterministic.

`synthetic_task_module.profile` adds task, module, syscall table, and LSTAR-adjacent symbol coverage for offline parser tests. It is still synthetic and does not prove distro support.
