use std::env;
use std::time::Instant;

use aegishv::trap::controller::{
    armable_mapping, TrapClassification, TrapController, TrapHit, TrapKind,
};
use aegishv::trap::jit::JitTrapPolicy;
use aegishv::trap::singlestep::SingleStepStrategy;
use aegishv::trap::stage2::{PageSize, Stage2BackendKind, Stage2Permissions};
use aegishv::trap::stage2_model::Stage2Table;
use aegishv::trap::storm::{TrapStormGuard, TrapStormMode};

fn main() {
    let iterations = parse_iterations(env::args().skip(1)).unwrap_or_else(|err| {
        eprintln!("{err}");
        std::process::exit(2);
    });
    let mut table = Stage2Table::new();
    table
        .map(
            armable_mapping(
                "bench-vm",
                "as0",
                0x2000,
                PageSize::Size4K,
                Stage2Permissions::READ_EXEC,
            )
            .expect("bench mapping is valid"),
        )
        .expect("bench table has no overlap");
    let storm = TrapStormGuard::new(
        iterations as u64 + 1,
        iterations as u32 + 1,
        TrapStormMode::FailClosed,
    )
    .expect("bench storm guard is valid");
    let mut controller = TrapController::new(
        Stage2BackendKind::Synthetic,
        table,
        storm,
        JitTrapPolicy::default(),
        SingleStepStrategy::SyntheticStep,
    );
    let trap_id = controller
        .arm_trap("bench-vm", "as0", 0x2000, TrapKind::Execute)
        .expect("bench trap can be armed");

    let start = Instant::now();
    for index in 0..iterations {
        controller
            .handle_hit(
                TrapHit {
                    owner_vm: "bench-vm".to_string(),
                    address_space: "as0".to_string(),
                    gpa: 0x2000,
                    vcpu_id: Some(0),
                    rip: Some(0x401000 + index),
                    kind: TrapKind::Execute,
                },
                TrapClassification::AllowStep,
                index,
            )
            .expect("bench trap hit should classify");
        controller
            .rearm(&trap_id)
            .expect("bench trap should re-arm");
    }
    let elapsed = start.elapsed();
    println!(
        "trap_synthetic_bench iterations={} elapsed_us={} transitions={}",
        iterations,
        elapsed.as_micros(),
        iterations.saturating_mul(2)
    );
}

fn parse_iterations(args: impl Iterator<Item = String>) -> Result<u64, String> {
    let mut iterations = 10_000_u64;
    let mut pending = None;
    for arg in args {
        if let Some(flag) = pending.take() {
            if flag == "--iterations" {
                iterations = arg
                    .parse::<u64>()
                    .map_err(|_| "--iterations expects a positive integer".to_string())?;
                continue;
            }
        }
        match arg.as_str() {
            "--iterations" => pending = Some(arg),
            "--help" | "-h" => {
                println!("usage: trap_synthetic_bench [--iterations N]");
                std::process::exit(0);
            }
            _ => return Err(format!("unknown argument: {arg}")),
        }
    }
    if pending.is_some() {
        return Err("--iterations expects a value".to_string());
    }
    if iterations == 0 {
        return Err("--iterations must be greater than zero".to_string());
    }
    Ok(iterations)
}
