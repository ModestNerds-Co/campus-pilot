//! Owns grading schemes, academic result snapshots, report cards, and progression review.
//!
//! Gradebook, Attendance, Academics, and SIS remain authoritative for source
//! records. This crate persists reviewed snapshots and never rewrites them.

pub mod dtos;
mod models;
pub mod ops;
pub mod routes;

pub use dtos::{
    AcademicReportBatchListQuery, AcademicReportBatchResponse, AcademicReportBatchStatus,
    AcademicReportBatchSummary, AcademicReportCardResponse, AcademicReportReferenceData,
    AcademicTranscriptResponse, CreateGradingSchemeRequest, DeleteAcademicReportQuery,
    DeleteGradingSchemeQuery, GenerateAcademicReportRequest, GradingBandInput, GradingBandResponse,
    GradingSchemeResponse, PaginatedAcademicReportBatchesResponse, ProgressionOutcome,
    ReopenAcademicReportRequest, TransitionAcademicReportRequest, UpdateGradingSchemeRequest,
    UpdateReportCardReviewRequest, UpdateReportCardTeacherCommentRequest,
};
pub use ops::{AcademicReportingAccessScope, AcademicReportingOps};
