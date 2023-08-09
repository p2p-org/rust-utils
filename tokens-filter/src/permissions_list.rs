use std::collections::HashMap;

use solana_sdk::pubkey::Pubkey;

const NOT_DENIED: bool = true;

pub struct PermissionsList {
    tokens: HashMap<Pubkey, bool>,
}

impl PermissionsList {
    pub fn new(tokens: HashMap<Pubkey, bool>) -> Self {
        Self { tokens }
    }

    pub fn is_whitelisted(&self, token: &Pubkey) -> bool {
        self.tokens.get(token).copied().unwrap_or_default()
    }

    pub fn is_blacklisted(&self, token: &Pubkey) -> bool {
        !self.tokens.get(token).copied().unwrap_or(NOT_DENIED)
    }
}

impl Default for PermissionsList {
    fn default() -> Self {
        Self {
            tokens: Default::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permissions_list() {
        let allowed = Pubkey::new_unique();
        let denied = Pubkey::new_unique();
        let unknown = Pubkey::new_unique();

        let tokens = [(allowed, true), (denied, false)].into_iter().collect();

        let list = PermissionsList::new(tokens);

        assert!(list.is_whitelisted(&allowed));
        assert!(list.is_blacklisted(&denied));
        assert!(!list.is_whitelisted(&unknown));
        assert!(!list.is_blacklisted(&unknown));
    }
}
