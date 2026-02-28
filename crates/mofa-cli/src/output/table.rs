//! Table formatting for CLI output

use comfy_table::{Cell, Color, ContentArrangement, Table as ComfyTable};
use serde_json::Value;

/// Table builder for CLI output
#[derive(Debug, Clone)]
pub struct TableBuilder {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
}

impl TableBuilder {
    /// Create a new table builder
    pub fn new() -> Self {
        Self {
            headers: Vec::new(),
            rows: Vec::new(),
        }
    }

    /// Set table headers
    pub fn headers(mut self, headers: &[&str]) -> Self {
        self.headers = headers.iter().map(|s| s.to_string()).collect();
        self
    }

    /// Add a row to the table
    pub fn add_row(mut self, row: &[&str]) -> Self {
        self.rows.push(row.iter().map(|s| s.to_string()).collect());
        self
    }

    /// Build the table
    #[must_use]
    pub fn build(self) -> Table {
        Table {
            inner: {
                let mut table = ComfyTable::new();
                table.set_header(&self.headers);
                for row in self.rows {
                    table.add_row(row);
                }
                table
            },
        }
    }
}

impl Default for TableBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Table for CLI output
#[derive(Debug, Clone)]
pub struct Table {
    inner: ComfyTable,
}

impl Table {
    /// Create a new table from builder
    pub fn builder() -> TableBuilder {
        TableBuilder::new()
    }

    /// Create a table from a JSON array
    pub fn from_json_array(arr: &[Value]) -> Self {
        let mut table = ComfyTable::new();

        if arr.is_empty() {
            return Self { inner: table };
        }

        // Extract headers from first object
        if let Some(first) = arr.first()
            && let Some(obj) = first.as_object()
        {
            let headers: Vec<String> = obj.keys().cloned().collect();
            table.set_header(&headers);
        }

        // Add rows
        for item in arr {
            if let Some(obj) = item.as_object() {
                let row: Vec<String> = obj
                    .values()
                    .map(|v| match v {
                        Value::String(s) => s.clone(),
                        Value::Null => "".to_string(),
                        _ => v.to_string(),
                    })
                    .collect();
                table.add_row(row);
            }
        }

        // Configure table appearance
        table
            .set_content_arrangement(ContentArrangement::Dynamic)
            .set_width(120);

        Self { inner: table }
    }

    /// Set table width
    pub fn with_width(mut self, width: usize) -> Self {
        let _ = self.inner.set_width(width as u16);
        self
    }
}

/// Table cell wrapper
#[derive(Debug, Clone)]
pub struct TableCell(Cell);

impl TableCell {
    /// Create a new cell from string
    pub fn new(content: &str) -> Self {
        Self(Cell::new(content))
    }

    /// Create a new cell with foreground color
    pub fn with_fg(content: &str, color: Color) -> Self {
        Self(Cell::new(content).fg(color))
    }

    /// Create a new cell with background color
    pub fn with_bg(content: &str, color: Color) -> Self {
        Self(Cell::new(content).bg(color))
    }
}

impl From<String> for TableCell {
    fn from(s: String) -> Self {
        Self(Cell::new(s))
    }
}

impl From<&str> for TableCell {
    fn from(s: &str) -> Self {
        Self(Cell::new(s))
    }
}

/// Table row wrapper
#[derive(Debug, Clone)]
pub struct TableRow(Vec<TableCell>);

impl TableRow {
    /// Create a new row from cells
    pub fn new(cells: Vec<TableCell>) -> Self {
        Self(cells)
    }

    /// Create a row from strings
    pub fn from_strings(strings: &[&str]) -> Self {
        Self(strings.iter().map(|s| TableCell::new(s)).collect())
    }
}

impl std::fmt::Display for Table {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_table_builder() {
        let table = Table::builder()
            .headers(&["Name", "Value"])
            .add_row(&["foo", "bar"])
            .build();

        let output = table.to_string();
        assert!(output.contains("Name"));
        assert!(output.contains("Value"));
        assert!(output.contains("foo"));
        assert!(output.contains("bar"));
    }

    #[test]
    fn test_from_json_array() {
        let json = serde_json::json!([
            {"name": "Alice", "age": "30"},
            {"name": "Bob", "age": "25"}
        ]);

        let arr = json.as_array().unwrap();
        let table = Table::from_json_array(arr);
        let output = table.to_string();

        assert!(output.contains("name"));
        assert!(output.contains("age"));
        assert!(output.contains("Alice"));
        assert!(output.contains("Bob"));
    }
}
