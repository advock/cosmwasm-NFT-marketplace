use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, BlockInfo, Coin, Decimal, Timestamp, Uint128};
use cw_storage_plus::{Index, IndexList, IndexedMap, Item, Map, MultiIndex};
use cw_utils::Duration;
use sg_controllers::Hooks;

use crate::helpers::ExpiryRange;
use std::fmt;

pub type TokenId = u32;

#[cw_serde]
pub struct SudoParams {
    /// Fair Burn fee for winning bids
    pub trading_fee_percent: Decimal,
    /// Valid time range for Asks
    /// (min, max) in seconds
    pub ask_expiry: ExpiryRange,
    /// Valid time range for Bids
    /// (min, max) in seconds
    pub bid_expiry: ExpiryRange,
    /// Operators are entites that are responsible for maintaining the active state of Asks
    /// They listen to NFT transfer events, and update the active state of Asks
    pub operators: Vec<Addr>,
    /// Max value for the finders fee
    pub max_finders_fee_percent: Decimal,
    /// Min value for a bid
    pub min_price: Uint128,
    /// Duration after expiry when a bid becomes stale
    pub stale_bid_duration: Duration,
    /// Stale bid removal reward
    pub bid_removal_reward_percent: Decimal,
    /// Listing fee to reduce spam
    pub listing_fee: Uint128,
}

pub const SUDO_PARAMS: Item<SudoParams> = Item::new("sudo-params");

pub const ASK_HOOKS: Hooks = Hooks::new("ask-hooks");
pub const BID_HOOKS: Hooks = Hooks::new("bid-hooks");
pub const SALE_HOOKS: Hooks = Hooks::new("sale-hooks");

pub trait Order {
    fn expires_at(&self) -> Timestamp;

    fn is_expired(&self, block: &BlockInfo) -> bool {
        self.expires_at() <= block.time
    }
}

#[cw_serde]
pub enum SaleType {
    FixedPrice,
    Auction,
}

impl fmt::Display for SaleType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            SaleType::FixedPrice => write!(f, "fixed_price"),
            SaleType::Auction => write!(f, "auction"),
        }
    }
}

#[cw_serde]
pub struct Ask {
    pub sale_type: SaleType,
    pub collection: Addr,
    pub token_id: TokenId,
    pub seller: Addr,
    pub price: Uint128,
    pub funds_recipient: Option<Addr>,
    pub reserve_for: Option<Addr>,
    pub finders_fee_bps: Option<u64>,
    pub expires_at: Timestamp,
    pub is_active: bool,
}
impl Order for Ask {
    fn expires_at(&self) -> Timestamp {
        self.expires_at
    }
}
/// Primary key for asks: (collection, token_id)
pub type AskKey = (Addr, TokenId);
/// Convenience ask key constructor
pub fn ask_key(collection: &Addr, token_id: TokenId) -> AskKey {
    (collection.clone(), token_id)
}

/// Defines indices for accessing Asks
pub struct AskIndicies<'a> {
    pub collection: MultiIndex<'a, Addr, Ask, AskKey>,
    pub collection_price: MultiIndex<'a, (Addr, u128), Ask, AskKey>,
    pub seller: MultiIndex<'a, Addr, Ask, AskKey>,
}

impl<'a> IndexList<Ask> for AskIndicies<'a> {
    fn get_indexes(&'_ self) -> Box<dyn Iterator<Item = &'_ dyn Index<Ask>> + '_> {
        let v: Vec<&dyn Index<Ask>> = vec![&self.collection, &self.collection_price, &self.seller];
        Box::new(v.into_iter())
    }
}

pub fn asks<'a>() -> IndexedMap<'a, AskKey, Ask, AskIndicies<'a>> {
    let indexes = AskIndicies {
        collection: MultiIndex::new(
            |_pk: &[u8], d: &Ask| d.collection.clone(),
            "asks",
            "asks__collection",
        ),
        collection_price: MultiIndex::new(
            |_pk: &[u8], d: &Ask| (d.collection.clone(), d.price.u128()),
            "asks",
            "asks__collection_price",
        ),
        seller: MultiIndex::new(
            |_pk: &[u8], d: &Ask| d.seller.clone(),
            "asks",
            "asks__seller",
        ),
    };
    IndexedMap::new("asks", indexes)
}

