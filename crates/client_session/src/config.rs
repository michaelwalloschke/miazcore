use std::{
    error::Error,
    fmt,
    fs::{self, File},
    io::Read,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
    time::Duration,
};

use client_protocol::TARGET_CLIENT_BUILD;
use zeroize::Zeroizing;

use crate::SanitizedIdentity;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CredentialFileKind {
    Account,
    Password,
}

impl fmt::Display for CredentialFileKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Account => formatter.write_str("account credential file"),
            Self::Password => formatter.write_str("password credential file"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CredentialFileProblem {
    Missing,
    Symlink,
    NotRegularFile,
    InsecurePermissions,
    Empty,
    TooLarge,
    InvalidCharacters,
    ExposedAsIdentity,
    Unreadable,
}

impl fmt::Display for CredentialFileProblem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing => formatter.write_str("is missing"),
            Self::Symlink => formatter.write_str("must not be a symbolic link"),
            Self::NotRegularFile => formatter.write_str("is not a regular file"),
            Self::InsecurePermissions => {
                formatter.write_str("must not be accessible by group or other users")
            }
            Self::Empty => formatter.write_str("is empty"),
            Self::TooLarge => formatter.write_str("exceeds the credential size limit"),
            Self::InvalidCharacters => formatter.write_str("contains unsupported characters"),
            Self::ExposedAsIdentity => {
                formatter.write_str("must not appear in the displayed identity")
            }
            Self::Unreadable => formatter.write_str("could not be read"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConfigError {
    InvalidIdentity(crate::IdentityError),
    UnsupportedBuild {
        configured: u16,
        required: u16,
    },
    NonLoopbackEndpoint,
    InvalidTimeout,
    DuplicateCredentialPath,
    CredentialFile {
        kind: CredentialFileKind,
        problem: CredentialFileProblem,
    },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidIdentity(error) => {
                write!(formatter, "invalid sanitized identity: {error}")
            }
            Self::UnsupportedBuild {
                configured,
                required,
            } => write!(
                formatter,
                "client build {configured} is unsupported; expected {required}"
            ),
            Self::NonLoopbackEndpoint => {
                formatter.write_str("World-entry Slice endpoints must be loopback addresses")
            }
            Self::InvalidTimeout => formatter.write_str("session timeouts must be non-zero"),
            Self::DuplicateCredentialPath => {
                formatter.write_str("account and password credential paths must differ")
            }
            Self::CredentialFile { kind, problem } => write!(formatter, "{kind} {problem}"),
        }
    }
}

impl Error for ConfigError {}

impl From<crate::IdentityError> for ConfigError {
    fn from(error: crate::IdentityError) -> Self {
        Self::InvalidIdentity(error)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CredentialPaths {
    account: PathBuf,
    password: PathBuf,
}

impl CredentialPaths {
    #[must_use]
    pub fn new(account: impl Into<PathBuf>, password: impl Into<PathBuf>) -> Self {
        Self {
            account: account.into(),
            password: password.into(),
        }
    }

    #[must_use]
    pub fn account(&self) -> &Path {
        &self.account
    }

    #[must_use]
    pub fn password(&self) -> &Path {
        &self.password
    }
}

#[derive(Clone, Debug)]
pub struct ClientConfigSpec {
    pub realm_id: u32,
    pub realm_name: String,
    pub character_name: String,
    pub client_build: u16,
    pub login_endpoint: SocketAddr,
    pub world_endpoint: SocketAddr,
    pub connect_timeout: Duration,
    pub io_timeout: Duration,
    pub credentials: CredentialPaths,
}

#[derive(Clone, Debug)]
pub struct ClientConfig {
    identity: SanitizedIdentity,
    login_endpoint: SocketAddr,
    world_endpoint: SocketAddr,
    connect_timeout: Duration,
    io_timeout: Duration,
    credentials: CredentialPaths,
}

impl ClientConfig {
    /// Validate and freeze a non-secret configuration specification.
    ///
    /// # Errors
    ///
    /// Returns an error for unsafe identity, endpoints, timeouts, paths, or client build values.
    pub fn new(spec: ClientConfigSpec) -> Result<Self, ConfigError> {
        if spec.client_build != TARGET_CLIENT_BUILD {
            return Err(ConfigError::UnsupportedBuild {
                configured: spec.client_build,
                required: TARGET_CLIENT_BUILD,
            });
        }
        if !spec.login_endpoint.ip().is_loopback() || !spec.world_endpoint.ip().is_loopback() {
            return Err(ConfigError::NonLoopbackEndpoint);
        }
        if spec.connect_timeout.is_zero() || spec.io_timeout.is_zero() {
            return Err(ConfigError::InvalidTimeout);
        }
        if spec.credentials.account == spec.credentials.password {
            return Err(ConfigError::DuplicateCredentialPath);
        }
        Ok(Self {
            identity: SanitizedIdentity::new(
                spec.realm_id,
                spec.realm_name,
                spec.character_name,
                spec.client_build,
            )?,
            login_endpoint: spec.login_endpoint,
            world_endpoint: spec.world_endpoint,
            connect_timeout: spec.connect_timeout,
            io_timeout: spec.io_timeout,
            credentials: spec.credentials,
        })
    }

    /// Build the accepted local Reference Realm configuration from a repository root.
    ///
    /// # Errors
    ///
    /// Returns an error if a repository-owned default violates configuration validation.
    pub fn reference_realm(repository_root: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let secret_root = repository_root.as_ref().join("infra/azerothcore/secrets");
        Self::new(ClientConfigSpec {
            realm_id: 1,
            realm_name: "Miazcore Reference Realm".to_owned(),
            character_name: "Miaztest".to_owned(),
            client_build: TARGET_CLIENT_BUILD,
            login_endpoint: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 3724),
            world_endpoint: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8085),
            connect_timeout: Duration::from_secs(5),
            io_timeout: Duration::from_secs(5),
            credentials: CredentialPaths::new(
                secret_root.join("fixture-account"),
                secret_root.join("fixture-password"),
            ),
        })
    }

