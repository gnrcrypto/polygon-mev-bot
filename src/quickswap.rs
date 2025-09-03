use log::info;
use ethers::prelude::*;
use ethers::abi::{Abi, AbiParser, FunctionExt, Token};
use once_cell::sync::Lazy;

// ---- QuickSwap Polygon addresses ----
pub static QUICKSWAP_ROUTER_ADDR: Lazy<Address> = Lazy::new(|| {
    "0xa5E0829CaCEd8fFDD4De3c43696c57F7D7A678ff".parse().unwrap()
});
pub static QUICKSWAP_FACTORY_ADDR: Lazy<Address> = Lazy::new(|| {
    "0x5757371414417b8c6caad45baef941abc7d3ab32".parse().unwrap()
});
// Minimal set of swap entrypoints we care about (you can add/remove)
pub static QUICKSWAP_ROUTER_ABI: Lazy<Abi> = Lazy::new(|| {
    // Human-readable signatures parsed at startup
    AbiParser::default().parse(&[
        // exact-in
        "function swapExactTokensForTokens(uint256,uint256,address[],address,uint256) returns (uint256[])",
        "function swapExactTokensForETH(uint256,uint256,address[],address,uint256) returns (uint256[])",
        "function swapExactETHForTokens(uint256,address[],address,uint256) returns (uint256[])",
        // exact-out
        "function swapTokensForExactTokens(uint256,uint256,address[],address,uint256) returns (uint256[])",
        "function swapETHForExactTokens(uint256,address[],address,uint256) returns (uint256[])",
        "function swapTokensForExactETH(uint256,uint256,address[],address,uint256) returns (uint256[])",
        // fee-on-transfer supporting variants
        "function swapExactTokensForTokensSupportingFeeOnTransferTokens(uint256,uint256,address[],address,uint256)",
        "function swapExactTokensForETHSupportingFeeOnTransferTokens(uint256,uint256,address[],address,uint256)",
        "function swapExactETHForTokensSupportingFeeOnTransferTokens(uint256,address[],address,uint256)",
    ]).expect("parse quickswap abi")
});

// Polygon mains
pub const WMATIC: &str = "0x0d500B1d8E8eF31E21C99d1Db9A6444d3ADf1270";
pub const USDC_E: &str = "0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174";


// ---------------------------------------------------------------------------------------
// Minimal ABIs
abigen!(IUniswapV2Factory, r#"[
    function getPair(address tokenA, address tokenB) external view returns (address)
]"#);

abigen!(IUniswapV2Pair, r#"[
    function token0() external view returns (address)
    function token1() external view returns (address)
    function getReserves() external view returns (uint112 reserve0, uint112 reserve1, uint32 blockTimestampLast)
]"#);

#[derive(Debug, Clone)]
pub enum QuickSwapAction {
    // exact-in (input is fixed, output >= min)
    SwapExactTokensForTokens {
        amount_in: U256,
        amount_out_min: U256,
        path: Vec<Address>,
        to: Address,
        deadline: U256,
    },
    SwapExactTokensForETH {
        amount_in: U256,
        amount_out_min: U256,
        path: Vec<Address>,
        to: Address,
        deadline: U256,
    },
    SwapExactETHForTokens {
        // amount_in is tx.value
        amount_in: U256,
        amount_out_min: U256,
        path: Vec<Address>,
        to: Address,
        deadline: U256,
    },

    // exact-out (output is fixed, input <= max)
    SwapTokensForExactTokens {
        amount_out: U256,
        amount_in_max: U256,
        path: Vec<Address>,
        to: Address,
        deadline: U256,
    },
    SwapTokensForExactETH {
        amount_out: U256,
        amount_in_max: U256,
        path: Vec<Address>,
        to: Address,
        deadline: U256,
    },
    SwapETHForExactTokens {
        // amount_in_max is tx.value
        amount_out: U256,
        path: Vec<Address>,
        to: Address,
        deadline: U256,
        amount_in_max: U256,
    },

