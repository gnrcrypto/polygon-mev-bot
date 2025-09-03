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
pub struct FastLaneBundle {
    pub data: Bytes,
    pub target_block: U64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BundleStatus {
    Pending,
    Included,
    Failed,
    Timeout,
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
        let contract = Contract::new(
            self.fastlane_contract,
            include_bytes!("../abis/FastLaneSender.json").as_ref(),
            self.provider.clone(),
        );
        
        let call = contract.method::<_, H256>(
            "sendTransaction",
            (bundle.data, bundle.target_block)
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
        let contract = Contract::new(
            self.fastlane_contract,
            include_bytes!("../abis/FastLaneSender.json").as_ref(),
            self.provider.clone(),
        );

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

    pub async fn create_arbitrage_bundle(
        &self,
        opportunity: &ArbitrageOpportunity,
        gas_price: U256,
    ) -> Result<FastLaneBundle> {
        let current_block = self.provider.get_block_number().await?;
        
        // Create the calldata for our combined solver/arbitrage contract
        let contract = Contract::new(
            self.solver_contract,
            include_bytes!("../abis/FlashLoanArbitrage.json").as_ref(),
            self.provider.clone(),
        );

        let data = contract
            .method::<_, Bytes>(
                "executeFlashLoanArbitrage",
                (
                    opportunity.token0,
                    opportunity.token1,
                    opportunity.amount0,
                    opportunity.amount1,
                    opportunity.fee,
                    opportunity.path.clone(),
                    opportunity.amounts.clone(),
                    opportunity.routers.clone(),
                )
            )?
            .calldata()
            .unwrap();

        Ok(FastLaneBundle {
            data,
            target_block: current_block + 1,
        })
    }

    // Helper function for preparing the bundle data
    pub async fn prepare_bundle_data(
        &self,
        opportunity: &ArbitrageOpportunity,
        target_block: U64,
    ) -> Result<FastLaneBundle> {
        let contract = Contract::new(
            self.solver_contract,
            include_bytes!("../abis/FlashLoanArbitrage.json").as_ref(),
            self.provider.clone(),
        );

        let data = contract
            .method::<_, Bytes>(
                "executeFlashLoanArbitrage",
                (
                    opportunity.token0,
                    opportunity.token1,
                    opportunity.amount0,
                    opportunity.amount1,
                    opportunity.fee,
                    opportunity.path.clone(),
                    opportunity.amounts.clone(),
                    opportunity.routers.clone(),
                )
            )?
            .calldata()
            .unwrap();

        Ok(FastLaneBundle {
            data,
            target_block,
        })
    }

    // Helper function to validate bundle parameters
    pub fn validate_bundle_params(&self, target_block: U64, current_block: U64) -> Result<()> {
        if target_block <= current_block {
            return Err(anyhow!("Target block must be in the future"));
        }
        if target_block > current_block + 5 {
            return Err(anyhow!("Target block too far in the future"));
        }
        Ok(())
    }
}

// Add ArbitrageOpportunity struct to match our contract
#[derive(Debug, Clone)]
pub struct ArbitrageOpportunity {
    pub token0: Address,
    pub token1: Address,
    pub amount0: U256,
    pub amount1: U256,
    pub fee: u32,
    pub path: Vec<Address>,
    pub amounts: Vec<U256>,
    pub routers: Vec<Address>,
}