    #[must_use]
    pub const fn identity(&self) -> &SanitizedIdentity {
        &self.identity
    }

    #[must_use]
    pub const fn login_endpoint(&self) -> SocketAddr {
        self.login_endpoint
    }

    #[must_use]
    pub const fn world_endpoint(&self) -> SocketAddr {
        self.world_endpoint
    }

    #[must_use]
    pub const fn connect_timeout(&self) -> Duration {
        self.connect_timeout
    }

    #[must_use]
    pub const fn io_timeout(&self) -> Duration {
        self.io_timeout
    }

    #[must_use]
    pub const fn credential_paths(&self) -> &CredentialPaths {
        &self.credentials
    }

    /// Validate and load credential files into zeroizing ownership.
    ///
    /// # Errors
    ///
    /// Returns an error when a credential path, permission, size, or value is unsafe.
    pub fn load(self) -> Result<LoadedClientConfig, ConfigError> {
        let account_file =
            open_credential_file(CredentialFileKind::Account, self.credentials.account())?;
        let password_file =
            open_credential_file(CredentialFileKind::Password, self.credentials.password())?;
        let credentials = CredentialMaterial {
            account: read_credential(CredentialFileKind::Account, account_file)?,
            password: read_credential(CredentialFileKind::Password, password_file)?,
        };
        reject_identity_exposure(
            CredentialFileKind::Account,
            &credentials.account,
            &self.identity,
        )?;
        reject_identity_exposure(
            CredentialFileKind::Password,
            &credentials.password,
            &self.identity,
        )?;
        Ok(LoadedClientConfig {
            config: self,
            credentials,
        })
    }
}

pub(crate) struct CredentialMaterial {
    account: Zeroizing<Vec<u8>>,
    password: Zeroizing<Vec<u8>>,
}

impl CredentialMaterial {
    pub(crate) fn normalize_for_login(&mut self) {
        self.account.make_ascii_uppercase();
        self.password.make_ascii_uppercase();
    }

    pub(crate) fn account(&self) -> &[u8] {
        &self.account
    }

    pub(crate) fn password(&self) -> &[u8] {
        &self.password
    }

    #[cfg(test)]
    pub(crate) fn synthetic(account: &[u8], password: &[u8]) -> Self {
        Self {
            account: Zeroizing::new(account.to_vec()),
            password: Zeroizing::new(password.to_vec()),
        }
    }
}

impl fmt::Debug for CredentialMaterial {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("CredentialMaterial([REDACTED])")
    }
}

pub struct LoadedClientConfig {
    pub(crate) config: ClientConfig,
    credentials: CredentialMaterial,
}

impl LoadedClientConfig {
    #[must_use]
    pub const fn config(&self) -> &ClientConfig {
        &self.config
    }

    pub(crate) fn into_parts(self) -> (ClientConfig, CredentialMaterial) {
        debug_assert!(!self.credentials.account.is_empty());
        debug_assert!(!self.credentials.password.is_empty());
        (self.config, self.credentials)
    }
}

impl fmt::Debug for LoadedClientConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LoadedClientConfig")
            .field("config", &self.config)
            .field("credentials", &"[REDACTED]")
            .finish()
    }
}

