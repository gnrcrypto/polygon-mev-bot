use ethers::{
    abi::Abi,
    prelude::*,
    types::{Address, Bytes, H256, U256},
};
use std::sync::Arc;
use anyhow::Result;

pub const UNISWAP_V3_ROUTER: &str = "0xE592427A0AEce92De3Edee1F18E0157C05861564";
pub const UNISWAP_V3_FACTORY: &str = "0x1F98431c8aD98523631AE4a59f267346ea31F984";
pub const DEFAULT_FEE: u32 = 3000; // 0.3%
pub const FEE_TIERS: [u32; 3] = [500, 3000, 10000];

#[derive(Debug, Clone)]
pub struct UniswapV3Router {
    pub address: Address,
    provider: Arc<Provider<Ws>>,
}

impl UniswapV3Router {
    pub fn new(provider: Arc<Provider<Ws>>) -> Self {
        Self {
            address: UNISWAP_V3_ROUTER.parse().unwrap(),
            provider,
        }
    }

    pub async fn exact_input_single(
        &self,
        params: ExactInputSingleParams,
    ) -> Result<Bytes> {
        let contract = Contract::new(
            self.address,
            include_bytes!("../../abis/UniswapV3Router.json").as_ref(),
            self.provider.clone(),
        );

        Ok(contract
            .method::<_, Bytes>("exactInputSingle", (params,))?
            .calldata()
            .unwrap())
    }
}

#[derive(Debug, Clone)]
pub struct ExactInputSingleParams {
    pub token_in: Address,
    pub token_out: Address,
    pub fee: u32,
    pub recipient: Address,
    pub deadline: U256,
    pub amount_in: U256,
    pub amount_out_minimum: U256,
    pub sqrt_price_limit_x96: U256,
}
