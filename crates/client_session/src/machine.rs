use crate::EntryStage;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EntryState {
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

/// Explicit ordered state for headless realm discovery or character selection.
pub(crate) struct EntryMachine {
    state: EntryState,
}

impl EntryMachine {
    pub(crate) const fn new() -> Self {
        Self {
            state: EntryState::AwaitingStart,
        }
    }

    pub(crate) fn begin(&mut self) -> Result<EntryStage, InvalidTransition> {
        self.advance(
            EntryState::AwaitingStart,
            EntryState::Connecting,
            EntryStage::LoginConnection,
        )
    }

    pub(crate) fn authenticating(&mut self) -> Result<EntryStage, InvalidTransition> {
        self.advance(
            EntryState::Connecting,
            EntryState::Authenticating,
            EntryStage::LoginAuthentication,
        )
    }

    pub(crate) fn selecting_realm(&mut self) -> Result<EntryStage, InvalidTransition> {
        self.advance(
            EntryState::Authenticating,
            EntryState::SelectingRealm,
            EntryStage::RealmSelection,
        )
    }

    pub(crate) fn complete(&mut self) -> Result<(), InvalidTransition> {
        if self.state != EntryState::SelectingCharacter {
            return Err(InvalidTransition);
        }
        self.state = EntryState::Complete;
        Ok(())
    }

    pub(crate) fn realm_discovered(&mut self) -> Result<(), InvalidTransition> {
        if self.state != EntryState::SelectingRealm {
            return Err(InvalidTransition);
        }
        self.state = EntryState::RealmSelected;
        Ok(())
    }

    pub(crate) fn complete_after_realm(&mut self) -> Result<(), InvalidTransition> {
        if self.state != EntryState::RealmSelected {
            return Err(InvalidTransition);
        }
        self.state = EntryState::Complete;
        Ok(())
    }

    pub(crate) fn authenticating_world(&mut self) -> Result<EntryStage, InvalidTransition> {
        self.advance(
            EntryState::RealmSelected,
            EntryState::AuthenticatingWorld,
            EntryStage::WorldAuthentication,
        )
    }

    pub(crate) fn selecting_character(&mut self) -> Result<EntryStage, InvalidTransition> {
        self.advance(
            EntryState::AuthenticatingWorld,
            EntryState::SelectingCharacter,
            EntryStage::CharacterSelection,
        )
    }

    pub(crate) fn fail(&mut self) {
        self.state = EntryState::Failed;
    }

    fn advance(
        &mut self,
        expected: EntryState,
        next: EntryState,
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

    use super::EntryMachine;

    #[test]
    fn state_machine_requires_challenge_proof_and_realm_order() {
        let mut machine = EntryMachine::new();
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
