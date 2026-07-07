#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkState {
    Down,
    Up,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuarantineState {
    Normal,
    Quarantined,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PacketClass {
    GuestData,
    Management,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetDecision {
    Allow,
    DropLinkDown,
    DropQuarantined,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VirtioNetQuarantine {
    pub link: LinkState,
    pub quarantine: QuarantineState,
    pub allow_management_traffic: bool,
}

impl VirtioNetQuarantine {
    pub const fn new(
        link: LinkState,
        quarantine: QuarantineState,
        allow_management_traffic: bool,
    ) -> Self {
        Self {
            link,
            quarantine,
            allow_management_traffic,
        }
    }

    pub const fn decide(self, packet: PacketClass) -> NetDecision {
        match self.link {
            LinkState::Down => NetDecision::DropLinkDown,
            LinkState::Up => match (self.quarantine, packet, self.allow_management_traffic) {
                (QuarantineState::Normal, _, _) => NetDecision::Allow,
                (QuarantineState::Quarantined, PacketClass::Management, true) => NetDecision::Allow,
                (QuarantineState::Quarantined, _, _) => NetDecision::DropQuarantined,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quarantine_drops_guest_data_but_can_allow_management_traffic() {
        let policy = VirtioNetQuarantine::new(LinkState::Up, QuarantineState::Quarantined, true);

        assert_eq!(
            policy.decide(PacketClass::GuestData),
            NetDecision::DropQuarantined
        );
        assert_eq!(policy.decide(PacketClass::Management), NetDecision::Allow);
    }

    #[test]
    fn link_down_fails_closed_before_quarantine_policy() {
        let policy = VirtioNetQuarantine::new(LinkState::Down, QuarantineState::Normal, true);

        assert_eq!(
            policy.decide(PacketClass::Management),
            NetDecision::DropLinkDown
        );
    }
}
