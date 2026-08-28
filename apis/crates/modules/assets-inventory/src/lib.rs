//! Owns the Assets and inventory item and store catalogues.
//!
//! Stock balances, movements, Procurement receiving, and Finance posting remain
//! outside this slice. Catalogue references and item quantity scales are durable.

pub mod dtos;
mod models;
pub mod ops;
pub mod routes;

pub use dtos::{
    AssetStatus, CreateItemRequest, CreateStoreRequest, DeleteAssetQuery, ItemListQuery,
    ItemResponse, PaginatedItemsResponse, PaginatedStoresResponse, StoreListQuery, StoreResponse,
    UpdateItemRequest, UpdateStoreRequest,
};
pub use ops::{ItemOps, StoreOps};
