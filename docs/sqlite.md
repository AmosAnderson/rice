# SQLite databases

Rice includes SQLite through the `DB...` functions. SQLite is bundled with Rice;
programs do not need a separate database server or SQLite installation. Use SQL
to create and alter tables and indexes, insert, read, update, and delete rows,
join tables, and run SQLite pragmas and maintenance commands.

```basic
DIM db AS INT64
DIM rows AS INT64
db = DBOPEN(":memory:")
CALL DBEXEC(db, "CREATE TABLE notes (id INTEGER PRIMARY KEY, text TEXT, attachment BLOB)")
CALL DBEXEC(db, "INSERT INTO notes (text, attachment) VALUES (?, ?)", "Hello, SQLite", BYTES(0, 255))
rows = DBQUERY(db, "SELECT id, text, attachment FROM notes WHERE id = ?", DBLASTID(db))
WHILE DBNEXT(rows)
    PRINT DBVALUE(rows, "id"); ": "; DBVALUE(rows, "text")
    PRINT HEXENCODE$(DBVALUE(rows, "attachment"))
WEND
CALL DBFREE(rows)
CALL DBCLOSE(db)
```

This prints `1: Hello, SQLite` and `00FF`. The [SQLite example](../tests/programs/sqlite.bas)
also exercises transactions, exact integers, boolean binding, and NULL values.

## Connections and lifetime

| Function | Result and behavior |
|---|---|
| `DBOPEN(path$[, mode$[, timeoutMs]])` | New INT64 connection handle. Mode is `"rwc"` (default: read/write, create if absent), `"rw"` (existing file, read/write), or `"ro"` (existing file, read-only); mode names are case-insensitive. |
| `DBCLOSE(db)` | Close the connection, invalidate its result handles, and roll back any uncommitted transaction. Returns BOOL TRUE. |
| `DBTIMEOUT(db, timeoutMs)` | Set how long SQLite waits for a database lock, in milliseconds. Returns BOOL TRUE. |
| `DBVERSION$()` | Version string of the bundled SQLite library. Parentheses are required. |

Relative paths use the process working directory, as with Rice file I/O. Use
`":memory:"` for a private in-memory database. An empty path opens a temporary
database. `DBOPEN` treats filenames literally, including a `file:` prefix and
URI option text. SQL `ATTACH` follows SQLite's own filename semantics.
Each connection starts with foreign
key enforcement enabled and a busy timeout of 5000 milliseconds. Timeouts must
be exact integers in `0..2147483647`; zero disables waiting.

Connection and result handles are opaque INT64 values owned by one interpreter.
Keep the returned value; do not calculate or guess handles. Handles are never
reused within an interpreter, and using a closed or freed handle raises an error.
They survive successive `run_source` calls and immediate REPL inputs. `CLEAR`
closes all database connections and frees all results, even when executed inside
a procedure. A fresh REPL `RUN` and interpreter destruction also release them.
Closing a connection or destroying its interpreter rolls back uncommitted work.

Each database function can be used in an expression or called with `CALL` to
discard its return value. Database arguments are evaluated left to right and
passed by value; there is no BYREF copy-back. Use parentheses for these calls,
including `DBVERSION$()`.

## Executing SQL and binding values

| Function | Result and behavior |
|---|---|
| `DBEXEC(db, sql$[, parameter, ...])` | Execute exactly one statement that returns no columns. Return an INT64 count of changed rows for DML, or zero for a non-DML statement. |
| `DBQUERY(db, sql$[, parameter, ...])` | Execute exactly one statement with result columns and return an INT64 result handle. Accepts SELECT, row-returning PRAGMA, and DML with RETURNING. |
| `DBCHANGES(db)` | SQLite's connection change counter, as INT64. Non-DML can leave the previous count or expose a count from internal SQLite writes. Use DBEXEC's return value for the submitted statement's count. |
| `DBLASTID(db)` | SQLite's last inserted rowid for this connection, as an exact INT64; initially zero. |

