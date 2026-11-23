//! HTML Table extraction into structured data and typed records.

use scraper::{Html, Selector as ScraperSelector};
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

use crate::error::SeleniumBaseError;

/// Structured HTML table data.
///
/// Provides table headers, rows of string cells, and mapping into typed records.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::extract::TableData;
/// use serde::Deserialize;
///
/// #[derive(Debug, PartialEq, Deserialize)]
/// struct ServerStatus {
///     host: String,
///     status: String,
///     latency_ms: String,
/// }
///
/// let html = r#"
///     <table id="status-table">
///         <thead>
///             <tr><th>host</th><th>status</th><th>latency_ms</th></tr>
///         </thead>
///         <tbody>
///             <tr><td>node-1</td><td>online</td><td>12</td></tr>
///             <tr><td>node-2</td><td>offline</td><td>0</td></tr>
///         </tbody>
///     </table>
/// "#;
///
/// let table = TableData::from_html(html).unwrap();
/// assert_eq!(table.headers(), &["host", "status", "latency_ms"]);
/// assert_eq!(table.rows().len(), 2);
///
/// let records: Vec<ServerStatus> = table.to_records().unwrap();
/// assert_eq!(records[0].host, "node-1");
/// assert_eq!(records[1].status, "offline");
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TableData {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
}

impl TableData {
    /// Creates a new `TableData` from headers and rows.
    #[must_use]
    pub fn new(headers: Vec<String>, rows: Vec<Vec<String>>) -> Self {
        Self { headers, rows }
    }

    /// Extracted column headers.
    #[must_use]
    pub fn headers(&self) -> &[String] {
        &self.headers
    }

    /// Extracted table row data.
    #[must_use]
    pub fn rows(&self) -> &[Vec<String>] {
        &self.rows
    }

    /// Number of rows in the table.
    #[must_use]
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Returns `true` if the table has no data rows.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Parses a `TableData` from an HTML fragment containing a `<table>`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] if no `<table>` element is present.
    pub fn from_html(html_text: &str) -> Result<Self, SeleniumBaseError> {
        let fragment = Html::parse_fragment(html_text);
        let table_sel = ScraperSelector::parse("table").map_err(|e| {
            SeleniumBaseError::InvalidSelector(format!("failed to parse table selector: {e:?}"))
        })?;

        let Some(table_elem) = fragment.select(&table_sel).next() else {
            return Err(SeleniumBaseError::InvalidConfig(
                "no <table> element found in provided HTML".to_string(),
            ));
        };

        let mut headers = Vec::new();
        let mut rows = Vec::new();

        // 1. Try finding <th> in <thead> or first <tr>
        if let Ok(th_sel) = ScraperSelector::parse("th") {
            for th in table_elem.select(&th_sel) {
                let text = th.text().collect::<Vec<_>>().join("").trim().to_string();
                headers.push(text);
            }
        }

        // 2. Extract rows
        if let Ok(tr_sel) = ScraperSelector::parse("tr")
            && let Ok(td_sel) = ScraperSelector::parse("td")
        {
            for tr in table_elem.select(&tr_sel) {
                let mut row = Vec::new();
                for td in tr.select(&td_sel) {
                    let text = td.text().collect::<Vec<_>>().join("").trim().to_string();
                    row.push(text);
                }
                if !row.is_empty() {
                    rows.push(row);
                }
            }
        }

        Ok(Self { headers, rows })
    }

    /// Converts table rows into typed records `T` matching column headers.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Json`] if JSON mapping or deserialization fails.
    pub fn to_records<T: DeserializeOwned>(&self) -> Result<Vec<T>, SeleniumBaseError> {
        let mut records = Vec::with_capacity(self.rows.len());

        for row in &self.rows {
            let mut map = Map::new();
            for (idx, header) in self.headers.iter().enumerate() {
                if let Some(cell) = row.get(idx) {
                    map.insert(header.clone(), Value::String(cell.clone()));
                }
            }
            let obj = Value::Object(map);
            let item = serde_json::from_value::<T>(obj)?;
            records.push(item);
        }

        Ok(records)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, PartialEq, Deserialize)]
    struct ScoreRow {
        name: String,
        score: String,
    }

    #[test]
    fn parses_table_and_deserializes() {
        let html = r#"
            <table class="leaderboard">
                <thead>
                    <tr><th>name</th><th>score</th></tr>
                </thead>
                <tbody>
                    <tr><td>Alice</td><td>100</td></tr>
                    <tr><td>Bob</td><td>95</td></tr>
                </tbody>
            </table>
        "#;

        let table = TableData::from_html(html).unwrap();
        assert_eq!(table.headers(), &["name", "score"]);
        assert_eq!(table.len(), 2);
        assert!(!table.is_empty());
        assert_eq!(table.rows()[0], vec!["Alice", "100"]);
        assert_eq!(table.rows()[1], vec!["Bob", "95"]);

        let records: Vec<ScoreRow> = table.to_records().unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].name, "Alice");
        assert_eq!(records[0].score, "100");
        assert_eq!(records[1].name, "Bob");
        assert_eq!(records[1].score, "95");
    }
}
