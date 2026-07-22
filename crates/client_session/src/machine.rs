use crate::EntryStage;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DiscoveryState {
    AwaitingStart,
    Connecting,
    Authenticating,
    SelectingRealm,
    RealmSelected,
    AuthenticatingWorld,
    SelectingCharacter,
    Complete,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct InvalidTransition;

/// Explicit ordered state for the login-only realm-discovery capability.
pub(crate) struct RealmDiscoveryMachine {
    state: DiscoveryState,
}

impl RealmDiscoveryMachine {
    pub(crate) const fn new() -> Self {
        Self {
            state: DiscoveryState::AwaitingStart,
        }
    }

    pub(crate) fn begin(&mut self) -> Result<EntryStage, InvalidTransition> {
        self.advance(
            DiscoveryState::AwaitingStart,
            DiscoveryState::Connecting,
            EntryStage::LoginConnection,
        )
    }

    pub(crate) fn authenticating(&mut self) -> Result<EntryStage, InvalidTransition> {
        self.advance(
            DiscoveryState::Connecting,
            DiscoveryState::Authenticating,
            EntryStage::LoginAuthentication,
        )
    }

    pub(crate) fn selecting_realm(&mut self) -> Result<EntryStage, InvalidTransition> {
        self.advance(
            DiscoveryState::Authenticating,
            DiscoveryState::SelectingRealm,
            EntryStage::RealmSelection,
        )
    }

    pub(crate) fn complete(&mut self) -> Result<(), InvalidTransition> {
        if self.state != DiscoveryState::SelectingCharacter {
            return Err(InvalidTransition);
        }
        self.state = DiscoveryState::Complete;
        Ok(())
    }

    pub(crate) fn realm_discovered(&mut self) -> Result<(), InvalidTransition> {
        if self.state != DiscoveryState::SelectingRealm {
            return Err(InvalidTransition);
        }
        self.state = DiscoveryState::RealmSelected;
        Ok(())
    }

    pub(crate) fn complete_after_realm(&mut self) -> Result<(), InvalidTransition> {
        if self.state != DiscoveryState::RealmSelected {
            return Err(InvalidTransition);
        }
        self.state = DiscoveryState::Complete;
        Ok(())
    }

    pub(crate) fn authenticating_world(&mut self) -> Result<EntryStage, InvalidTransition> {
        self.advance(
            DiscoveryState::RealmSelected,
            DiscoveryState::AuthenticatingWorld,
            EntryStage::WorldAuthentication,
        )
    }

    pub(crate) fn selecting_character(&mut self) -> Result<EntryStage, InvalidTransition> {
        self.advance(
            DiscoveryState::AuthenticatingWorld,
            DiscoveryState::SelectingCharacter,
            EntryStage::CharacterSelection,
        )
    }

    pub(crate) fn fail(&mut self) {
        self.state = DiscoveryState::Failed;
    }

    fn advance(
        &mut self,
        expected: DiscoveryState,
        next: DiscoveryState,
        stage: EntryStage,
    ) -> Result<EntryStage, InvalidTransition> {
        if self.state != expected {
            return Err(InvalidTransition);
        }
        self.state = next;
        Ok(stage)
    }
}

#[cfg(test)]
mod tests {
    use crate::EntryStage;

    use super::RealmDiscoveryMachine;

    #[test]
    fn state_machine_requires_challenge_proof_and_realm_order() {
        let mut machine = RealmDiscoveryMachine::new();
        assert!(machine.selecting_realm().is_err());
        assert_eq!(machine.begin().unwrap(), EntryStage::LoginConnection);
        assert_eq!(
            machine.authenticating().unwrap(),
            EntryStage::LoginAuthentication
        );
        assert_eq!(
            machine.selecting_realm().unwrap(),
            EntryStage::RealmSelection
        );
        machine.realm_discovered().unwrap();
        assert_eq!(
            machine.authenticating_world().unwrap(),
            EntryStage::WorldAuthentication
        );
        assert_eq!(
            machine.selecting_character().unwrap(),
            EntryStage::CharacterSelection
        );
        machine.complete().unwrap();
        assert!(machine.authenticating().is_err());
    }
}
