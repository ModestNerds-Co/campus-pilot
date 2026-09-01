//! Owns assessment mark sheets, review, and result publication.
//!
//! Academics owns assessment structure and SIS owns learner identity and
//! enrolment. Gradebook stores stable references and exact mark values only.

pub mod dtos;
mod models;
pub mod ops;
pub mod routes;

pub use dtos::{
    CreateMarkSheetRequest, DeleteMarkSheetQuery, GradebookComponentReference, GradebookMarkInput,
    GradebookMarkResponse, GradebookMarkStatus, GradebookReferenceData, GradebookSheetListQuery,
    GradebookSheetResponse, GradebookSheetStatus, GradebookSheetSummary,
    PaginatedGradebookSheetsResponse, ReopenMarkSheetRequest, TransitionMarkSheetRequest,
    UpdateGradebookMarksRequest,
};
pub use ops::{GradebookAccessScope, GradebookOps};
