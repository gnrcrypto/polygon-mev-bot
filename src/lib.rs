use ethers::prelude::*;
use std::sync::Arc;

abigen!(
    FlashLoanArbitrage,
    "./abis/FlashLoanArbitrage.json",
    event_derives(serde::Serialize, serde::Deserialize)
);

abigen!(
    FastLaneSender,
    "./abis/FastLaneSender.json",
    event_derives(serde::Serialize, serde::Deserialize)
);
