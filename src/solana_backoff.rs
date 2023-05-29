use std::time::Duration;

use backoff::ExponentialBackoff;
use futures::Future;
use solana_client::client_error::{ClientError, ClientErrorKind};

async fn call<I: std::fmt::Debug>(
    fut: impl Future<Output = Result<I, ClientError>>,
) -> Result<I, backoff::Error<ClientError>> {
    fut.await.map_err(|err| match &err.kind {
        ClientErrorKind::Io(_)
        | ClientErrorKind::Reqwest(_)
        | ClientErrorKind::RpcError(_)
        | ClientErrorKind::Custom(_) => {
            tracing::warn!(?err, "Transient error happened while SolanaRpc call");
            backoff::Error::transient(err)
        },
        ClientErrorKind::SerdeJson(_)
        | ClientErrorKind::SigningError(_)
        | ClientErrorKind::TransactionError(_)
        | ClientErrorKind::FaucetError(_) => backoff::Error::permanent(err),
    })
}

pub async fn call_with_backoff<I: std::fmt::Debug, Fut: Future<Output = Result<I, ClientError>>>(
    timeout: Option<Duration>,
    fut: impl Fn() -> Fut,
) -> Result<I, ClientError> {
    backoff::future::retry(
        ExponentialBackoff {
            max_elapsed_time: timeout,
            ..Default::default()
        },
        || async { call(fut()).await },
    )
    .await
}

pub async fn call_with_backoff_default_timeout<I: std::fmt::Debug, Fut: Future<Output = Result<I, ClientError>>>(
    fut: impl Fn() -> Fut,
) -> Result<I, ClientError> {
    call_with_backoff(Some(Duration::from_secs(30)), fut).await
}
