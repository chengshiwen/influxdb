//! A panic-safe write abstraction for [`MutableBatch`]

use crate::column::{Column, ColumnData, INVALID_DID};
use crate::MutableBatch;
use arrow_util::bitset::iter_set_positions;
use data_types::partition_metadata::{StatValues, Statistics};
use schema::{InfluxColumnType, InfluxFieldType};
use snafu::Snafu;
use std::num::NonZeroU64;

#[allow(missing_docs, missing_copy_implementations)]
#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Unable to insert {} type into a column of {}", inserted, existing))]
    TypeMismatch {
        existing: InfluxColumnType,
        inserted: InfluxColumnType,
    },

    #[snafu(display("Incorrect number of values provided"))]
    InsufficientValues,

    #[snafu(display("Key not found in dictionary: {}", key))]
    KeyNotFound { key: usize },
}

/// A specialized `Error` for [`Writer`] errors
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// [`Writer`] provides a panic-safe abstraction to append a number of rows to a [`MutableBatch`]
///
/// If a [`Writer`] is dropped without calling [`Writer::commit`], the [`MutableBatch`] will be
/// truncated to the original number of rows, and the statistics not updated
#[derive(Debug)]
pub struct Writer<'a> {
    /// The mutable batch that is being mutated
    batch: &'a mut MutableBatch,
    /// A list of column index paired with Statistics
    ///
    /// Statistics updates are deferred to commit time
    statistics: Vec<(usize, Statistics)>,
    /// The initial number of rows in the MutableBatch
    initial_rows: usize,
    /// The number of rows to insert
    to_insert: usize,
    /// If this Writer committed successfully
    success: bool,
}

impl<'a> Writer<'a> {
    /// Create a [`Writer`] for inserting `to_insert` rows to the provided `batch`
    ///
    /// If the writer is dropped without calling commit all changes will be rolled back
    pub fn new(batch: &'a mut MutableBatch, to_insert: usize) -> Self {
        let initial_rows = batch.rows();
        Self {
            batch,
            statistics: vec![],
            initial_rows,
            to_insert,
            success: false,
        }
    }

    /// Write the f64 typed column identified by `name`
    ///
    /// For each set bit in `valid_mask` an a value from `values` is inserted at the
    /// corresponding index in the column. Nulls are inserted for the other rows
    ///
    /// # Panic
    ///
    /// - panics if this column has already been written to by this `Writer`
    ///
    pub fn write_f64<I>(
        &mut self,
        name: &str,
        valid_mask: Option<&[u8]>,
        mut values: I,
    ) -> Result<()>
    where
        I: Iterator<Item = f64>,
    {
        let initial_rows = self.initial_rows;
        let to_insert = self.to_insert;

        let (col_idx, col) =
            self.column_mut(name, InfluxColumnType::Field(InfluxFieldType::Float))?;

        let mut stats = StatValues::new_empty();
        match &mut col.data {
            ColumnData::F64(col_data, _) => {
                col_data.resize(initial_rows + to_insert, 0_f64);
                for idx in set_position_iterator(valid_mask, to_insert) {
                    let value = values.next().ok_or(Error::InsufficientValues)?;
                    col_data[initial_rows + idx] = value;
                    stats.update(&value);
                }
            }
            x => unreachable!("expected f64 got {} for column \"{}\"", x, name),
        }

        append_valid_mask(col, valid_mask, to_insert);

        stats.update_for_nulls(to_insert as u64 - stats.total_count);
        self.statistics.push((col_idx, Statistics::F64(stats)));

        Ok(())
    }

    /// Write the i64 typed column identified by `name`
    ///
    /// For each set bit in `valid_mask` an a value from `values` is inserted at the
    /// corresponding index in the column. Nulls are inserted for the other rows
    ///
    /// # Panic
    ///
    /// - panics if this column has already been written to by this `Writer`
    ///
    pub fn write_i64<I>(
        &mut self,
        name: &str,
        valid_mask: Option<&[u8]>,
        mut values: I,
    ) -> Result<()>
    where
        I: Iterator<Item = i64>,
    {
        let initial_rows = self.initial_rows;
        let to_insert = self.to_insert;

        let (col_idx, col) =
            self.column_mut(name, InfluxColumnType::Field(InfluxFieldType::Integer))?;

        let mut stats = StatValues::new_empty();
        match &mut col.data {
            ColumnData::I64(col_data, _) => {
                col_data.resize(initial_rows + to_insert, 0_i64);
                for idx in set_position_iterator(valid_mask, to_insert) {
                    let value = values.next().ok_or(Error::InsufficientValues)?;
                    col_data[initial_rows + idx] = value;
                    stats.update(&value);
                }
            }
            x => unreachable!("expected i64 got {} for column \"{}\"", x, name),
        }

        append_valid_mask(col, valid_mask, to_insert);

        stats.update_for_nulls(to_insert as u64 - stats.total_count);
        self.statistics.push((col_idx, Statistics::I64(stats)));

        Ok(())
    }

