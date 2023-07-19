use async_trait::async_trait;
use solana_address_lookup_table_program::state::AddressLookupTable;
use solana_client::{
    client_error::{ClientError, ClientErrorKind},
    nonblocking::rpc_client::RpcClient,
};
use solana_sdk::{address_lookup_table_account::AddressLookupTableAccount, message::v0, pubkey::Pubkey};

const ERR_PREFIX: &str = "SolanaClientsExtension";

#[async_trait]
pub trait LoadFromLookupTable {
    async fn load_address_lookup_table_accounts(
        &self,
        message_address_table_lookups: &[v0::MessageAddressTableLookup],
    ) -> Result<Vec<AddressLookupTableAccount>, ClientError>;

    async fn load_address_lookup_table_addresses(
        &self,
        message_address_table_lookups: &[v0::MessageAddressTableLookup],
    ) -> Result<v0::LoadedAddresses, ClientError>;
}

fn load_addresses(
    message_address_table_lookups: &[v0::MessageAddressTableLookup],
    address_table_lookup_accounts: &[AddressLookupTableAccount],
) -> v0::LoadedAddresses {
    address_table_lookup_accounts
        .iter()
        .zip(message_address_table_lookups)
        .map(|(account, lookup)| {
            let writable = lookup
                .writable_indexes
                .iter()
                .map(|&idx| account.addresses[idx as usize])
                .collect();
            let readonly = lookup
                .readonly_indexes
                .iter()
                .map(|&idx| account.addresses[idx as usize])
                .collect();
            v0::LoadedAddresses { writable, readonly }
        })
        .collect()
}

#[async_trait]
impl LoadFromLookupTable for RpcClient {
    async fn load_address_lookup_table_accounts(
        &self,
        message_address_table_lookups: &[v0::MessageAddressTableLookup],
    ) -> Result<Vec<AddressLookupTableAccount>, ClientError> {
        let address_table_lookup_addresses: Vec<Pubkey> = message_address_table_lookups
            .iter()
            .map(|lookup| lookup.account_key)
            .collect();

        let accounts = self
            .get_multiple_accounts(&address_table_lookup_addresses)
            .await?
            .into_iter()
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| {
                ClientError::from(ClientErrorKind::Custom(format!(
                    "{ERR_PREFIX}: AddressTableLookup account not found"
                )))
            })?;

        let address_lookup_tables = accounts
            .iter()
            .map(|account| AddressLookupTable::deserialize(&account.data))
            .collect::<Result<Vec<AddressLookupTable>, _>>()
            .map_err(|error| ClientError::from(ClientErrorKind::Custom(format!("{ERR_PREFIX}: {error}"))))?;

        let address_lookup_table_accounts = address_table_lookup_addresses
            .into_iter()
            .zip(address_lookup_tables.iter())
            .map(|(key, account)| AddressLookupTableAccount {
                key,
                addresses: account.addresses.to_vec(),
            })
            .collect();

        Ok(address_lookup_table_accounts)
    }

    async fn load_address_lookup_table_addresses(
        &self,
        message_address_table_lookups: &[v0::MessageAddressTableLookup],
    ) -> Result<v0::LoadedAddresses, ClientError> {
        let accounts = self
            .load_address_lookup_table_accounts(message_address_table_lookups)
            .await?;
        Ok(load_addresses(message_address_table_lookups, &accounts))
    }
}

#[cfg(test)]
mod tests {
    use core::panic;
    use std::str::FromStr;

    use solana_client::rpc_config::RpcTransactionConfig;
    use solana_sdk::{message::v0::LoadedAddresses, pubkey, signature::Signature};
    use solana_transaction_status::{EncodedTransaction, UiMessage, UiTransactionEncoding};

    use super::*;

    const RPC_URL: &str = "https://api.mainnet-beta.solana.com";

