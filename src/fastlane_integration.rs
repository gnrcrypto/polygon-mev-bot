// src/fastlane_integration.rs
use ethers::{
    abi::Abi,
    prelude::*,
    types::{Address, Bytes, H256, U256},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FastLaneBundle {
    pub transactions: Vec<FastLaneTransaction>,
    pub block_number: U64,
    pub min_timestamp: Option<U256>,
    pub max_timestamp: Option<U256>,
    pub reverting_tx_hashes: Vec<H256>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FastLaneTransaction {
    pub tx: Transaction,
    pub can_revert: bool,
}

#[derive(Debug, Clone)]
pub struct FastLaneClient {
    provider: Arc<Provider<Ws>>,
    fastlane_contract: Address,
    solver_contract: Address,
}

impl FastLaneClient {
    pub fn new(provider: Arc<Provider<Ws>>, fastlane_address: Address, solver_address: Address) -> Self {
        Self {
            provider,
            fastlane_contract: fastlane_address,
            solver_contract: solver_address,
        }
    }

    pub async fn submit_bundle(&self, bundle: FastLaneBundle) -> Result<H256> {
        // Implement FastLane bundle submission
        let contract = self.get_fastlane_contract().await?;
        
        let call = contract.submit_bundle(
            bundle.transactions,
            bundle.block_number,
            bundle.min_timestamp,
            bundle.max_timestamp,
            bundle.reverting_tx_hashes,
        );

        let pending_tx = call.send().await?;
        let receipt = pending_tx.await?;

        if let Some(receipt) = receipt {
            info!("FastLane bundle submitted: {:?}", receipt.transaction_hash);
            return Ok(receipt.transaction_hash);
        }

        Err(anyhow!("Failed to submit FastLane bundle"))
    }

    pub async fn get_bundle_status(&self, bundle_hash: H256) -> Result<BundleStatus> {
        // Check bundle status
        let contract = self.get_fastlane_contract().await?;
        let status = contract.get_bundle_status(bundle_hash).call().await?;
        
        Ok(status)
    }

    async fn get_fastlane_contract(&self) -> Result<ContractInstance<Provider<Ws>, Provider<Ws>>> {
        let abi = include_bytes!("../abis/FastLane.json");
        let abi: Abi = serde_json::from_slice(abi)?;
        
        Ok(Contract::new(self.fastlane_contract, abi, self.provider.clone()))
    }

    pub async fn create_arbitrage_bundle(
        &self,
        opportunity: &ArbitrageOpportunity,
        gas_price: U256,
    ) -> Result<FastLaneBundle> {
        // Create optimized bundle for FastLane
        let flash_loan_tx = self.create_flash_loan_tx(opportunity, gas_price).await?;
        let arbitrage_tx = self.create_arbitrage_tx(opportunity, gas_price).await?;
        let repayment_tx = self.create_repayment_tx(opportunity, gas_price).await?;

        let transactions = vec![
            FastLaneTransaction {
                tx: flash_loan_tx,
                can_revert: false,
            },
            FastLaneTransaction {
                tx: arbitrage_tx,
                can_revert: true,
            },
            FastLaneTransaction {
                tx: repayment_tx,
                can_revert: false,
            },
        ];

        Ok(FastLaneBundle {
            transactions,
            block_number: self.provider.get_block_number().await?,
            min_timestamp: None,
            max_timestamp: None,
            reverting_tx_hashes: vec![],
        })
    }

    async fn create_flash_loan_tx(
        &self,
        opportunity: &ArbitrageOpportunity,
        gas_price: U256,
    ) -> Result<Transaction> {
        // Create flash loan transaction
        Ok(Transaction {
            to: Some(opportunity.flash_loan_contract),
            value: U256::zero(),
            gas_price,
            gas: U256::from(300000),
            input: Bytes::from("flash_loan_call_data"),
            ..Default::default()
        })
    }

    async fn create_arbitrage_tx(
        &self,
        opportunity: &ArbitrageOpportunity,
        gas_price: U256,
    ) -> Result<Transaction> {
        // Create arbitrage execution transaction
        Ok(Transaction {
            to: Some(self.solver_contract),
            value: U256::zero(),
            gas_price,
            gas: U256::from(500000),
            input: Bytes::from("arbitrage_execution_data"),
            ..Default::default()
        })
    }

    async fn create_repayment_tx(
        &self,
        opportunity: &ArbitrageOpportunity,
        gas_price: U256,
    ) -> Result<Transaction> {
        // Create flash loan repayment transaction
        Ok(Transaction {
            to: Some(opportunity.flash_loan_contract),
            value: U256::zero(),
            gas_price,
            gas: U256::from(200000),
            input: Bytes::from("repayment_data"),
            ..Default::default()
        })
    }
}
