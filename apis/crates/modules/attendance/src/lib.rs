//! Owns daily learner attendance registers and their submission lifecycle.
//!
//! Academics owns terms and classes, while SIS owns learner identity and
//! enrolment eligibility. Attendance stores only stable references to them.

pub mod dtos;
mod models;
pub mod ops;
pub mod routes;

pub use dtos::{
    AttendanceClassReference, AttendanceLearnerSummary, AttendanceMarkInput,
    AttendanceMarkResponse, AttendanceMarkStatus, AttendancePeriod, AttendanceReferenceData,
    AttendanceRegisterListQuery, AttendanceRegisterResponse, AttendanceRegisterStatus,
    AttendanceRegisterSummary, CreateAttendanceRegisterRequest, DeleteAttendanceRegisterQuery,
    PaginatedAttendanceRegistersResponse, ReopenAttendanceRegisterRequest,
    SubmitAttendanceRegisterRequest, UpdateAttendanceMarksRequest,
};
pub use ops::AttendanceOps;
