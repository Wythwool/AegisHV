# Windows symbol cache

The Windows symbol cache is an offline metadata format for pre-extracted kernel symbols. It is not a downloader, PDB parser, Microsoft symbol server client, or profile-distribution service.

The first logical line is:

```text
aegishv-windows-symbol-cache-v1
```

Required fields:

- `pdb_file=<nt-kernel-pdb-name>`
- `pdb_guid=<pdb-guid-without-dashes>`
- `pdb_age=<age>`
- `source=<short-source-description>`

Optional compatibility field:

- `profile_version=aegishv-windows-profile-v1`

Symbol records:

- `symbol=<name>,<rva>[,<size>]`

The cache rejects empty symbol sets, duplicate names, unsupported profile versions, and unknown keys.

## Intended use

The cache records symbols that were already extracted elsewhere. A caller can turn those symbols into a Windows profile fixture, or compare cache identity against a profile before constructing a profile in a controlled test path.

The cache does not contact the network and does not infer symbols from binaries. Exact PDB identity still matters: file name, GUID, and age must match the profile identity used by the caller.

## Limits

No real Windows symbol cache data ships by default. The shipped cache fixtures are synthetic and do not represent Microsoft binaries.

There is no nearest-match fallback. There is no automatic symbol download. There is no PDB type reconstruction. Structure offsets must be supplied explicitly by a trusted offline source before any walker that depends on those offsets can run.