fn open_credential_file(kind: CredentialFileKind, path: &Path) -> Result<File, ConfigError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| ConfigError::CredentialFile {
        kind,
        problem: if error.kind() == std::io::ErrorKind::NotFound {
            CredentialFileProblem::Missing
        } else {
            CredentialFileProblem::Unreadable
        },
    })?;
    if metadata.file_type().is_symlink() {
        return Err(ConfigError::CredentialFile {
            kind,
            problem: CredentialFileProblem::Symlink,
        });
    }
    if !metadata.is_file() {
        return Err(ConfigError::CredentialFile {
            kind,
            problem: CredentialFileProblem::NotRegularFile,
        });
    }
    let file = File::open(path).map_err(|_| ConfigError::CredentialFile {
        kind,
        problem: CredentialFileProblem::Unreadable,
    })?;
    let opened_metadata = file.metadata().map_err(|_| ConfigError::CredentialFile {
        kind,
        problem: CredentialFileProblem::Unreadable,
    })?;
    if !same_file(&metadata, &opened_metadata) {
        return Err(ConfigError::CredentialFile {
            kind,
            problem: CredentialFileProblem::Unreadable,
        });
    }
    if opened_metadata.len() == 0 {
        return Err(ConfigError::CredentialFile {
            kind,
            problem: CredentialFileProblem::Empty,
        });
    }
    if opened_metadata.len() > 256 {
        return Err(ConfigError::CredentialFile {
            kind,
            problem: CredentialFileProblem::TooLarge,
        });
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if opened_metadata.permissions().mode() & 0o077 != 0 {
            return Err(ConfigError::CredentialFile {
                kind,
                problem: CredentialFileProblem::InsecurePermissions,
            });
        }
    }
    Ok(file)
}

fn read_credential(
    kind: CredentialFileKind,
    file: File,
) -> Result<Zeroizing<Vec<u8>>, ConfigError> {
    let mut value = Zeroizing::new(Vec::with_capacity(257));
    file.take(257)
        .read_to_end(&mut value)
        .map_err(|_| ConfigError::CredentialFile {
            kind,
            problem: CredentialFileProblem::Unreadable,
        })?;
    if value.len() > 256 {
        return Err(ConfigError::CredentialFile {
            kind,
            problem: CredentialFileProblem::TooLarge,
        });
    }
    if value.last() == Some(&b'\n') {
        value.pop();
        if value.last() == Some(&b'\r') {
            value.pop();
        }
    }
    if value.is_empty() {
        return Err(ConfigError::CredentialFile {
            kind,
            problem: CredentialFileProblem::Empty,
        });
    }
    if !value.iter().all(u8::is_ascii_graphic) {
        return Err(ConfigError::CredentialFile {
            kind,
            problem: CredentialFileProblem::InvalidCharacters,
        });
    }
    Ok(value)
}

#[cfg(unix)]
fn same_file(first: &fs::Metadata, second: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;

    first.dev() == second.dev() && first.ino() == second.ino()
}

#[cfg(not(unix))]
fn same_file(first: &fs::Metadata, second: &fs::Metadata) -> bool {
    first.is_file() && second.is_file() && first.len() == second.len()
}