    /// Write the u64 typed column identified by `name`
    ///
    /// For each set bit in `valid_mask` an a value from `values` is inserted at the
    /// corresponding index in the column. Nulls are inserted for the other rows
    ///
    /// # Panic
    ///
    /// - panics if this column has already been written to by this `Writer`
    ///
    pub fn write_u64<I>(
        &mut self,
        name: &str,
        valid_mask: Option<&[u8]>,
        mut values: I,
    ) -> Result<()>
    where
        I: Iterator<Item = u64>,
    {
        let initial_rows = self.initial_rows;
        let to_insert = self.to_insert;

        let (col_idx, col) =
            self.column_mut(name, InfluxColumnType::Field(InfluxFieldType::UInteger))?;

        let mut stats = StatValues::new_empty();
        match &mut col.data {
            ColumnData::U64(col_data, _) => {
                col_data.resize(initial_rows + to_insert, 0_u64);
                for idx in set_position_iterator(valid_mask, to_insert) {
                    let value = values.next().ok_or(Error::InsufficientValues)?;
                    col_data[initial_rows + idx] = value;
                    stats.update(&value);
                }
            }
            x => unreachable!("expected u64 got {} for column \"{}\"", x, name),
        }

        append_valid_mask(col, valid_mask, to_insert);

        stats.update_for_nulls(to_insert as u64 - stats.total_count);
        self.statistics.push((col_idx, Statistics::U64(stats)));

        Ok(())
    }

    /// Write the boolean typed column identified by `name`
    ///
    /// For each set bit in `valid_mask` an a value from `values` is inserted at the
    /// corresponding index in the column. Nulls are inserted for the other rows
    ///
    /// # Panic
    ///
    /// - panics if this column has already been written to by this `Writer`
    ///
    pub fn write_bool<I>(
        &mut self,
        name: &str,
        valid_mask: Option<&[u8]>,
        mut values: I,
    ) -> Result<()>
    where
        I: Iterator<Item = bool>,
    {
        let initial_rows = self.initial_rows;
        let to_insert = self.to_insert;

        let (col_idx, col) =
            self.column_mut(name, InfluxColumnType::Field(InfluxFieldType::Boolean))?;

        let mut stats = StatValues::new_empty();
        match &mut col.data {
            ColumnData::Bool(col_data, _) => {
                col_data.append_unset(to_insert);
                for idx in set_position_iterator(valid_mask, to_insert) {
                    let value = values.next().ok_or(Error::InsufficientValues)?;
                    if value {
                        col_data.set(initial_rows + idx);
                    }
                    stats.update(&value);
                }
            }
            x => unreachable!("expected bool got {} for column \"{}\"", x, name),
        }

        append_valid_mask(col, valid_mask, to_insert);

        stats.update_for_nulls(to_insert as u64 - stats.total_count);
        self.statistics.push((col_idx, Statistics::Bool(stats)));

        Ok(())
    }