    #[tokio::test]
    async fn load_address_lookup_table_accounts_check() {
        let client = RpcClient::new(RPC_URL.to_string());

        let tx = client
            .get_transaction_with_config(
                &Signature::from_str(
                    "3f2NjDiyqPLXudcmacW44cmnvCg4pakSLjK5xe2cieSpHaU5G1fQ3MwUGtw5z59LJaKYjPVmkeDR8Pt8kdJTYiay",
                )
                .unwrap(),
                RpcTransactionConfig {
                    encoding: Some(UiTransactionEncoding::Json),
                    max_supported_transaction_version: Some(0),
                    commitment: None,
                },
            )
            .await
            .unwrap();

        let EncodedTransaction::Json(ui_tx) = tx.transaction.transaction else {
            panic!("Wrong encoding")
        };

        let UiMessage::Raw(msg) = ui_tx.message else {
            panic!("Wrong message")
        };

        let tables = msg
            .address_table_lookups
            .unwrap()
            .into_iter()
            .map(|x| v0::MessageAddressTableLookup {
                account_key: Pubkey::from_str(&x.account_key).unwrap(),
                writable_indexes: x.writable_indexes.clone(),
                readonly_indexes: x.readonly_indexes.clone(),
            })
            .collect::<Vec<_>>();

        let loaded_addresses = client.load_address_lookup_table_addresses(&tables).await.unwrap();

        let expect = LoadedAddresses {
            writable: vec![
                pubkey!("65sR8agQm768HYCjktunDJG3bbQszi7U8VD4pAKEYiXW"),
                pubkey!("G5Q7dTUPYw5pEXbfzZbAFSaPstP9bdFmEJ7XXcyrkxVJ"),
                pubkey!("EZd87x1Fu1ufV7pVRuXAEcL9y6aEMWWqtpcr7AHdU8ms"),
                pubkey!("7qbRF6YsyGuLUVs6Y1q64bdVrfe4ZcUUz1JRdoVNUJnm"),
                pubkey!("9RfZwn2Prux6QesG1Noo4HzMEBv3rPndJ2bN2Wwd6a7p"),
                pubkey!("BVNo8ftg2LkkssnWT4ZWdtoFaevnfD6ExYeramwM27pe"),
                pubkey!("8JqnWDVFfu9tgN5DXvvFXbWm2HmCEFHZfsWHS6FTyNoX"),
                pubkey!("BbsiNbFfJsRDwqF4JaiJ6sKecNuY4eWmEaDHcY6h6HuD"),
                pubkey!("3eVE92aEAsLYcBACXNu1yxoHVfTM8e8vmQC2zSApGRJX"),
                pubkey!("EsYaDKJCmcJtJHFuJYwQZwqohvVMCrFzcg8yo3i328No"),
                pubkey!("FWBCbjZnypLKz7uHGJXpBAEez2FurQXi9J3js7ZT1xDe"),
            ],
            readonly: vec![
                pubkey!("MERLuDFBMmsHnsBPZw2sDQZHvXFMwp8EdjudcU2HKky"),
                pubkey!("UST3iPxDFwUUiToMyLF7DYqSP9uaoz7Mzs2LxRYxVJG"),
                pubkey!("44K7k9pjjKB6LcWZ1sJ7TvksR3sb4AXpBxwzF1pcEJ5n"),
                pubkey!("whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc"),
                pubkey!("6vK8gSiRHSnZzAa5JsvBF2ej1LrxpRX21Y185CzP4PeA"),
                pubkey!("9W959DqEETiGZocYWCQPaJ6sBmUzgfxXfqGeTEdp3aQP"),
                pubkey!("68Bg6yQxWm3mrUYk3XzMiF5ycE41HwPhyEdaB1cp6wuo"),
                pubkey!("BpshqwEmPXmJwJfFgTFafmXoHN8Lc7SouvFsh6jyYQAm"),
            ],
        };

        assert_eq!(expect, loaded_addresses)
    }
}
