use std::sync::Arc;

use crate::{
    providers::{PredeployedClientProvider, RpcClientProvider},
    transaction_waiter::TransactionWaiter,
};
use starknet::{
    accounts::{Account, SingleOwnerAccount},
    contract::ContractFactory,
    core::types::{
        contract::{CompiledClass, SierraClass},
        DeclareTransactionResult, FieldElement, InvokeTransactionResult,
    },
    providers::{JsonRpcClient, Provider},
    signers::Signer,
};

pub const SIERRA_STR: &str = include_str!(
    "../../cartridge_account/target/dev/cartridge_account_Account.contract_class.json"
);
pub const CASM_STR: &str = include_str!(
    "../../cartridge_account/target/dev/cartridge_account_Account.compiled_contract_class.json"
);

pub async fn declare_contract<T, P, S>(
    rpc_provider: &impl RpcClientProvider<T>,
    account: SingleOwnerAccount<P, S>,
) -> Result<DeclareTransactionResult, String>
where
    JsonRpcClient<T>: Provider,
    T: Send + Sync,
    P: Provider + Send + Sync,
    S: Signer + Send + Sync,
{
    // Sierra class artifact. Output of the `starknet-compile` command
    let contract_artifact: SierraClass =
        serde_json::from_str(SIERRA_STR).map_err(|e| e.to_string())?;

    // Class hash of the compiled CASM class from the `starknet-sierra-compile` command
    let compiled_class: CompiledClass =
        serde_json::from_str(CASM_STR).map_err(|e| e.to_string())?;
    let casm_class_hash = compiled_class.class_hash().map_err(|e| e.to_string())?;

    // We need to flatten the ABI into a string first
    let flattened_class = contract_artifact.flatten().map_err(|e| e.to_string())?;

    let declaration_result = account
        .declare(Arc::new(flattened_class), casm_class_hash)
        .send()
        .await
        .unwrap();

    TransactionWaiter::new(
        declaration_result.transaction_hash,
        &rpc_provider.get_client(),
    )
    .await
    .unwrap();

    Ok(declaration_result)
}

#[derive(Debug, Clone)]
pub struct DeployResult {
    pub deployed_address: FieldElement,
    pub transaction_hash: FieldElement,
}

pub async fn deploy_contract<T, P, S>(
    client_provider: &(impl RpcClientProvider<T> + PredeployedClientProvider),
    constructor_calldata: Vec<FieldElement>,
    salt: FieldElement,
    account: SingleOwnerAccount<P, S>,
    class_hash: FieldElement,
) -> Result<DeployResult, String>
where
    P: Provider + Send + Sync,
    S: Signer + Send + Sync,
    JsonRpcClient<T>: Provider,
    T: Send + Sync,
{
    let contract_factory = ContractFactory::new_with_udc(
        class_hash,
        account,
        client_provider.predeployed_udc().address,
    );

    let deployment = contract_factory.deploy(constructor_calldata, salt, false);
    let deployed_address = deployment.deployed_address();
    let InvokeTransactionResult { transaction_hash } =
        deployment.send().await.expect("Unable to deploy contract");

    TransactionWaiter::new(transaction_hash, &client_provider.get_client())
        .await
        .unwrap();

    Ok(DeployResult {
        deployed_address,
        transaction_hash,
    })
}
