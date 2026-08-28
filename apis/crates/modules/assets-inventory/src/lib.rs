//! Owns the Assets and inventory item and store catalogues.
//!
//! Stock balances and immutable movements remain Assets-owned. Procurement
//! receipt lifecycle and Finance valuation/posting stay behind typed boundaries.

pub mod dtos;
mod models;
pub mod ops;
pub mod routes;
pub mod stock_dtos;
mod stock_models;
pub mod stock_ops;

pub use dtos::{
    AssetStatus, CreateItemRequest, CreateStoreRequest, DeleteAssetQuery, ItemListQuery,
    ItemResponse, PaginatedItemsResponse, PaginatedStoresResponse, StoreListQuery, StoreResponse,
    UpdateItemRequest, UpdateStoreRequest,
};
pub use ops::{ItemOps, StoreOps};
pub use stock_dtos::{
    AdjustStockLineInput, AdjustStockRequest, AllocateGoodsReceiptLineInput,
    AllocateGoodsReceiptRequest, GoodsReceiptAllocationLineResponse,
    GoodsReceiptAllocationListQuery, GoodsReceiptAllocationResponse, IssueStockRequest,
    ManualReceiptRequest, PaginatedGoodsReceiptAllocationsResponse, PaginatedStockBalancesResponse,
    PaginatedStockMovementsResponse, ReverseStockMovementRequest, StockBalanceListQuery,
    StockBalanceResponse, StockMovementLineResponse, StockMovementListQuery, StockMovementResponse,
    StockMovementSummaryResponse, StockQuantityLineInput, TransferStockLineInput,
    TransferStockRequest,
};
pub use stock_ops::{
    GoodsReceiptAllocationOps, StockBalanceOps, StockMovementOps,
    bounded_goods_receipt_allocation_page,
};
