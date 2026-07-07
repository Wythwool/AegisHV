use aegishv::trap::controller::{
    armable_mapping, TrapClassification, TrapController, TrapDecision, TrapHit, TrapKind, TrapState,
};
use aegishv::trap::invalidation::{plan_invalidation, InvalidationPrimitive, InvalidationScope};
use aegishv::trap::jit::{JitTrapContext, JitTrapPolicy, JitTrapRule};
use aegishv::trap::singlestep::{select_single_step, SingleStepCapabilities, SingleStepStrategy};
use aegishv::trap::stage2::{PageSize, Stage2BackendKind, Stage2Permissions};
use aegishv::trap::stage2_model::Stage2Table;
use aegishv::trap::storm::{TrapStormGuard, TrapStormMode};
use aegishv::trap::TrapErrorKind;

fn controller_with_mapping(storm: TrapStormGuard, jit_policy: JitTrapPolicy) -> TrapController {
    let mut table = Stage2Table::new();
    table
        .map(
            armable_mapping(
                "vm-a",
                "cr3:0x1000",
                0x2000,
                PageSize::Size4K,
                Stage2Permissions::READ_EXEC,
            )
            .unwrap(),
        )
        .unwrap();
    TrapController::new(
        Stage2BackendKind::Synthetic,
        table,
        storm,
        jit_policy,
        SingleStepStrategy::SyntheticStep,
    )
}

fn exec_hit() -> TrapHit {
    TrapHit {
        owner_vm: "vm-a".to_string(),
        address_space: "cr3:0x1000".to_string(),
        gpa: 0x2008,
        vcpu_id: Some(0),
        rip: Some(0x401000),
        kind: TrapKind::Execute,
    }
}

#[test]
fn synthetic_exec_trap_allows_one_step_and_rearms() {
    let storm = TrapStormGuard::new(1000, 8, TrapStormMode::FailClosed).unwrap();
    let mut controller = controller_with_mapping(storm, JitTrapPolicy::default());
    let trap_id = controller
        .arm_trap("vm-a", "cr3:0x1000", 0x2008, TrapKind::Execute)
        .unwrap();

    assert_eq!(
        controller
            .table()
            .lookup("vm-a", "cr3:0x1000", 0x2008)
            .unwrap()
            .permissions,
        Stage2Permissions::READ
    );

    let out = controller
        .handle_hit(exec_hit(), TrapClassification::AllowStep, 10)
        .unwrap();
    assert_eq!(out.trap_id, trap_id);
    assert_eq!(out.state, TrapState::AllowedStep);
    assert_eq!(out.decision, TrapDecision::AllowStep);
    assert_eq!(out.permissions_after, Stage2Permissions::READ_EXEC);
    assert_eq!(
        out.invalidation.primitive,
        InvalidationPrimitive::SyntheticRecord
    );

    let rearmed = controller.rearm(&trap_id).unwrap();
    assert_eq!(rearmed.state, TrapState::Rearmed);
    assert_eq!(
        controller
            .table()
            .lookup("vm-a", "cr3:0x1000", 0x2008)
            .unwrap()
            .permissions,
        Stage2Permissions::READ
    );
}

#[test]
fn synthetic_write_trap_uses_jit_policy_for_temporary_window() {
    let storm = TrapStormGuard::new(1000, 8, TrapStormMode::FailClosed).unwrap();
    let rule = JitTrapRule::new(Some("vm-a"), "java|node", None, 2, 5).unwrap();
    let mut controller = controller_with_mapping(storm, JitTrapPolicy::new(vec![rule]));
    let trap_id = controller
        .arm_trap("vm-a", "cr3:0x1000", 0x2008, TrapKind::Write)
        .unwrap();
    let mut hit = exec_hit();
    hit.kind = TrapKind::Write;

    let out = controller
        .handle_hit(
            hit,
            TrapClassification::JitTemporaryWindow(JitTrapContext {
                vm: "vm-a".to_string(),
                process: Some("java".to_string()),
                module: Some("libjvm.so".to_string()),
                symbol: Some("jit_compile".to_string()),
                page_count: 1,
                requested_window_ms: 4,
            }),
            10,
        )
        .unwrap();

    assert_eq!(out.decision, TrapDecision::AllowTemporaryWrite);
    assert_eq!(out.state, TrapState::AllowedStep);
    assert_eq!(
        controller
            .table()
            .lookup("vm-a", "cr3:0x1000", 0x2008)
            .unwrap()
            .permissions,
        Stage2Permissions::READ_EXEC
    );

    controller.rearm(&trap_id).unwrap();
    assert_eq!(
        controller
            .table()
            .lookup("vm-a", "cr3:0x1000", 0x2008)
            .unwrap()
            .permissions,
        Stage2Permissions::READ_EXEC.without_access(aegishv::trap::stage2::TrapAccessKind::Write)
    );
}

