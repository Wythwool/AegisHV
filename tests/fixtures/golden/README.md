# Golden JSONL fixtures

These fixtures are checked by `tests/golden_tests.rs`.

`replay_kvm_exit_sample.jsonl` is generated from `examples/traces/kvm_exit_sample.log`:

```bash
cargo run --locked -- run --replay ./examples/traces/kvm_exit_sample.log --deterministic-replay --jsonl tests/fixtures/golden/replay_kvm_exit_sample.jsonl --listen '' --quiet
```

`replay_wx_policy_action_sample.jsonl` is generated from the existing W^X corpus and `config.example.toml`:

```bash
cargo run --locked -- run --replay ./corpus/malicious/wx_same_vm_same_as.log --deterministic-replay --config config.example.toml --jsonl tests/fixtures/golden/replay_wx_policy_action_sample.jsonl --listen '' --quiet
```

`pmu_loss_contract_sample.jsonl` is hand-authored and schema-checked. Replay mode intentionally disables the PMU sampler, and queue-loss replay depends on scheduler timing rather than event semantics. This fixture covers the PMU and loss JSON contracts without pretending deterministic replay produces PMU samples or deterministic queue drops.