    // supporting-fee-on-transfer (exact-in semantics but no amounts[] return)
    SwapExactTokensForTokensSupportingFeeOnTransferTokens {
        amount_in: U256,
        amount_out_min: U256,
        path: Vec<Address>,
        to: Address,
        deadline: U256,
    },
    SwapExactTokensForETHSupportingFeeOnTransferTokens {
        amount_in: U256,
        amount_out_min: U256,
        path: Vec<Address>,
        to: Address,
        deadline: U256,
    },
    SwapExactETHForTokensSupportingFeeOnTransferTokens {
        amount_in: U256, // tx.value
        amount_out_min: U256,
        path: Vec<Address>,
        to: Address,
        deadline: U256,
    },
}

impl QuickSwapAction {
    pub fn get_path(&self)->Vec<Address>{
        match self {
            QuickSwapAction::SwapExactTokensForTokens { path, .. }
            | QuickSwapAction::SwapExactTokensForETH { path, .. }
            | QuickSwapAction::SwapExactETHForTokens { path, .. }
            | QuickSwapAction::SwapTokensForExactTokens { path, .. }
            | QuickSwapAction::SwapTokensForExactETH { path, .. }
            | QuickSwapAction::SwapETHForExactTokens { path, .. }
            | QuickSwapAction::SwapExactTokensForTokensSupportingFeeOnTransferTokens { path, .. }
            | QuickSwapAction::SwapExactTokensForETHSupportingFeeOnTransferTokens { path, .. }
            | QuickSwapAction::SwapExactETHForTokensSupportingFeeOnTransferTokens { path, .. } => {
                path.clone()
            }
        }
    }
}
/// Decode a QuickSwap router call from a tx. Returns `None` if not a QS call or unknown selector.
pub fn parse_quickswap_tx(tx: &Transaction) -> Option<QuickSwapAction> {
    if tx.to != Some(*QUICKSWAP_ROUTER_ADDR) {
        return None;
    }
    let input = &tx.input.0;
    if input.len() < 4 { return None; }
    let selector = &input[..4];

    // Find the matching function in our ABI
    for f in QUICKSWAP_ROUTER_ABI.functions() {
        if f.selector() == selector {
            // Decode the calldata (skip selector)
            let tokens = f.decode_input(&input[4..]).ok()?;
            let name = f.name.as_str();

            // Helper to map Token::Array(Address) -> Vec<Address>
            fn to_addr_vec(t: &Token) -> Option<Vec<Address>> {
                match t {
                    Token::Array(v) => {
                        let mut out = Vec::with_capacity(v.len());
                        for x in v {
                            if let Token::Address(a) = x { out.push(*a); } else { return None; }
                        }
                        Some(out)
                    }
                    _ => None
                }
            }
            // Helper to get U256
            fn to_u256(t: &Token) -> Option<U256> {
                match t { Token::Uint(u) => Some(*u), _ => None }
            }
            // Helper to get Address
            fn to_addr(t: &Token) -> Option<Address> {
                match t { Token::Address(a) => Some(*a), _ => None }
            }            
            match name {
                // --- exact in ---
                "swapExactTokensForTokens" => {
                    // (amountIn, amountOutMin, path, to, deadline)
                    if tokens.len() != 5 { return None; }
                    return Some(QuickSwapAction::SwapExactTokensForTokens {
                        amount_in:      to_u256(&tokens[0])?,
                        amount_out_min: to_u256(&tokens[1])?,
                        path:           to_addr_vec(&tokens[2])?,
                        to:             to_addr(&tokens[3])?,
                        deadline:       to_u256(&tokens[4])?,
                    });
                }
                "swapExactTokensForETH" => {
                    if tokens.len() != 5 { return None; }
                    return Some(QuickSwapAction::SwapExactTokensForETH {
                        amount_in:      to_u256(&tokens[0])?,
                        amount_out_min: to_u256(&tokens[1])?,
                        path:           to_addr_vec(&tokens[2])?,
                        to:             to_addr(&tokens[3])?,
                        deadline:       to_u256(&tokens[4])?,
                    });
                }
                "swapExactETHForTokens" => {
                    // (amountOutMin, path, to, deadline); amountIn is tx.value
                    if tokens.len() != 4 { return None; }
                    return Some(QuickSwapAction::SwapExactETHForTokens {
                        amount_in:      tx.value, // from msg.value
                        amount_out_min: to_u256(&tokens[0])?,
                        path:           to_addr_vec(&tokens[1])?,
                        to:             to_addr(&tokens[2])?,
                        deadline:       to_u256(&tokens[3])?,
                    });
                }

                // --- exact out ---
                "swapTokensForExactTokens" => {
                    // (amountOut, amountInMax, path, to, deadline)
                    if tokens.len() != 5 { return None; }
                    return Some(QuickSwapAction::SwapTokensForExactTokens {
                        amount_out:   to_u256(&tokens[0])?,
                        amount_in_max:to_u256(&tokens[1])?,
                        path:         to_addr_vec(&tokens[2])?,
                        to:           to_addr(&tokens[3])?,
                        deadline:     to_u256(&tokens[4])?,
                    });
                }
                "swapTokensForExactETH" => {
                    if tokens.len() != 5 { return None; }
                    return Some(QuickSwapAction::SwapTokensForExactETH {
                        amount_out:    to_u256(&tokens[0])?,
                        amount_in_max: to_u256(&tokens[1])?,
                        path:          to_addr_vec(&tokens[2])?,
                        to:            to_addr(&tokens[3])?,
                        deadline:      to_u256(&tokens[4])?,
                    });
                }
                "swapETHForExactTokens" => {
                    // (amountOut, path, to, deadline); input cap is tx.value
                    if tokens.len() != 4 { return None; }
                    return Some(QuickSwapAction::SwapETHForExactTokens {
                        amount_out:    to_u256(&tokens[0])?,
                        path:          to_addr_vec(&tokens[1])?,
                        to:            to_addr(&tokens[2])?,
                        deadline:      to_u256(&tokens[3])?,
                        amount_in_max: tx.value,
                    });
                }

                // --- fee-on-transfer variants (exact in) ---
                "swapExactTokensForTokensSupportingFeeOnTransferTokens" => {
                    if tokens.len() != 5 { return None; }
                    return Some(QuickSwapAction::SwapExactTokensForTokensSupportingFeeOnTransferTokens {
                        amount_in:      to_u256(&tokens[0])?,
                        amount_out_min: to_u256(&tokens[1])?,
                        path:           to_addr_vec(&tokens[2])?,
                        to:             to_addr(&tokens[3])?,
                        deadline:       to_u256(&tokens[4])?,
                    });
                }
                "swapExactTokensForETHSupportingFeeOnTransferTokens" => {
                    if tokens.len() != 5 { return None; }
                    return Some(QuickSwapAction::SwapExactTokensForETHSupportingFeeOnTransferTokens {
                        amount_in:      to_u256(&tokens[0])?,
                        amount_out_min: to_u256(&tokens[1])?,
                        path:           to_addr_vec(&tokens[2])?,
                        to:             to_addr(&tokens[3])?,
                        deadline:       to_u256(&tokens[4])?,
                    });
                }
                "swapExactETHForTokensSupportingFeeOnTransferTokens" => {
                    if tokens.len() != 4 { return None; }
                    return Some(QuickSwapAction::SwapExactETHForTokensSupportingFeeOnTransferTokens {
                        amount_in:      tx.value,
                        amount_out_min: to_u256(&tokens[0])?,
                        path:           to_addr_vec(&tokens[1])?,
                        to:             to_addr(&tokens[2])?,
                        deadline:       to_u256(&tokens[3])?,
                    });
                }

                // Unknown (not in our minimal ABI list)
                _ =>{
                    info!("Quickswap tx not in our abi list");
                    return None

                } 
            }
        }
    }
    None
}
