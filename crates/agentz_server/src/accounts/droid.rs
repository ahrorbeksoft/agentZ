//! Factory Droid's accounts.

use std::collections::BTreeMap;

use super::{AgentDescription, LoginCheck};

/// Droid keeps everything in `.factory` under `FACTORY_HOME_OVERRIDE`, or else the user's home.
/// Its login there is encrypted with a key from the keychain that every home shares, so the
/// login itself stays per home.
pub(super) fn description() -> AgentDescription {
    AgentDescription {
        home_variables: BTreeMap::from([("FACTORY_HOME_OVERRIDE".into(), String::new())]),
        file_storage: BTreeMap::new(),
        // A key in the environment overrides the stored login.
        login_variables: vec!["FACTORY_API_KEY".into()],
        // Logged out, `session/new` fails and offers a pairing code.
        login_check: LoginCheck::Session,
    }
}
