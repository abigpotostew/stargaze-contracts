# NFT PFP Minter Contract


half life dutch auction

start price = 10;


10, 5, 2.5, 1.25, 0.625, 0.3125, 0.15
0   5  10  15    20     25       30
over 30 minutes, 5 minute drop time
30/5 = 6 drops

0.15 * 2^6
9.6


---
https://www.paradigm.xyz/2022/04/gda

---


const DEFAULT_DUTCH_AUCTION_DECLINE_PERIOD_SECONDS: u64 = 300;
const DEFAULT_DUTCH_AUCTION_DECLINE_COEFFICIENT: u64 = 850000;

