//! Interpreter-owned SQLite connections and fully materialized query results.
//!
//! No statement borrows survive a call into this module. This lets Rice keep
//! multiple result handles without holding SQLite locks or using unsafe lifetimes.

use std::collections::HashMap;
use std::time::Duration;

use rusqlite::types::{Value as SqlValue, ValueRef};
use rusqlite::{Connection, OpenFlags, params_from_iter};

use crate::error::RuntimeError;
use crate::value::Value;

#[derive(Default)]
pub(crate) struct Databases {
    connections: HashMap<i64, Connection>,
    results: HashMap<i64, QueryResult>,
    // Shared across both kinds of handle, and deliberately preserved by CLEAR.
    last_handle: i64,
}

struct QueryResult {
    connection: i64,
    columns: Vec<String>,
    rows: Vec<Vec<Value>>,
    next_row: usize,
    current_row: Option<usize>,
}

/// Observes the actions SQLite compiles, independently of its change counters.
/// These private connections have no other authorizer and execute one Rice
/// database operation at a time, so observers must never overlap.
struct SqlActionsObserver<'connection> {
    connection: &'connection Connection,
    actions: Box<std::cell::Cell<u8>>,
}

impl<'connection> SqlActionsObserver<'connection> {
    const DML: u8 = 1;
    const NON_DML: u8 = 2;

    fn new(connection: &'connection Connection) -> Result<Self, RuntimeError> {
        let actions = Box::new(std::cell::Cell::new(0));
        let context = std::ptr::from_ref(actions.as_ref()).cast_mut().cast();
        // SAFETY: the connection outlives this guard and the Box gives the
        // callback a stable address even when the guard moves. SQLite invokes
        // the callback synchronously on this connection, which is not Sync.
        // Drop unregisters the callback before freeing the Box.
        let code = unsafe {
            rusqlite::ffi::sqlite3_set_authorizer(connection.handle(), Some(Self::observe), context)
        };
        if code != rusqlite::ffi::SQLITE_OK {
            return Err(database_error(
                "DBEXEC",
                rusqlite::Error::SqliteFailure(rusqlite::ffi::Error::new(code), None),
            ));
        }
        Ok(Self {
            connection,
            actions,
        })
    }

    /// Snapshot immediately after preparation, before stepping: virtual tables
    /// can compile internal PRAGMA or schema statements during execution.
    /// Keep the observer alive through execution because SQLite may reprepare.
    fn is_dml(&self) -> bool {
        self.actions.get() == Self::DML
    }

    unsafe extern "C" fn observe(
        context: *mut std::ffi::c_void,
        action: std::ffi::c_int,
        _argument1: *const std::ffi::c_char,
        _argument2: *const std::ffi::c_char,
        _database: *const std::ffi::c_char,
        _accessor: *const std::ffi::c_char,
    ) -> std::ffi::c_int {
        let observed = match action {
            rusqlite::ffi::SQLITE_INSERT
            | rusqlite::ffi::SQLITE_UPDATE
            | rusqlite::ffi::SQLITE_DELETE => Self::DML,
            rusqlite::ffi::SQLITE_READ
            | rusqlite::ffi::SQLITE_SELECT
            | rusqlite::ffi::SQLITE_FUNCTION
            | rusqlite::ffi::SQLITE_RECURSIVE => 0,
            // DDL can also emit INSERT/UPDATE/DELETE for sqlite_schema or
            // cascading foreign keys. Its own action must override those.
            // PRAGMAs, transactions, attachment, maintenance, and any future
            // unrecognized action likewise do not represent direct DML.
            _ => Self::NON_DML,
        };
        if !context.is_null() {
            // SAFETY: new installed this pointer to the still-live boxed Cell;
            // the connection cannot invoke it concurrently. Reading numeric
            // action codes avoids interpreting potentially invalid UTF-8 names.
            let actions = unsafe { &*context.cast::<std::cell::Cell<u8>>() };
            actions.set(actions.get() | observed);
        }
        rusqlite::ffi::SQLITE_OK
    }
}

impl Drop for SqlActionsObserver<'_> {
    fn drop(&mut self) {
        // SAFETY: the borrowed connection is still open. Unregister while the
        // callback's boxed Cell is alive; dropping fields happens afterwards.
        unsafe {
            rusqlite::ffi::sqlite3_set_authorizer(
                self.connection.handle(),
                None,
                std::ptr::null_mut(),
            );
        }
    }
}

fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::IllegalFunctionCall {
        msg: message.into(),
    }
}

fn database_error(context: &str, error: rusqlite::Error) -> RuntimeError {
    let code = match &error {
        rusqlite::Error::SqliteFailure(error, _) => error.extended_code,
        _ => 0,
    };
    RuntimeError::Database {
        msg: format!("{context}: {error}"),
        code,
    }
}

fn count_value(count: u64) -> Result<Value, RuntimeError> {
    i64::try_from(count)
        .map(Value::Int64)
        .map_err(|_| RuntimeError::Overflow {
            msg: "database count exceeds INT64".into(),
        })
}

fn timeout(value: &Value) -> Result<Duration, RuntimeError> {
    let milliseconds = value.to_exact_i64()?;
    if !(0..=i64::from(i32::MAX)).contains(&milliseconds) {
        return Err(invalid(
            "database timeout must be in 0..2147483647 milliseconds",
        ));
    }
    Ok(Duration::from_millis(milliseconds as u64))
}

fn bind_value(value: &Value) -> Result<SqlValue, RuntimeError> {
    Ok(match value {
        Value::Null => SqlValue::Null,
        Value::Integer(n) => SqlValue::Integer(i64::from(*n)),
        Value::Long(n) => SqlValue::Integer(i64::from(*n)),
        Value::Int64(n) => SqlValue::Integer(*n),
        Value::Byte(n) => SqlValue::Integer(i64::from(*n)),
        Value::Bool(b) => SqlValue::Integer(i64::from(*b)),
        Value::Numeric(n) => {
            if n.is_nan() {
                return Err(invalid("cannot bind NaN to SQLite; use NULL() explicitly"));
            }
            SqlValue::Real(*n)
        }
        Value::Single(n) => return bind_value(&Value::Numeric(f64::from(*n))),
        Value::Str(text) => SqlValue::Text(text.clone()),
        Value::Bytes(bytes) => SqlValue::Blob(bytes.clone()),
        Value::Record { .. } => {
            return Err(RuntimeError::TypeMismatch {
                msg: "SQL parameters must be scalar values, BYTES, or NULL()".into(),
            });
        }
    })
}

fn read_value(value: ValueRef<'_>) -> Result<Value, RuntimeError> {
    Ok(match value {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(n) => Value::Int64(n),
        ValueRef::Real(n) => Value::Numeric(n),
        ValueRef::Text(bytes) => Value::Str(
            std::str::from_utf8(bytes)
                .map_err(|_| RuntimeError::Database {
                    msg:
                        "SQL TEXT is not valid UTF-8; select CAST(column AS BLOB) to read raw bytes"
                            .into(),
                    code: 0,
                })?
                .to_owned(),
        ),
        ValueRef::Blob(bytes) => Value::Bytes(bytes.to_owned()),
    })
}

fn read_column_names(
    connection: &Connection,
    statement: &rusqlite::Statement<'_>,
) -> Result<Vec<String>, RuntimeError> {
    // rusqlite's column-name API panics on invalid UTF-8 and does not expose
    // its statement pointer. Validate all live statements before using that
    // API, including any statements retained internally by virtual tables.
    // This conservatively rejects invalid metadata on those internal statements
    // too, without relying on statement ordering or rusqlite's private layout.
    //
    // SAFETY: The borrowed, interpreter-owned connection remains open, and is
    // used only on this thread. Enumeration and metadata access do not step,
    // prepare, reset, or finalize statements. Every pointer comes from SQLite
    // for this live connection; column indices are bounded by its column count.
    // SQLite owns each NUL-terminated name until statement mutation, and we
    // check for NULL before borrowing it. No C string reference escapes this
    // block, and no step/prepare occurs before the safe column_names call below.
    unsafe {
        let database = connection.handle();
        let mut current = rusqlite::ffi::sqlite3_next_stmt(database, std::ptr::null_mut());
        while !current.is_null() {
            for column in 0..rusqlite::ffi::sqlite3_column_count(current) {
                let name = rusqlite::ffi::sqlite3_column_name(current, column);
                if name.is_null() {
                    return Err(RuntimeError::Database {
                        msg: "SQLite could not allocate a result column name".into(),
                        code: rusqlite::ffi::SQLITE_NOMEM,
                    });
                }
                std::ffi::CStr::from_ptr(name)
                    .to_str()
                    .map_err(|_| RuntimeError::Database {
                        msg: "SQL result column name is not valid UTF-8".into(),
                        code: 0,
                    })?;
            }
            current = rusqlite::ffi::sqlite3_next_stmt(database, current);
        }
    }
    Ok(statement
        .column_names()
        .into_iter()
        .map(str::to_owned)
        .collect())
}

