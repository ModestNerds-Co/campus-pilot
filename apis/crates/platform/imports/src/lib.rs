//! Parses bounded tabular import sources into a destination-neutral table.
//!
//! This crate owns CSV/XLSX decoding and source-shape invariants only. It does
//! not know tenant records, destination fields, permissions, or commit policy.

use std::{collections::HashSet, io::Cursor};

use calamine::{Reader, open_workbook_auto_from_rs};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Maximum retained source size accepted by the shared import boundary.
pub const MAX_SOURCE_BYTES: usize = 5 * 1024 * 1024;
/// Maximum number of data rows accepted in one staged import.
pub const MAX_SOURCE_ROWS: usize = 5_000;
/// Maximum number of columns accepted in one staged import.
pub const MAX_SOURCE_COLUMNS: usize = 100;

/// Supported server-side source formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceFormat {
    Csv,
    Xlsx,
}

impl SourceFormat {
    /// Stable value persisted with the retained source.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Xlsx => "xlsx",
        }
    }

    fn from_filename(filename: &str) -> Result<Self, SourceParseError> {
        let extension = filename
            .rsplit_once('.')
            .map(|(_, extension)| extension.to_ascii_lowercase())
            .ok_or(SourceParseError::UnsupportedFormat)?;
        match extension.as_str() {
            "csv" => Ok(Self::Csv),
            "xlsx" => Ok(Self::Xlsx),
            _ => Err(SourceParseError::UnsupportedFormat),
        }
    }
}

/// One source row, numbered as it appears in the original sheet or CSV.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRow {
    pub row_number: u32,
    pub values: Vec<String>,
}

/// A parsed source with unique canonical headers and bounded rows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceTable {
    pub headers: Vec<String>,
    pub rows: Vec<SourceRow>,
}

impl SourceTable {
    /// Returns a trimmed cell value by its canonical header.
    #[must_use]
    pub fn value<'a>(&'a self, row: &'a SourceRow, header: &str) -> Option<&'a str> {
        self.headers
            .iter()
            .position(|candidate| candidate == header)
            .and_then(|index| row.values.get(index))
            .map(String::as_str)
            .map(str::trim)
    }
}

