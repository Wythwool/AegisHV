#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Viewer,
    Operator,
    IncidentResponder,
    Admin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionPermission {
    PauseVm,
    ResumeVm,
    DumpGuestMemory,
    QuarantineNic,
    PolicyUpdate,
    ApproveAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RbacDecision {
    Allowed,
    Denied(&'static str),
}

pub fn authorize(role: Role, permission: ActionPermission) -> RbacDecision {
    match role {
        Role::Admin => RbacDecision::Allowed,
        Role::IncidentResponder => match permission {
            ActionPermission::DumpGuestMemory
            | ActionPermission::QuarantineNic
            | ActionPermission::ApproveAction => RbacDecision::Allowed,
            ActionPermission::PauseVm
            | ActionPermission::ResumeVm
            | ActionPermission::PolicyUpdate => {
                RbacDecision::Denied("incident responder role cannot change VM power state or policy")
            }
        },
        Role::Operator => match permission {
            ActionPermission::PauseVm | ActionPermission::ResumeVm => RbacDecision::Allowed,
            ActionPermission::DumpGuestMemory
            | ActionPermission::QuarantineNic
            | ActionPermission::PolicyUpdate
            | ActionPermission::ApproveAction => {
                RbacDecision::Denied("operator role is not authorized for evidence, quarantine, policy, or approval actions")
            }
        },
        Role::Viewer => RbacDecision::Denied("viewer role is read-only"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rbac_allows_admin_and_denies_viewer_actions() {
        assert_eq!(
            authorize(Role::Admin, ActionPermission::PolicyUpdate),
            RbacDecision::Allowed
        );
        assert!(matches!(
            authorize(Role::Viewer, ActionPermission::PauseVm),
            RbacDecision::Denied(_)
        ));
    }

    #[test]
    fn rbac_separates_operator_from_incident_responder_permissions() {
        assert_eq!(
            authorize(Role::Operator, ActionPermission::PauseVm),
            RbacDecision::Allowed
        );
        assert!(matches!(
            authorize(Role::Operator, ActionPermission::DumpGuestMemory),
            RbacDecision::Denied(_)
        ));
        assert_eq!(
            authorize(Role::IncidentResponder, ActionPermission::DumpGuestMemory),
            RbacDecision::Allowed
        );
        assert!(matches!(
            authorize(Role::IncidentResponder, ActionPermission::PolicyUpdate),
            RbacDecision::Denied(_)
        ));
    }
}