impl QueryResult {
    fn column(&self, value: &Value) -> Result<usize, RuntimeError> {
        if let Value::Str(name) = value {
            let mut matching = self
                .columns
                .iter()
                .enumerate()
                .filter(|(_, column)| column.eq_ignore_ascii_case(name));
            let index = matching
                .next()
                .map(|(index, _)| index)
                .ok_or_else(|| invalid(format!("unknown result column: {name}")))?;
            if matching.next().is_some() {
                return Err(invalid(format!(
                    "ambiguous result column: {name}; use its index"
                )));
            }
            return Ok(index);
        }
        self.column_index(value)
    }

    fn column_index(&self, value: &Value) -> Result<usize, RuntimeError> {
        let index = value.to_exact_i64()?;
        if index < 1 || index as u64 > self.columns.len() as u64 {
            return Err(invalid(
                "database column index is out of range (indices start at 1)",
            ));
        }
        Ok(index as usize - 1)
    }

    fn value(&self, column: &Value) -> Result<&Value, RuntimeError> {
        let column = self.column(column)?;
        let row = self.current_row.ok_or_else(|| {
            invalid("no current database row; call DBNEXT and check its result first")
        })?;
        Ok(&self.rows[row][column])
    }
}

impl Databases {
    pub(crate) fn is_function(name: &str) -> bool {
        matches!(
            name,
            "DBOPEN"
                | "DBCLOSE"
                | "DBEXEC"
                | "DBQUERY"
                | "DBNEXT"
                | "DBVALUE"
                | "DBISNULL"
                | "DBFREE"
                | "DBCOLUMNCOUNT"
                | "DBCOLUMNNAME$"
                | "DBCOLUMNTYPE$"
                | "DBROWCOUNT"
                | "DBCHANGES"
                | "DBLASTID"
                | "DBINTRANSACTION"
                | "DBBEGIN"
                | "DBCOMMIT"
                | "DBROLLBACK"
                | "DBTIMEOUT"
                | "DBVERSION$"
        )
    }

    pub(crate) fn clear(&mut self) {
        self.results.clear();
        // SQLite rolls back any open transaction when its connection closes.
        self.connections.clear();
    }

    fn allocate_handle(&mut self) -> Result<i64, RuntimeError> {
        self.last_handle =
            self.last_handle
                .checked_add(1)
                .ok_or_else(|| RuntimeError::Overflow {
                    msg: "database handle space exhausted".into(),
                })?;
        Ok(self.last_handle)
    }

    fn connection(&self, handle: i64) -> Result<&Connection, RuntimeError> {
        self.connections
            .get(&handle)
            .ok_or_else(|| invalid(format!("invalid or closed database handle: {handle}")))
    }

    fn result(&self, handle: i64) -> Result<&QueryResult, RuntimeError> {
        self.results
            .get(&handle)
            .ok_or_else(|| invalid(format!("invalid or freed result handle: {handle}")))
    }