`DBEXEC` rejects a row-returning statement before stepping it; a rejected
SELECT or DML with RETURNING is not run. `DBQUERY` requires result columns,
including when the query produces zero rows, and rejects non-returning DML
before stepping it. Some SQLite PRAGMAs apply settings during SQL preparation,
so a rejected call can still change a connection setting. This also applies to
a multi-statement or parameter-count error after a PRAGMA has been prepared. Neither function
accepts a batch of SQL statements. Make separate calls for a script, optionally
inside a transaction. SQL such as `CREATE`, `ALTER`, `DROP`, `SAVEPOINT`, `RELEASE`,
`ATTACH`, and `VACUUM` uses these same functions according to whether it returns
columns; Rice does not implement separate grammar for individual SQL commands.

`DBEXEC` counts rows directly changed by INSERT, UPDATE, DELETE, or REPLACE;
trigger and foreign-key side effects are excluded. Non-DML returns zero even
when SQLite performs internal writes, such as creating a virtual table or
dropping a table with cascading foreign keys.

Bind data with `?` placeholders:

```basic
CALL DBEXEC(db, "UPDATE notes SET text = ? WHERE id = ?", "It's bound data", CINT64(1))
rows = DBQUERY(db, "SELECT text FROM notes WHERE id = ?", CINT64(1))
```

Parameter values are passed separately from SQL text, so quotes and binary data
do not need SQL escaping. Placeholders stand for data values, not table names,
column names, or SQL syntax. The number of supplied values must match the
statement's SQLite parameter-slot count exactly. Bindings follow slot order
`1..N`. SQLite `?N` and named placeholders are also accepted, but repeated names
share one slot and numbered gaps still contribute to the slot count. Ordinary
`?` placeholders avoid that ambiguity.

| Rice value | SQLite binding | Returned Rice value |
|---|---|---|
| INTEGER, LONG, INT64, BYTE | INTEGER, preserving exact integer bits | INT64 |
| BOOL | INTEGER `1` for TRUE, `0` for FALSE | INT64; use `CBOOL` when a boolean is desired |
| NUMERIC/DOUBLE, SINGLE | REAL | NUMERIC |
| STRING | TEXT | STRING |
| BYTES | BLOB, including an empty buffer | BYTES |
| `NULL()` | NULL | NULL |

Records and whole arrays cannot be bound as scalar parameters. NaN parameters
are rejected; bind `NULL()` explicitly for missing data. SQL statement text
cannot contain NUL characters, although bound TEXT and BLOB values can. SQLite applies
the SQL expression and column affinity rules, so the stored or returned type may
differ from the original binding. Result conversion follows each value's SQLite
storage class, not the column's declared SQL type. TEXT must contain valid UTF-8;
invalid UTF-8 raises a database error. Select `CAST(column AS BLOB)` to retrieve
such a value as raw bytes. Result column names must also be valid UTF-8;
invalid names raise a catchable database error, even for empty results.
Use `CINT64("decimal digits")`
when constructing integer values too large for an exact Rice numeric literal.

## Reading result rows

Results contain all rows materialized when `DBQUERY` executes. Iterating does
not rerun the query and later database changes do not change those rows. In
particular, a DML statement with RETURNING performs its work during `DBQUERY`,
before the first `DBNEXT` call.

| Function | Result and behavior |
|---|---|
| `DBNEXT(result)` | Advance to the next row and return BOOL TRUE, or BOOL FALSE after the last row. Starts before the first row; once exhausted, further calls remain FALSE. |
| `DBVALUE(result, column)` | Read the current row's value with its native Rice type. |
| `DBISNULL(result, column)` | Whether the current value is SQLite NULL, as BOOL. |
| `DBCOLUMNTYPE$(result, column)` | Current value's SQLite storage class: `"NULL"`, `"INTEGER"`, `"REAL"`, `"TEXT"`, or `"BLOB"`. |
| `DBCOLUMNCOUNT(result)` | INT64 count of result columns, available before iteration and for empty results. |
| `DBCOLUMNNAME$(result, index)` | Column name at an exact 1-based index, available even for empty results. |
| `DBROWCOUNT(result)` | INT64 count of materialized rows, available before iteration. |
| `DBFREE(result)` | Release result memory and invalidate its handle. Returns BOOL TRUE. |

