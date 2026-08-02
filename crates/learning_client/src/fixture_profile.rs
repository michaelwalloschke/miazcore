use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::Path,
    time::Duration,
};

use client_protocol::TARGET_CLIENT_BUILD;
use client_session::{ClientConfig, ClientConfigSpec, ConfigError, CredentialPaths};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FixtureProfile {
    PairA,
    PairB,
}

impl FixtureProfile {
    #[allow(dead_code)] // used by the binary, not the fixture examples
    pub(crate) const fn token(self) -> &'static str {
        match self {
            Self::PairA => "pair-a",
            Self::PairB => "pair-b",
        }
    }

    #[allow(dead_code)] // used by the binary, not the fixture examples
    pub(crate) const fn window_suffix(self) -> &'static str {
        match self {
            Self::PairA => "PAIR A",
            Self::PairB => "PAIR B",
        }
    }

    pub(crate) const fn character_name(self) -> &'static str {
        match self {
            Self::PairA => "Miazpaira",
            Self::PairB => "Miazpairb",
        }
    }

    const fn credentials(self) -> (&'static str, &'static str) {
        match self {
            Self::PairA => ("fixture-pair-a-account", "fixture-pair-a-password"),
            Self::PairB => ("fixture-pair-b-account", "fixture-pair-b-password"),
        }
    }
}

pub(crate) fn configuration(
    repository_root: impl AsRef<Path>,
    profile: FixtureProfile,
) -> Result<ClientConfig, ConfigError> {
    let secret_root = repository_root.as_ref().join("infra/azerothcore/secrets");
    let (account, password) = profile.credentials();
    ClientConfig::new(ClientConfigSpec {
        realm_id: 1,
        realm_name: "Miazcore Reference Realm".to_owned(),
        character_name: profile.character_name().to_owned(),
        client_build: TARGET_CLIENT_BUILD,
        login_endpoint: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 3724),
        world_endpoint: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8085),
        connect_timeout: Duration::from_secs(5),
        io_timeout: Duration::from_secs(5),
        credentials: CredentialPaths::new(secret_root.join(account), secret_root.join(password)),
    })
}

#[cfg(test)]
mod tests {
    use super::{FixtureProfile, configuration};

    #[test]
    fn closed_profiles_have_distinct_fixed_identities_and_credential_paths() {
        let root = std::path::Path::new("/repository-root");
        let pair_a = configuration(root, FixtureProfile::PairA).unwrap();
        let pair_b = configuration(root, FixtureProfile::PairB).unwrap();

        assert_eq!(pair_a.identity().character_name(), "Miazpaira");
        assert_eq!(pair_b.identity().character_name(), "Miazpairb");
        assert_ne!(pair_a.credential_paths(), pair_b.credential_paths());
        for path in [
            pair_a.credential_paths().account(),
            pair_a.credential_paths().password(),
            pair_b.credential_paths().account(),
            pair_b.credential_paths().password(),
        ] {
            assert!(path.starts_with(root.join("infra/azerothcore/secrets")));
        }
    }
}
