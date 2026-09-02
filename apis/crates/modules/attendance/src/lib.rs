//! Owns daily learner attendance registers and their submission lifecycle.
//!
//! Academics owns terms and classes, while SIS owns learner identity and
//! enrolment eligibility. Attendance stores only stable references to them.

pub mod dtos;
mod exceptions;
mod lesson_sessions;
mod models;
pub mod ops;
pub mod routes;

pub use dtos::{
    AcknowledgeAttendanceExceptionRequest, AttendanceAccessScope, AttendanceClassReference,
    AttendanceExceptionListQuery, AttendanceExceptionMark, AttendanceExceptionResponse,
    AttendanceExceptionStatus, AttendanceLearnerSummary, AttendanceLessonSessionListQuery,
    AttendanceLessonSessionStatus, AttendanceLessonSessionSummary, AttendanceMarkInput,
    AttendanceMarkResponse, AttendanceMarkStatus, AttendancePeriod, AttendanceReferenceData,
    AttendanceRegisterListQuery, AttendanceRegisterResponse, AttendanceRegisterStatus,
    AttendanceRegisterSummary, CancelAttendanceLessonSessionRequest,
    CreateAttendanceRegisterRequest, DeleteAttendanceRegisterQuery, LearnerAttendanceHistoryEntry,
    LearnerAttendanceHistoryQuery, LearnerAttendanceHistoryResponse,
    OpenAttendanceLessonSessionRequest, PaginatedAttendanceExceptionsResponse,
    PaginatedAttendanceLessonSessionsResponse, PaginatedAttendanceRegistersResponse,
    ReopenAttendanceExceptionRequest, ReopenAttendanceRegisterRequest,
    ResolveAttendanceExceptionRequest, SubmitAttendanceRegisterRequest,
    SyncAttendanceLessonSessionsRequest, SyncAttendanceLessonSessionsResponse,
    UpdateAttendanceMarksRequest,
};
pub use ops::AttendanceOps;
