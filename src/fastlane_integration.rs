use ethers::{
    abi::Abi,
    prelude::*,
    types::{Address, Bytes, H256, U256},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use anyhow::{Result, anyhow};
use log::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BundleStatus {
    Pending,
    Included,
    Failed,
    Timeout,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FastLaneBundle {
    pub transactions: Vec<FastLaneTransaction>,
    pub block_number: U64,
    pub min_timestamp: Option<U256>,
    pub max_timestamp: Option<U256>,
    pub reverting_tx_hashes: Vec<H256>,
    pub target_block: Option<U64>,
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
        let contract = self.get_fastlane_contract().await?;
        
        let call = contract.method::<_, H256>(
            "submitBundle",
            (
                bundle.transactions,
                bundle.block_number,
                bundle.min_timestamp,
                bundle.max_timestamp,
                bundle.reverting_tx_hashes,
            ),
        )?;

        let pending_tx = call.send().await?;
        let receipt = pending_tx.await?;

        match receipt {
            Some(r) => {
                info!("FastLane bundle submitted: {:?}", r.transaction_hash);
                Ok(r.transaction_hash)
            }
            None => Err(anyhow!("Failed to submit FastLane bundle"))
        }
    }

    pub async fn get_bundle_status(&self, bundle_hash: H256) -> Result<BundleStatus> {
        let contract = self.get_fastlane_contract().await?;
        let status: u8 = contract
            .method::<_, u8>("getBundleStatus", bundle_hash)?
            .call()
            .await?;
        
        Ok(match status {
            0 => BundleStatus::Pending,
            1 => BundleStatus::Included,
            2 => BundleStatus::Failed,
            _ => BundleStatus::Timeout,
        })
    }

    async fn get_fastlane_contract(&self) -> Result<Contract<Provider<Ws>>> {
        let abi: &[u8] = include_bytes!("../abis/FastLane.json");
        let abi: Abi = serde_json::from_slice(abi)?;
        
        Ok(Contract::new(
            self.fastlane_contract,
            abi,
            self.provider.clone()
        ))
    }

    pub async fn create_arbitrage_bundle(
        &self,
        opportunity: &ArbitrageOpportunity,
        gas_price: U256,
    ) -> Result<FastLaneBundle> {
        let current_block = self.provider.get_block_number().await?;
        
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
            block_number: current_block + 1,
            min_timestamp: None,
            max_timestamp: Some(U256::from(block.timestamp + 120)), // 2 minute timeout
            reverting_tx_hashes: vec![],
            target_block: Some(current_block + 1),
        })
    }

    async fn create_flash_loan_tx(
        &self,
        opportunity: &ArbitrageOpportunity,
        gas_price: U256,
    ) -> Result<Transaction> {
        let contract = Contract::new(
            opportunity.flash_loan_contract,
            include_bytes!("../abis/FlashLoan.json").as_ref(),
            self.provider.clone(),
        );

        let data = contract
            .method::<_, Bytes>(
                "executeFlashLoan",
                (
                    opportunity.token_in,
                    opportunity.token_out,
                    opportunity.amount_in,
                    opportunity.path.clone(),
                )
            )?
            .calldata()
            .unwrap();

        Ok(Transaction {
            to: Some(opportunity.flash_loan_contract),
            value: U256::zero(),
            gas_price: Some(gas_price),
            gas: U256::from(300000),
            data,
            nonce: None,
            ..Default::default()
        })
    }

    async fn create_arbitrage_tx(
        &self,
        opportunity: &ArbitrageOpportunity,
        gas_price: U256,
    ) -> Result<Transaction> {
        let contract = Contract::new(
            self.solver_contract,
            include_bytes!("../abis/Arbitrage.json").as_ref(),
            self.provider.clone(),
        );

        let data = contract
            .method::<_, Bytes>(
                "executeArbitrage",
                (
                    opportunity.path.clone(),
                    opportunity.amounts.clone(),
                    opportunity.routers.clone(),
                )
            )?
            .calldata()
            .unwrap();

        Ok(Transaction {
            to: Some(self.solver_contract),
            value: U256::zero(),
            gas_price: Some(gas_price),
            gas: U256::from(500000),
            data,
            nonce: None,
            ..Default::default()
        })
    }

    async fn create_repayment_tx(
        &self,
        opportunity: &ArbitrageOpportunity,
        gas_price: U256,
    ) -> Result<Transaction> {
        let contract = Contract::new(
            opportunity.flash_loan_contract,
            include_bytes!("../abis/FlashLoan.json").as_ref(),
            self.provider.clone(),
        );

        let data = contract
            .method::<_, Bytes>(
                "repayFlashLoan",
                (
                    opportunity.token_in,
                    opportunity.amount_in,
                )
            )?
            .calldata()
            .unwrap();

        Ok(Transaction {
            to: Some(opportunity.flash_loan_contract),
            value: U256::zero(),
            gas_price: Some(gas_price),
            gas: U256::from(200000),
            data,
            nonce: None,
            ..Default::default()
        })
    }
}