/// A parsed, immutable source with the digest persisted beside its bytes.
#[derive(Debug, Clone)]
pub struct ParsedSource {
    pub format: SourceFormat,
    pub sha256_hex: String,
    pub table: SourceTable,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SourceParseError {
    #[error("Use a CSV or XLSX file.")]
    UnsupportedFormat,
    #[error("The import file is empty.")]
    Empty,
    #[error("The import file exceeds the 5 MB limit.")]
    SourceTooLarge,
    #[error("The import file has more than 100 columns.")]
    TooManyColumns,
    #[error("The import file has more than 5,000 data rows.")]
    TooManyRows,
    #[error("The first row must contain column names.")]
    MissingHeaders,
    #[error("Column names must be non-empty and unique.")]
    InvalidHeaders,
    #[error("The import file could not be read.")]
    InvalidSource,
}

/// Parses and fingerprints a source without applying destination rules.
pub fn parse_source(filename: &str, bytes: &[u8]) -> Result<ParsedSource, SourceParseError> {
    if bytes.is_empty() {
        return Err(SourceParseError::Empty);
    }
    if bytes.len() > MAX_SOURCE_BYTES {
        return Err(SourceParseError::SourceTooLarge);
    }
    let format = SourceFormat::from_filename(filename)?;
    let table = match format {
        SourceFormat::Csv => parse_csv(bytes)?,
        SourceFormat::Xlsx => parse_xlsx(bytes)?,
    };
    let sha256_hex = format!("{:x}", Sha256::digest(bytes));
    Ok(ParsedSource {
        format,
        sha256_hex,
        table,
    })
}

fn parse_csv(bytes: &[u8]) -> Result<SourceTable, SourceParseError> {
    let mut reader = csv::ReaderBuilder::new()
        .flexible(false)
        .trim(csv::Trim::All)
        .from_reader(bytes);
    let headers = reader
        .headers()
        .map_err(|_| SourceParseError::InvalidSource)?
        .iter()
        .map(remove_bom)
        .map(str::trim)
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    validate_headers(&headers)?;
    let mut rows = Vec::new();
    for (index, record) in reader.records().enumerate() {
        if rows.len() == MAX_SOURCE_ROWS {
            return Err(SourceParseError::TooManyRows);
        }
        let record = record.map_err(|_| SourceParseError::InvalidSource)?;
        rows.push(SourceRow {
            row_number: (index + 2) as u32,
            values: record
                .iter()
                .map(str::trim)
                .map(ToOwned::to_owned)
                .collect(),
        });
    }
    Ok(SourceTable { headers, rows })
}

fn parse_xlsx(bytes: &[u8]) -> Result<SourceTable, SourceParseError> {
    let cursor = Cursor::new(bytes.to_vec());
    let mut workbook =
        open_workbook_auto_from_rs(cursor).map_err(|_| SourceParseError::InvalidSource)?;
    let range = workbook
        .worksheet_range_at(0)
        .ok_or(SourceParseError::Empty)?
        .map_err(|_| SourceParseError::InvalidSource)?;
    let mut sheet_rows = range.rows();
    let headers = sheet_rows
        .next()
        .ok_or(SourceParseError::MissingHeaders)?
        .iter()
        .map(ToString::to_string)
        .map(|value| value.trim().to_string())
        .collect::<Vec<_>>();
    validate_headers(&headers)?;
    let mut rows = Vec::new();
    for (index, row) in sheet_rows.enumerate() {
        if rows.len() == MAX_SOURCE_ROWS {
            return Err(SourceParseError::TooManyRows);
        }
        let mut values = row
            .iter()
            .map(ToString::to_string)
            .map(|value| value.trim().to_string())
            .collect::<Vec<_>>();
        values.resize(headers.len(), String::new());
        values.truncate(headers.len());
        rows.push(SourceRow {
            row_number: (index + 2) as u32,
            values,
        });
    }
    Ok(SourceTable { headers, rows })
}

fn validate_headers(headers: &[String]) -> Result<(), SourceParseError> {
    if headers.is_empty() {
        return Err(SourceParseError::MissingHeaders);
    }
    if headers.len() > MAX_SOURCE_COLUMNS {
        return Err(SourceParseError::TooManyColumns);
    }
    let mut unique = HashSet::new();
    if headers
        .iter()
        .any(|header| header.is_empty() || !unique.insert(header.to_lowercase()))
    {
        return Err(SourceParseError::InvalidHeaders);
    }
    Ok(())
}

fn remove_bom(value: &str) -> &str {
    value.strip_prefix('\u{feff}').unwrap_or(value)
}

#[cfg(test)]
mod tests {
    use super::{SourceParseError, parse_source};

    #[test]
    fn csv_source_is_trimmed_numbered_and_fingerprinted() {
        let parsed = parse_source(
            "learners.csv",
            b" learner number , name \n 1001 , Ada Lovelace \n",
        )
        .unwrap_or_else(|error| panic!("valid CSV should parse: {error}"));
        assert_eq!(parsed.table.headers, ["learner number", "name"]);
        assert_eq!(parsed.table.rows[0].row_number, 2);
        assert_eq!(
            parsed.table.value(&parsed.table.rows[0], "name"),
            Some("Ada Lovelace")
        );
        assert_eq!(parsed.sha256_hex.len(), 64);
    }

    #[test]
    fn duplicate_headers_are_rejected_case_insensitively() {
        assert_eq!(
            parse_source("guardians.csv", b"Email,email\na@b.test,a@b.test\n")
                .expect_err("duplicate headers must fail"),
            SourceParseError::InvalidHeaders
        );
    }

    #[test]
    fn unsupported_and_empty_sources_are_rejected() {
        assert_eq!(
            parse_source("learners.xls", b"not empty").expect_err("xls is unsupported"),
            SourceParseError::UnsupportedFormat
        );
        assert_eq!(
            parse_source("learners.csv", b"").expect_err("empty source must fail"),
            SourceParseError::Empty
        );
    }
}