    pub(crate) fn call(&mut self, name: &str, args: &[Value]) -> Result<Value, RuntimeError> {
        let (minimum, maximum) = match name {
            "DBOPEN" => (1, 3),
            "DBEXEC" | "DBQUERY" => (2, usize::MAX),
            "DBBEGIN" => (1, 2),
            "DBVALUE" | "DBISNULL" | "DBCOLUMNNAME$" | "DBCOLUMNTYPE$" | "DBTIMEOUT" => (2, 2),
            "DBVERSION$" => (0, 0),
            _ => (1, 1),
        };
        if args.len() < minimum || args.len() > maximum {
            return Err(invalid(format!("{name}: wrong number of arguments")));
        }
        if name == "DBVERSION$" {
            return Ok(Value::Str(rusqlite::version().into()));
        }
        if name == "DBOPEN" {
            return self.open(args);
        }
        let handle = args[0].to_exact_i64()?;
        match name {
            "DBCLOSE" => {
                self.connection(handle)?;
                let connection = self
                    .connections
                    .remove(&handle)
                    .expect("validated connection");
                if let Err((connection, error)) = connection.close() {
                    self.connections.insert(handle, connection);
                    return Err(database_error(name, error));
                }
                self.results.retain(|_, result| result.connection != handle);
            }
            "DBEXEC" | "DBQUERY" => return self.execute(handle, name, args),
            "DBNEXT" => {
                let result = self
                    .results
                    .get_mut(&handle)
                    .ok_or_else(|| invalid(format!("invalid or freed result handle: {handle}")))?;
                if result.next_row == result.rows.len() {
                    result.current_row = None;
                    return Ok(Value::Bool(false));
                }
                result.current_row = Some(result.next_row);
                result.next_row += 1;
                return Ok(Value::Bool(true));
            }
            "DBVALUE" => return Ok(self.result(handle)?.value(&args[1])?.clone()),
            "DBISNULL" => {
                return Ok(Value::Bool(matches!(
                    self.result(handle)?.value(&args[1])?,
                    Value::Null
                )));
            }
            "DBFREE" => {
                self.result(handle)?;
                self.results.remove(&handle);
            }
            "DBCOLUMNCOUNT" => return count_value(self.result(handle)?.columns.len() as u64),
            "DBROWCOUNT" => return count_value(self.result(handle)?.rows.len() as u64),
            "DBCOLUMNNAME$" => {
                let result = self.result(handle)?;
                let index = result.column_index(&args[1])?;
                return Ok(Value::Str(result.columns[index].clone()));
            }
            "DBCOLUMNTYPE$" => {
                let storage_class = match self.result(handle)?.value(&args[1])? {
                    Value::Null => "NULL",
                    Value::Int64(_) => "INTEGER",
                    Value::Numeric(_) => "REAL",
                    Value::Str(_) => "TEXT",
                    Value::Bytes(_) => "BLOB",
                    _ => unreachable!("only SQLite storage classes enter query results"),
                };
                return Ok(Value::Str(storage_class.into()));
            }
            "DBCHANGES" => return count_value(self.connection(handle)?.changes()),
            "DBLASTID" => return Ok(Value::Int64(self.connection(handle)?.last_insert_rowid())),
            "DBINTRANSACTION" => {
                return Ok(Value::Bool(!self.connection(handle)?.is_autocommit()));
            }
            "DBTIMEOUT" => self
                .connection(handle)?
                .busy_timeout(timeout(&args[1])?)
                .map_err(|error| database_error(name, error))?,
            "DBBEGIN" | "DBCOMMIT" | "DBROLLBACK" => {
                let sql = if name == "DBBEGIN" {
                    let mode = if args.len() == 2 {
                        args[1].to_string_val()?.to_ascii_uppercase()
                    } else {
                        "DEFERRED".into()
                    };
                    if !matches!(mode.as_str(), "DEFERRED" | "IMMEDIATE" | "EXCLUSIVE") {
                        return Err(invalid(
                            "DBBEGIN mode must be DEFERRED, IMMEDIATE, or EXCLUSIVE",
                        ));
                    }
                    format!("BEGIN {mode}")
                } else if name == "DBCOMMIT" {
                    "COMMIT".into()
                } else {
                    "ROLLBACK".into()
                };
                self.connection(handle)?
                    .execute(&sql, [])
                    .map_err(|error| database_error(name, error))?;
            }
            _ => return Err(invalid(format!("unknown database function: {name}"))),
        }
        Ok(Value::Bool(true))
    }