#[cw_serde]
pub struct Bid {
    pub collection: Addr,
    pub token_id: TokenId,
    pub bidder: Addr,
    pub price: Uint128,
    pub finders_fee_bps: Option<u64>,
    pub expires_at: Timestamp,
}

impl Bid {
    pub fn new(
        collection: Addr,
        token_id: TokenId,
        bidder: Addr,
        price: Uint128,
        finders_fee_bps: Option<u64>,
        expires: Timestamp,
    ) -> Self {
        Bid {
            collection,
            token_id,
            bidder,
            price,
            finders_fee_bps,
            expires_at: expires,
        }
    }
}

impl Order for Bid {
    fn expires_at(&self) -> Timestamp {
        self.expires_at
    }
}

/// Primary key for bids: (collection, token_id, bidder)
pub type BidKey = (Addr, TokenId, Addr);
/// Convenience bid key constructor
pub fn bid_key(collection: &Addr, token_id: TokenId, bidder: &Addr) -> BidKey {
    (collection.clone(), token_id, bidder.clone())
}

pub struct BidIndicies<'a> {
    pub collection: MultiIndex<'a, Addr, Bid, BidKey>,
    pub collection_token_id: MultiIndex<'a, (Addr, TokenId), Bid, BidKey>,
    pub collection_price: MultiIndex<'a, (Addr, u128), Bid, BidKey>,
    pub bidder: MultiIndex<'a, Addr, Bid, BidKey>,
    // Cannot include `Timestamp` in index, converted `Timestamp` to `seconds` and stored as `u64`
    pub bidder_expires_at: MultiIndex<'a, (Addr, u64), Bid, BidKey>,
}

impl<'a> IndexList<Bid> for BidIndicies<'a> {
    fn get_indexes(&'_ self) -> Box<dyn Iterator<Item = &'_ dyn Index<Bid>> + '_> {
        let v: Vec<&dyn Index<Bid>> = vec![
            &self.collection,
            &self.collection_token_id,
            &self.collection_price,
            &self.bidder,
            &self.bidder_expires_at,
        ];
        Box::new(v.into_iter())
    }
}

pub fn bids<'a>() -> IndexedMap<'a, BidKey, Bid, BidIndicies<'a>> {
    let indexes = BidIndicies {
        collection: MultiIndex::new(
            |_pk: &[u8], d: &Bid| d.collection.clone(),
            "bids",
            "bids__collection",
        ),
        collection_token_id: MultiIndex::new(
            |_pk: &[u8], d: &Bid| (d.collection.clone(), d.token_id),
            "bids",
            "bids__collection_token_id",
        ),
        collection_price: MultiIndex::new(
            |_pk: &[u8], d: &Bid| (d.collection.clone(), d.price.u128()),
            "bids",
            "bids__collection_price",
        ),
        bidder: MultiIndex::new(
            |_pk: &[u8], d: &Bid| d.bidder.clone(),
            "bids",
            "bids__bidder",
        ),
        bidder_expires_at: MultiIndex::new(
            |_pk: &[u8], d: &Bid| (d.bidder.clone(), d.expires_at.seconds()),
            "bids",
            "bids__bidder_expires_at",
        ),
    };
    IndexedMap::new("bids", indexes)
}

#[cw_serde]
pub struct TokenInfo {
    pub owner: Addr,
    pub base_price: Vec<Coin>,
    pub token_uri: Option<String>,
    pub token_id: u64,
}
pub const TOKENS: Map<u64, TokenInfo> = Map::new("tokens");

#[cw_serde]
pub struct State {
    pub name: String,
    pub symbol: String,
    pub minter: Addr,
    pub num_tokens: u64,
}
pub const CONFIG: Item<State> = Item::new("config");