fn reject_identity_exposure(
    kind: CredentialFileKind,
    credential: &[u8],
    identity: &SanitizedIdentity,
) -> Result<(), ConfigError> {
    let displayed_identity = format!(
        "{} {} {} {}",
        identity.realm_id(),
        identity.realm_name(),
        identity.character_name(),
        identity.client_build()
    );
    if displayed_identity
        .as_bytes()
        .windows(credential.len())
        .any(|window| window == credential)
    {
        return Err(ConfigError::CredentialFile {
            kind,
            problem: CredentialFileProblem::ExposedAsIdentity,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };

    use super::{ClientConfig, ConfigError, CredentialFileProblem};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    struct TestRepository(std::path::PathBuf);

    impl TestRepository {
        fn new(account: &[u8], password: &[u8]) -> Self {
            let unique = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "miazcore-config-test-{}-{unique}",
                std::process::id()
            ));
            let secrets = root.join("infra/azerothcore/secrets");
            fs::create_dir_all(&secrets).unwrap();
            write_secret(&secrets.join("fixture-account"), account);
            write_secret(&secrets.join("fixture-password"), password);
            Self(root)
        }
    }

    impl Drop for TestRepository {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    fn write_secret(path: &std::path::Path, value: &[u8]) {
        fs::write(path, value).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
        }
    }

    #[test]
    fn valid_reference_realm_configuration_loads_without_formatting_secrets() {
        let repository = TestRepository::new(b"ACCOUNT\n", b"super-secret-value\n");
        let loaded = ClientConfig::reference_realm(&repository.0)
            .unwrap()
            .load()
            .unwrap();
        let formatted = format!("{loaded:?}");

        assert_eq!(
            loaded.config().identity().realm_name(),
            "Miazcore Reference Realm"
        );
        assert_eq!(loaded.config().identity().character_name(), "Miaztest");
        assert_eq!(loaded.config().identity().client_build(), 12_340);
        assert!(!formatted.contains("ACCOUNT"));
        assert!(!formatted.contains("super-secret-value"));
        assert!(formatted.contains("[REDACTED]"));
    }

    #[cfg(unix)]
    #[test]
    fn insecure_permissions_fail_without_disclosing_file_contents() {
        use std::os::unix::fs::PermissionsExt;

        let repository = TestRepository::new(b"ACCOUNT\n", b"do-not-print-me\n");
        let password = repository
            .0
            .join("infra/azerothcore/secrets/fixture-password");
        fs::set_permissions(&password, fs::Permissions::from_mode(0o644)).unwrap();
        let error = ClientConfig::reference_realm(&repository.0)
            .unwrap()
            .load()
            .unwrap_err();
        let formatted = error.to_string();

        assert_eq!(
            error,
            ConfigError::CredentialFile {
                kind: super::CredentialFileKind::Password,
                problem: CredentialFileProblem::InsecurePermissions,
            }
        );
        assert!(!formatted.contains("do-not-print-me"));
    }

    #[test]
    fn invalid_credential_characters_are_rejected_without_echoing_values() {
        let repository = TestRepository::new(b"ACCOUNT\n", b"line\nbreak\n");
        let error = ClientConfig::reference_realm(&repository.0)
            .unwrap()
            .load()
            .unwrap_err();
        let formatted = error.to_string();

        assert!(matches!(
            error,
            ConfigError::CredentialFile {
                problem: CredentialFileProblem::InvalidCharacters,
                ..
            }
        ));
        assert!(!formatted.contains("line"));
        assert!(!formatted.contains("break"));
    }

    #[test]
    fn missing_empty_large_and_non_file_credentials_are_rejected() {
        let missing = TestRepository::new(b"ACCOUNT\n", b"secret\n");
        fs::remove_file(missing.0.join("infra/azerothcore/secrets/fixture-password")).unwrap();
        assert_credential_problem(&missing, CredentialFileProblem::Missing);

        let empty = TestRepository::new(b"ACCOUNT\n", b"");
        assert_credential_problem(&empty, CredentialFileProblem::Empty);

        let large = TestRepository::new(b"ACCOUNT\n", &[b'x'; 257]);
        assert_credential_problem(&large, CredentialFileProblem::TooLarge);

        let directory = TestRepository::new(b"ACCOUNT\n", b"secret\n");
        let password = directory
            .0
            .join("infra/azerothcore/secrets/fixture-password");
        fs::remove_file(&password).unwrap();
        fs::create_dir(&password).unwrap();
        assert_credential_problem(&directory, CredentialFileProblem::NotRegularFile);
    }

    #[cfg(unix)]
    #[test]
    fn symbolic_link_credentials_are_rejected() {
        use std::os::unix::fs::symlink;

        let repository = TestRepository::new(b"ACCOUNT\n", b"secret\n");
        let secrets = repository.0.join("infra/azerothcore/secrets");
        let password = secrets.join("fixture-password");
        fs::remove_file(&password).unwrap();
        symlink(secrets.join("fixture-account"), &password).unwrap();

        assert_credential_problem(&repository, CredentialFileProblem::Symlink);
    }

    #[test]
    fn credential_material_cannot_be_reused_as_display_identity() {
        let repository = TestRepository::new(b"ACCOUNT\n", b"Miaztest\n");
        let error = ClientConfig::reference_realm(&repository.0)
            .unwrap()
            .load()
            .unwrap_err();

        assert_eq!(
            error,
            ConfigError::CredentialFile {
                kind: super::CredentialFileKind::Password,
                problem: CredentialFileProblem::ExposedAsIdentity,
            }
        );
        assert!(!error.to_string().contains("Miaztest"));
    }

    fn assert_credential_problem(repository: &TestRepository, expected: CredentialFileProblem) {
        assert_eq!(
            ClientConfig::reference_realm(&repository.0)
                .unwrap()
                .load()
                .unwrap_err(),
            ConfigError::CredentialFile {
                kind: super::CredentialFileKind::Password,
                problem: expected,
            }
        );
    }
}