#[test]
fn jit_policy_denies_unattributed_temporary_window() {
    let storm = TrapStormGuard::new(1000, 8, TrapStormMode::FailClosed).unwrap();
    let rule = JitTrapRule::new(None, "java", None, 1, 5).unwrap();
    let mut controller = controller_with_mapping(storm, JitTrapPolicy::new(vec![rule]));
    controller
        .arm_trap("vm-a", "cr3:0x1000", 0x2008, TrapKind::Write)
        .unwrap();
    let mut hit = exec_hit();
    hit.kind = TrapKind::Write;

    let out = controller
        .handle_hit(
            hit,
            TrapClassification::JitTemporaryWindow(JitTrapContext {
                vm: "vm-a".to_string(),
                process: None,
                module: Some("libjvm.so".to_string()),
                symbol: None,
                page_count: 1,
                requested_window_ms: 4,
            }),
            10,
        )
        .unwrap();

    assert_eq!(out.state, TrapState::Denied);
    assert_eq!(out.decision, TrapDecision::Deny);
    assert!(out.note.contains("missing guest process attribution"));
}

#[test]
fn storm_guard_throttles_by_vm_page_and_vcpu() {
    let storm = TrapStormGuard::new(1000, 1, TrapStormMode::FailClosed).unwrap();
    let mut controller = controller_with_mapping(storm, JitTrapPolicy::default());
    controller
        .arm_trap("vm-a", "cr3:0x1000", 0x2008, TrapKind::Execute)
        .unwrap();
    controller
        .handle_hit(
            exec_hit(),
            TrapClassification::Deny("first hit".to_string()),
            10,
        )
        .unwrap();

    let out = controller
        .handle_hit(exec_hit(), TrapClassification::AllowStep, 11)
        .unwrap();

    assert_eq!(out.state, TrapState::StormThrottled);
    assert_eq!(out.decision, TrapDecision::FailClosed);
    assert_eq!(out.permissions_after, Stage2Permissions::READ);
}

#[test]
fn invalidation_plans_map_architecture_specific_primitives() {
    let page = InvalidationScope::SinglePage {
        owner_vm: "vm-a".to_string(),
        address_space: "as0".to_string(),
        gpa: 0x2000,
        page_size: PageSize::Size4K,
    };

    assert_eq!(
        plan_invalidation(Stage2BackendKind::IntelEpt, &page)
            .unwrap()
            .primitive,
        InvalidationPrimitive::IntelInveptSingleContext
    );
    assert_eq!(
        plan_invalidation(Stage2BackendKind::AmdNpt, &page)
            .unwrap()
            .primitive,
        InvalidationPrimitive::AmdInvlpga
    );
    assert_eq!(
        plan_invalidation(Stage2BackendKind::ArmStage2, &page)
            .unwrap()
            .primitive,
        InvalidationPrimitive::ArmTlbiVaae2
    );
}

#[test]
fn single_step_selection_refuses_missing_backend_capability() {
    let caps = SingleStepCapabilities {
        intel_monitor_trap_flag: true,
        ..SingleStepCapabilities::default()
    };
    assert_eq!(
        select_single_step(Stage2BackendKind::IntelEpt, caps).unwrap(),
        SingleStepStrategy::IntelMonitorTrapFlag
    );

    let err = select_single_step(
        Stage2BackendKind::ArmStage2,
        SingleStepCapabilities::default(),
    )
    .unwrap_err();
    assert_eq!(err.kind(), TrapErrorKind::UnsupportedCapability);
}
