use ethers::{
    abi::Abi,
    prelude::*,
    types::{Address, Bytes, H256, U256},
};
use std::sync::Arc;
use anyhow::Result;

pub const SUSHISWAP_ROUTER: &str = "0x1b02dA8Cb0d097eB8D57A175b88c7D8b47997506";
pub const SUSHISWAP_FACTORY: &str = "0xc35DADB65012eC5796536bD9864eD8773aBc74C4";
pub const DEFAULT_FEE: u32 = 3000; // 0.3%

#[derive(Debug, Clone)]
pub struct SushiswapRouter {
    pub address: Address,
    provider: Arc<Provider<Ws>>,
}

impl SushiswapRouter {
    pub fn new(provider: Arc<Provider<Ws>>) -> Self {
        Self {
            address: SUSHISWAP_ROUTER.parse().unwrap(),
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
            include_bytes!("../../abis/SushiswapRouter.json").as_ref(),
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
            include_bytes!("../../abis/SushiswapRouter.json").as_ref(),
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
