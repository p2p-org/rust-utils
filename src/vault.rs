use anyhow::Result;
use reqwest::Client;
use std::{collections::HashMap, env};
use tokio::fs;

pub async fn init_env() -> Result<()> {
    VaultClient::default().init_env_from_secret().await
}

struct VaultClient {
    base_url: String,
    client: Client,
}

impl Default for VaultClient {
    fn default() -> Self {
        match env::var("VAULT_ADDR") {
            Ok(vault_addr) => Self::new(vault_addr),
            Err(_) => Self::new("http://vault.vault.svc.cluster.local:8200"),
        }
    }
}

impl VaultClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            client: Client::new(),
        }
    }

    pub async fn read_k8s_token(&self) -> Result<String> {
        static K8S_SERVICEACCOUNT_TOKEN_PATH: &str = "/var/run/secrets/kubernetes.io/serviceaccount/token";
        let k8s_serviceaccount_token = fs::read_to_string(K8S_SERVICEACCOUNT_TOKEN_PATH).await?;
        Ok(k8s_serviceaccount_token)
    }

    pub async fn k8s_login(&self, role: &str, jwt: &str) -> Result<String> {
        #[derive(serde::Serialize)]
        struct Request<'a> {
            role: &'a str,
            jwt: &'a str,
        }

        #[derive(serde::Deserialize)]
        struct Response {
            auth: Auth,
        }
        #[derive(serde::Deserialize)]
        struct Auth {
            client_token: String,
        }

        let Response {
            auth: Auth { client_token },
        } = self
            .client
            .post(format!("{}/v1/auth/kubernetes/login", self.base_url))
            .json(&Request { role, jwt })
            .send()
            .await?
            .json()
            .await?;

        Ok(client_token)
    }

    pub async fn read_secret(&self, vault_token: &str, secret_mount_path: &str) -> Result<HashMap<String, String>> {
        #[derive(serde::Deserialize)]
        struct Response {
            data: Data,
        }
        #[derive(serde::Deserialize)]
        struct Data {
            data: HashMap<String, String>,
        }

        let Response { data: Data { data } } = self
            .client
            .get(format!("{}/v1/kv/data/{secret_mount_path}", self.base_url))
            .header("X-Vault-Token", vault_token)
            .send()
            .await?
            .json()
            .await?;

        Ok(data)
    }

    pub async fn k8s_login_and_read_secret(
        &self,
        role: &str,
        secret_mount_path: &str,
    ) -> Result<HashMap<String, String>> {
        let k8s_serviceaccount_token = self.read_k8s_token().await?;
        let vault_token = self.k8s_login(role, &k8s_serviceaccount_token).await?;
        let secret = self.read_secret(&vault_token, secret_mount_path).await?;
        Ok(secret)
    }

    pub fn setup_env<'a, 'b: 'a>(&self, data: impl IntoIterator<Item = (&'a String, &'a String)> + 'b) {
        for (key, value) in data {
            if !key.is_empty() {
                env::set_var(key, value);
            }
        }
    }

    pub async fn setup_env_from_secret(&self, role: &str, secret_mount_path: &str) -> Result<()> {
        let secret = self.k8s_login_and_read_secret(role, secret_mount_path).await?;
        self.setup_env(&secret);
        Ok(())
    }

    pub async fn init_env_from_secret(&self) -> Result<()> {
        if let (Ok(role), Ok(secret_mount_path)) = (env::var("VAULT_ROLE"), env::var("VAULT_SECRET_MOUNT_PATH")) {
            self.setup_env_from_secret(&role, &secret_mount_path).await?;
        }
        Ok(())
    }

    #[cfg(feature = "settings")]
    pub async fn read_config_from_secret<'de, T: serde::Deserialize<'de>>(&self, prefix: &str, role: &str, secret_mount_path: &str) -> Result<T> {
        let settings = self.k8s_login_and_read_secret(role, secret_mount_path).await?;
        Ok(config::Config::builder()
            .add_source(config::Environment::with_prefix(prefix).source(Some(settings)))
            .build()?
            .try_deserialize()?)
    }

    #[cfg(feature = "settings")]
    pub async fn init_config_from_secret<'de, T: serde::Deserialize<'de>>(&self, prefix: &str) -> Result<T> {
        let source = config::Environment::with_prefix(prefix);
        let source = if let (Ok(role), Ok(secret_mount_path)) = (env::var("VAULT_ROLE"), env::var("VAULT_SECRET_MOUNT_PATH")) {
            source.source(Some(self.k8s_login_and_read_secret(role, secret_mount_path).await?))
        } else {
            source
        };
        Ok(config::Config::builder().add_source(source).build()?.try_deserialize()?)
    }
}