    fn open(&mut self, args: &[Value]) -> Result<Value, RuntimeError> {
        let path = args[0].to_string_val()?;
        if path.contains('\0') {
            return Err(invalid("DBOPEN path must not contain NUL"));
        }
        let mode = if args.len() >= 2 {
            args[1].to_string_val()?.to_ascii_lowercase()
        } else {
            "rwc".into()
        };
        let flags = match mode.as_str() {
            "rwc" => OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
            "rw" => OpenFlags::SQLITE_OPEN_READ_WRITE,
            "ro" => OpenFlags::SQLITE_OPEN_READ_ONLY,
            _ => return Err(invalid("DBOPEN mode must be rwc, rw, or ro")),
        } | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        let wait = if args.len() == 3 {
            timeout(&args[2])?
        } else {
            Duration::from_millis(5000)
        };
        // Bundled SQLite enables URI handling globally. An ordinary path must
        // not reach sqlite3_open with a leading `file:` URI scheme, even when
        // SQLITE_OPEN_URI is absent. Resolve just these relative filenames;
        // preserve SQLite's special empty and :memory: paths.
        let path = if path.starts_with("file:") {
            std::env::current_dir()
                .map_err(|error| RuntimeError::from_io("DBOPEN working directory", error))?
                .join(path)
        } else {
            std::path::PathBuf::from(path)
        };
        let handle = self.allocate_handle()?;
        let connection = Connection::open_with_flags(path, flags)
            .map_err(|error| database_error("DBOPEN", error))?;
        connection
            .busy_timeout(wait)
            .map_err(|error| database_error("DBOPEN", error))?;
        connection
            .pragma_update(None, "foreign_keys", true)
            .map_err(|error| database_error("DBOPEN", error))?;
        self.connections.insert(handle, connection);
        Ok(Value::Int64(handle))
    }

    fn execute(&mut self, handle: i64, name: &str, args: &[Value]) -> Result<Value, RuntimeError> {
        let sql = args[1].to_string_val()?;
        if sql.contains('\0') || sql.trim().is_empty() {
            return Err(invalid("SQL must be nonempty and must not contain NUL"));
        }
        let params = args[2..]
            .iter()
            .map(bind_value)
            .collect::<Result<Vec<_>, _>>()?;
        // Reserve before execution so handle exhaustion cannot hide successful writes.
        let result_handle = if name == "DBQUERY" {
            self.allocate_handle()?
        } else {
            0
        };
        let result = {
            let connection = self.connection(handle)?;
            let observer = if name == "DBEXEC" {
                Some(SqlActionsObserver::new(connection)?)
            } else {
                None
            };
            // Prepare on every call: cached column metadata can become stale
            // after PRAGMAs, and rejected PRAGMAs can retain compile-time effects
            // that must be reapplied on the next valid call. Preparation also
            // rejects additional SQL statements through rusqlite.
            let mut statement = connection
                .prepare(&sql)
                .map_err(|error| database_error(name, error))?;
            let is_dml = observer.as_ref().is_some_and(SqlActionsObserver::is_dml);
            if statement.parameter_count() != params.len() {
                return Err(invalid(format!(
                    "{name}: SQL requires {} parameters, got {}",
                    statement.parameter_count(),
                    params.len()
                )));
            }
            if name == "DBEXEC" {
                // Reject RETURNING before stepping it: it may otherwise write and
                // only then report that rows were returned.
                if statement.column_count() != 0 {
                    return Err(invalid("DBEXEC cannot return rows; use DBQUERY"));
                }
                let changed = statement
                    .execute(params_from_iter(params))
                    .map_err(|error| database_error(name, error))?;
                // DDL may perform internal writes without being caller DML.
                return count_value(if is_dml { changed as u64 } else { 0 });
            }
            if statement.column_count() == 0 {
                return Err(invalid(
                    "DBQUERY requires SQL that returns columns; use DBEXEC",
                ));
            }
            let mut rows = Vec::new();
            let mut columns = None;
            {
                let mut cursor = statement
                    .query(params_from_iter(params))
                    .map_err(|error| database_error(name, error))?;
                while let Some(row) = cursor.next().map_err(|error| database_error(name, error))? {
                    // SQLite may reprepare after a concurrent schema change. Get
                    // metadata after the first step, from the actual result row.
                    if columns.is_none() {
                        columns = Some(read_column_names(connection, row.as_ref())?);
                    }
                    let mut values = Vec::new();
                    for index in 0..row.as_ref().column_count() {
                        values.push(read_value(
                            row.get_ref(index)
                                .map_err(|error| database_error(name, error))?,
                        )?);
                    }
                    rows.try_reserve(1).map_err(|_| RuntimeError::Overflow {
                        msg: "database result exceeds available memory; use LIMIT to page results"
                            .into(),
                    })?;
                    rows.push(values);
                }
            }
            let columns = match columns {
                Some(columns) => columns,
                None => read_column_names(connection, &statement)?,
            };
            QueryResult {
                connection: handle,
                columns,
                rows,
                next_row: 0,
                current_row: None,
            }
        };
        self.results.insert(result_handle, result);
        Ok(Value::Int64(result_handle))
    }
}
