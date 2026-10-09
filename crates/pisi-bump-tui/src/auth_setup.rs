use crossterm::event::KeyCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthOption {
    GhAuth,
    Manual,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthChoice {
    GhAuth,
    Manual(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AuthSetupState {
    pub option: Option<AuthOption>,
    pub token_input: String,
}

impl AuthSetupState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn handle_key(&mut self, key: KeyCode) -> Option<AuthChoice> {
        match key {
            KeyCode::Char('1') => {
                self.option = Some(AuthOption::GhAuth);
                None
            }
            KeyCode::Char('2') => {
                self.option = Some(AuthOption::Manual);
                None
            }
            KeyCode::Backspace if self.option == Some(AuthOption::Manual) => {
                self.token_input.pop();
                None
            }
            KeyCode::Char(character) if self.option == Some(AuthOption::Manual) => {
                self.token_input.push(character);
                None
            }
            KeyCode::Enter => self.resolve_choice(),
            _ => None,
        }
    }

    fn resolve_choice(&self) -> Option<AuthChoice> {
        match self.option {
            Some(AuthOption::GhAuth) => Some(AuthChoice::GhAuth),
            Some(AuthOption::Manual) if !self.token_input.trim().is_empty() => {
                Some(AuthChoice::Manual(self.token_input.trim().to_string()))
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_produce_no_choice_until_an_option_is_picked() {
        let mut state = AuthSetupState::new();
        assert_eq!(state.handle_key(KeyCode::Enter), None);
        assert_eq!(state.option, None);
    }

    #[test]
    fn should_confirm_gh_auth_immediately_on_enter() {
        let mut state = AuthSetupState::new();
        state.handle_key(KeyCode::Char('1'));
        assert_eq!(state.option, Some(AuthOption::GhAuth));
        assert_eq!(state.handle_key(KeyCode::Enter), Some(AuthChoice::GhAuth));
    }

    #[test]
    fn should_require_non_empty_token_before_confirming_manual_entry() {
        let mut state = AuthSetupState::new();
        state.handle_key(KeyCode::Char('2'));
        assert_eq!(state.option, Some(AuthOption::Manual));

        assert_eq!(
            state.handle_key(KeyCode::Enter),
            None,
            "empty token must not confirm"
        );

        state.handle_key(KeyCode::Char('a'));
        state.handle_key(KeyCode::Char('b'));
        state.handle_key(KeyCode::Char('c'));
        assert_eq!(state.token_input, "abc");

        assert_eq!(
            state.handle_key(KeyCode::Enter),
            Some(AuthChoice::Manual("abc".to_string()))
        );
    }

    #[test]
    fn should_support_backspace_while_entering_manual_token() {
        let mut state = AuthSetupState::new();
        state.handle_key(KeyCode::Char('2'));
        state.handle_key(KeyCode::Char('x'));
        state.handle_key(KeyCode::Char('y'));
        state.handle_key(KeyCode::Backspace);
        assert_eq!(state.token_input, "x");
    }

    #[test]
    fn should_ignore_character_input_before_manual_option_is_selected() {
        let mut state = AuthSetupState::new();
        state.handle_key(KeyCode::Char('z'));
        assert_eq!(state.token_input, "");
        assert_eq!(state.option, None);
    }

    #[test]
    fn should_switch_option_and_keep_working_when_toggled() {
        let mut state = AuthSetupState::new();
        state.handle_key(KeyCode::Char('2'));
        state.handle_key(KeyCode::Char('a'));
        state.handle_key(KeyCode::Char('1'));
        assert_eq!(state.option, Some(AuthOption::GhAuth));
        assert_eq!(
            state.handle_key(KeyCode::Enter),
            Some(AuthChoice::GhAuth),
            "switching to gh_auth should confirm regardless of leftover token text"
        );
    }
}
