use ethers::{
    abi::Abi,
    prelude::*,
    types::{Address, Bytes, H256, U256},
};
use std::sync::Arc;
use anyhow::Result;

pub const QUICKSWAP_ROUTER: &str = "0xa5E0829CaCEd8fFDD4De3c43696c57F7D7A678ff";
pub const QUICKSWAP_FACTORY: &str = "0x5757371414417b8C6CAad45bAeF941aBc7d3Ab32";
pub const DEFAULT_FEE: u32 = 3000; // 0.3%

#[derive(Debug, Clone)]
pub struct QuickswapRouter {
    pub address: Address,
    provider: Arc<Provider<Ws>>,
}

impl QuickswapRouter {
    pub fn new(provider: Arc<Provider<Ws>>) -> Self {
        Self {
            address: QUICKSWAP_ROUTER.parse().unwrap(),
            provider,
        }
    }

    pub async fn get_amounts_out(
        &self,
        amount_in: U256,
        path: &[Address],
    ) -> Result<Vec<U256>> {
        let contract = Contract::new(
            self.address,
            include_bytes!("../../abis/QuickswapRouter.json").as_ref(),
            self.provider.clone(),
        );

        let amounts: Vec<U256> = contract
            .method::<_, Vec<U256>>("getAmountsOut", (amount_in, path.to_vec()))?
            .call()
            .await?;

        Ok(amounts)
    }

    pub async fn swap_exact_tokens_for_tokens(
        &self,
        amount_in: U256,
        amount_out_min: U256,
        path: Vec<Address>,
        to: Address,
        deadline: U256,
    ) -> Result<Bytes> {
        let contract = Contract::new(
            self.address,
            include_bytes!("../../abis/QuickswapRouter.json").as_ref(),
            self.provider.clone(),
        );

        Ok(contract
            .method::<_, Bytes>(
                "swapExactTokensForTokens",
                (amount_in, amount_out_min, path, to, deadline),
            )?
            .calldata()
            .unwrap())
    }
}