    /// Write the string field typed column identified by `name`
    ///
    /// For each set bit in `valid_mask` an a value from `values` is inserted at the
    /// corresponding index in the column. Nulls are inserted for the other rows
    ///
    /// # Panic
    ///
    /// - panics if this column has already been written to by this `Writer`
    ///
    pub fn write_string<'s, I>(
        &mut self,
        name: &str,
        valid_mask: Option<&[u8]>,
        mut values: I,
    ) -> Result<()>
    where
        I: Iterator<Item = &'s str>,
    {
        let initial_rows = self.initial_rows;
        let to_insert = self.to_insert;

        let (col_idx, col) =
            self.column_mut(name, InfluxColumnType::Field(InfluxFieldType::String))?;

        let mut stats = StatValues::new_empty();
        match &mut col.data {
            ColumnData::String(col_data, _) => {
                for idx in set_position_iterator(valid_mask, to_insert) {
                    let value = values.next().ok_or(Error::InsufficientValues)?;
                    col_data.extend(initial_rows + idx - col_data.len());
                    col_data.append(value);
                    stats.update(value);
                }
            }
            x => unreachable!("expected tag got {} for column \"{}\"", x, name),
        }

        append_valid_mask(col, valid_mask, to_insert);

        stats.update_for_nulls(to_insert as u64 - stats.total_count);
        self.statistics.push((col_idx, Statistics::String(stats)));

        Ok(())
    }

    /// Write the tag typed column identified by `name`
    ///
    /// For each set bit in `valid_mask` an a value from `values` is inserted at the
    /// corresponding index in the column. Nulls are inserted for the other rows
    ///
    /// # Panic
    ///
    /// - panics if this column has already been written to by this `Writer`
    ///
    pub fn write_tag<'s, I>(
        &mut self,
        name: &str,
        valid_mask: Option<&[u8]>,
        mut values: I,
    ) -> Result<()>
    where
        I: Iterator<Item = &'s str>,
    {
        let initial_rows = self.initial_rows;
        let to_insert = self.to_insert;

        let (col_idx, col) = self.column_mut(name, InfluxColumnType::Tag)?;

        let mut stats = StatValues::new_empty();
        match &mut col.data {
            ColumnData::Tag(col_data, dict, _) => {
                col_data.resize(initial_rows + to_insert, INVALID_DID);

                for idx in set_position_iterator(valid_mask, to_insert) {
                    let value = values.next().ok_or(Error::InsufficientValues)?;
                    col_data[initial_rows + idx] = dict.lookup_value_or_insert(value);
                    stats.update(value);
                }
            }
            x => unreachable!("expected tag got {} for column \"{}\"", x, name),
        }

        append_valid_mask(col, valid_mask, to_insert);

        stats.update_for_nulls(to_insert as u64 - stats.total_count);
        self.statistics.push((col_idx, Statistics::String(stats)));

        Ok(())
    }

    /// Write the tag typed column identified by `name`
    ///
    /// For each set bit in `valid_mask` an a value from `values` is inserted at the
    /// corresponding index in the column. Nulls are inserted for the other rows
    ///
    /// # Panic
    ///
    /// - panics if this column has already been written to by this `Writer`
    ///
    pub fn write_tag_dict<'s, K, V>(
        &mut self,
        name: &str,
        valid_mask: Option<&[u8]>,
        mut keys: K,
        values: V,
    ) -> Result<()>
    where
        K: Iterator<Item = usize>,
        V: Iterator<Item = &'s str>,
    {
        let initial_rows = self.initial_rows;
        let to_insert = self.to_insert;

        let (col_idx, col) = self.column_mut(name, InfluxColumnType::Tag)?;

        let mut stats = StatValues::new_empty();
        match &mut col.data {
            ColumnData::Tag(col_data, dict, _) => {
                // Lazily compute mappings to handle dictionaries with unused mappings
                let mut mapping: Vec<_> = values.map(|value| (value, None)).collect();

                col_data.resize(initial_rows + to_insert, INVALID_DID);

                for idx in set_position_iterator(valid_mask, to_insert) {
                    let key = keys.next().ok_or(Error::InsufficientValues)?;
                    let (value, maybe_did) =
                        mapping.get_mut(key).ok_or(Error::KeyNotFound { key })?;

                    match maybe_did {
                        Some(did) => col_data[initial_rows + idx] = *did,
                        None => {
                            let did = dict.lookup_value_or_insert(value);
                            *maybe_did = Some(did);
                            col_data[initial_rows + idx] = did
                        }
                    }
                    stats.update(*value);
                }
            }
            x => unreachable!("expected tag got {} for column \"{}\"", x, name),
        }

        append_valid_mask(col, valid_mask, to_insert);

        stats.update_for_nulls(to_insert as u64 - stats.total_count);
        self.statistics.push((col_idx, Statistics::String(stats)));

        Ok(())
    }

    /// Write the time typed column identified by `name`
    ///
    /// For each set bit in `valid_mask` an a value from `values` is inserted at the
    /// corresponding index in the column. Nulls are inserted for the other rows
    ///
    /// # Panic
    ///
    /// - panics if this column has already been written to by this `Writer`
    ///
    pub fn write_time<I>(&mut self, name: &str, mut values: I) -> Result<()>
    where
        I: Iterator<Item = i64>,
    {
        let initial_rows = self.initial_rows;
        let to_insert = self.to_insert;

        let (col_idx, col) = self.column_mut(name, InfluxColumnType::Timestamp)?;

        let mut stats = StatValues::new_empty();
        match &mut col.data {
            ColumnData::I64(col_data, _) => {
                col_data.resize(initial_rows + to_insert, 0_i64);
                for idx in 0..to_insert {
                    let value = values.next().ok_or(Error::InsufficientValues)?;
                    col_data[initial_rows + idx] = value;
                    stats.update(&value)
                }
            }
            x => unreachable!("expected i64 got {} for column \"{}\"", x, name),
        }

        append_valid_mask(col, None, to_insert);

        stats.update_for_nulls(to_insert as u64 - stats.total_count);
        self.statistics.push((col_idx, Statistics::I64(stats)));

        Ok(())
    }

    fn column_mut(
        &mut self,
        name: &str,
        influx_type: InfluxColumnType,
    ) -> Result<(usize, &mut Column)> {
        let columns_len = self.batch.columns.len();

        let column_idx = *self
            .batch
            .column_names
            .raw_entry_mut()
            .from_key(name)
            .or_insert_with(|| (name.to_string(), columns_len))
            .1;

        if columns_len == column_idx {
            self.batch
                .columns
                .push(Column::new(self.initial_rows, influx_type))
        }

        let col = &mut self.batch.columns[column_idx];

        if col.influx_type != influx_type {
            return Err(Error::TypeMismatch {
                existing: col.influx_type,
                inserted: influx_type,
            });
        }

        assert_eq!(
            col.valid.len(),
            self.initial_rows,
            "expected {} rows in column \"{}\" got {} when performing write of {} rows",
            self.initial_rows,
            name,
            col.valid.len(),
            self.to_insert
        );

        Ok((column_idx, col))
    }

    /// Commits the writes performed on this [`Writer`]. This will update the statistics
    /// and pad any unwritten columns with nulls
    pub fn commit(mut self) {
        let initial_rows = self.initial_rows;
        let final_rows = initial_rows + self.to_insert;

        self.statistics
            .sort_unstable_by_key(|(col_idx, _)| *col_idx);
        let mut statistics = self.statistics.iter();

        for (col_idx, col) in self.batch.columns.iter_mut().enumerate() {
            // All columns should either have received a write and have statistics or not
            if col.valid.len() == initial_rows {
                col.push_nulls_to_len(final_rows);
            } else {
                assert_eq!(
                    col.valid.len(),
                    final_rows,
                    "expected {} rows in column index {} got {} when performing write of {} rows",
                    final_rows,
                    col_idx,
                    col.valid.len(),
                    self.to_insert
                );

                let (stats_col_idx, stats) = statistics.next().unwrap();
                assert_eq!(*stats_col_idx, col_idx);

                match (&mut col.data, stats) {
                    (ColumnData::F64(_, stats), Statistics::F64(new)) => {
                        stats.update_from(new);
                    }
                    (ColumnData::I64(_, stats), Statistics::I64(new)) => {
                        stats.update_from(new);
                    }
                    (ColumnData::U64(_, stats), Statistics::U64(new)) => {
                        stats.update_from(new);
                    }
                    (ColumnData::String(_, stats), Statistics::String(new)) => {
                        stats.update_from(new);
                    }
                    (ColumnData::Bool(_, stats), Statistics::Bool(new)) => {
                        stats.update_from(new);
                    }
                    (ColumnData::Tag(_, dict, stats), Statistics::String(new)) => {
                        stats.update_from(new);
                        stats.distinct_count = match stats.null_count {
                            0 => NonZeroU64::new(dict.values().len() as u64),
                            _ => NonZeroU64::new(dict.values().len() as u64 + 1),
                        }
                    }
                    _ => unreachable!("column: {}, statistics: {}", col.data, stats.type_name()),
                }
            }
        }
        self.batch.row_count = final_rows;
        self.success = true;
    }
}

