/*extern crate grid;
use grid::Grid;
use crate::Mm;

/// Set the way of column line offset calculation
enum ColumnLineConfiguration<S: Into<String>> {
    /// Calculates automatically the column offset so that each columns width is equal to each other.
    /// The parameter is the amount of columns.
    /// - [ ] in progress
    Automatic(u8),
    /// Set the columns offset manually.
    /// The parameter is a List of the x-values started at the bottom left corner of the table
    /// - [ ] in progress
    Manually(Vec<Mm>),
    /// Calculates automatically the column offset based on the text written in the table
    /// The parameter contains a Grid with all text inputs in the table.
    /// If you want to have empty cells at the beginning of the table generation.
    /// Please use `Automatic` or `Manually` instead.
    /// - [ ] in progress
    ContentAdapting(Grid<S>),
}*/