A `column` argument is either an exact 1-based index or a unique column name,
matched without ASCII case distinctions. Non-ASCII characters must match
exactly. Numeric indices always begin at 1, independent of
`OPTION BASE`. Unknown or ambiguous names raise errors; use SQL aliases or
numeric indices for duplicate names. Reading a value or its type requires a
successful `DBNEXT`; reads before the first row or after exhaustion fail.

SQL NULL is distinct from zero, an empty string, and an empty BLOB. Use
`DBISNULL` or `ISNULL` to test for it:

```basic
rows = DBQUERY(db, "SELECT NULL AS missing, '' AS text, X'' AS binary")
IF DBNEXT(rows) THEN
    item = DBVALUE(rows, "missing")
    PRINT ISNULL(item)                       ' TRUE
    PRINT TYPEOF$(item)                      ' NULL
    PRINT DBCOLUMNTYPE$(rows, "binary")       ' BLOB
END IF
CALL DBFREE(rows)
```

An unsuffixed implicit variable such as `item` can hold NULL. There is no
`AS NULL` declaration, and every declared type rejects NULL. Check for NULL
before assigning a nullable result into a typed variable, or use SQL `COALESCE`
to choose a fallback value. See [data types](data-types.md#null-values).

## Transactions and error recovery

| Function | Result and behavior |
|---|---|
| `DBBEGIN(db[, mode$])` | Begin a transaction; mode is DEFERRED (default), IMMEDIATE, or EXCLUSIVE, case-insensitively. Returns BOOL TRUE. |
| `DBCOMMIT(db)` | Commit the transaction. Returns BOOL TRUE. |
| `DBROLLBACK(db)` | Roll back the transaction. Returns BOOL TRUE. |
| `DBINTRANSACTION(db)` | Whether SQLite autocommit is disabled, as BOOL. |

SQLite transaction rules apply, including errors for nested BEGIN or COMMIT
without an active transaction. Use SQL SAVEPOINT/ROLLBACK TO/RELEASE through
`DBEXEC` for savepoints. SQL transaction commands and the helper functions share
the same connection state.

```basic
CALL DBBEGIN(db, "IMMEDIATE")
WHEN EXCEPTION IN
    CALL DBEXEC(db, "INSERT INTO notes (text) VALUES (?)", "first")
    CALL DBEXEC(db, "INSERT INTO notes (text) VALUES (?)", "second")
    CALL DBCOMMIT(db)
USE
    PRINT EXTEXT$
    IF DBINTRANSACTION(db) THEN CALL DBROLLBACK(db)
END WHEN
```

Handling an error does not automatically roll back a transaction. SQLite may
leave a transaction active or roll it back according to the error and SQL
conflict policy; inspect `DBINTRANSACTION` before recovery. A busy or failed
commit can leave the transaction open for retry or rollback. Query materialization
errors can occur after SQL has already changed data: for example, DML with
RETURNING can write before Rice discovers invalid UTF-8 in a returned value.
There is no implicit savepoint around a call. Use an explicit transaction and
rollback when a failed read/conversion must undo writes, and remember that some
PRAGMA settings are not transactional.

SQLite failures use classic `ERR = 90` or structured `EXTYPE = 9000`; the
diagnostic text includes SQLite's extended error code, or zero for a wrapper
error such as invalid returned UTF-8. Invalid handles, invalid
options, invalid column selections, and parameter-count errors use ordinary
Rice argument errors; incompatible argument values use type mismatch. Both
`WHEN EXCEPTION` and `ON ERROR` can handle database errors under their usual
scope rules. See [error handling](error-handling.md).

## Limits

Queries materialize their complete results in memory. Use SQL filtering, LIMIT,
and pagination for large datasets, and call `DBFREE` when finished. There is no
streaming cursor API, reusable prepared-statement handle, custom SQL function
registration, or extension-loading API. SQLite features are those included in
the bundled build; `DBVERSION$()` reports its version. Database access uses the
same host filesystem permissions as the running interpreter.