fn set_position_iterator(
    valid_mask: Option<&[u8]>,
    to_insert: usize,
) -> impl Iterator<Item = usize> + '_ {
    match valid_mask {
        Some(mask) => itertools::Either::Left(
            iter_set_positions(mask).take_while(move |idx| *idx < to_insert),
        ),
        None => itertools::Either::Right(0..to_insert),
    }
}

fn append_valid_mask(column: &mut Column, valid_mask: Option<&[u8]>, to_insert: usize) {
    match valid_mask {
        Some(mask) => column.valid.append_bits(to_insert, mask),
        None => column.valid.append_set(to_insert),
    }
}

impl<'a> Drop for Writer<'a> {
    fn drop(&mut self) {
        if !self.success {
            let initial_rows = self.initial_rows;
            for col in &mut self.batch.columns {
                col.valid.truncate(initial_rows);
                match &mut col.data {
                    ColumnData::F64(col_data, _) => col_data.truncate(initial_rows),
                    ColumnData::I64(col_data, _) => col_data.truncate(initial_rows),
                    ColumnData::U64(col_data, _) => col_data.truncate(initial_rows),
                    ColumnData::String(col_data, _) => col_data.truncate(initial_rows),
                    ColumnData::Bool(col_data, _) => col_data.truncate(initial_rows),
                    ColumnData::Tag(col_data, dict, _) => {
                        col_data.truncate(initial_rows);
                        match col_data.iter().max() {
                            Some(max) => dict.truncate(*max),
                            None => dict.clear(),
                        }
                    }
                }
            }
        }
    }
}
