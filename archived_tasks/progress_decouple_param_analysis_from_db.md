# Decouple Parameter Analysis from Database Logic

## Task Goal
Refactor parameter preparation logic to separate SQL analysis/validation from database-specific type conversions, making the code more modular and reusable between SQLite and PostgreSQL runners.

## Current Issues

### Tight Coupling Between Analysis and Execution
- **Problem**: Parameter preparation functions directly convert JSON to database-specific `ToSql` types
- **Impact**: Cannot share common parameter analysis logic between SQLite and PostgreSQL runners
- **Example**: `parameter_value_to_postgres()` and `parameter_value_to_sqlite()` are nearly identical but duplicated

### Mixed Responsibilities
- **Problem**: Single functions handle both SQL token replacement AND type conversion
- **Impact**: Database-agnostic logic is mixed with database-specific logic
- **Example**: `prepare_statement_parameters()` converts JSON values during parameter collection

## Proposed Architecture

### 1. Generic Parameter Preparation
```rust
struct PreparedParameterStatement {
    sql: String,
    parameters: Vec<ParameterValue>,
}

enum ParameterValue {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Blob(Vec<u8>),
}

fn prepare_parameter_statement_generic(
    sql: &str,
    all_parameters: &[Parameter],
    request_params: &serde_json::Map<String, serde_json::Value>,
) -> Result<PreparedParameterStatement>
```

### 2. Database-Specific Conversions
```rust
// SQLite runner - converts generic to rusqlite::ToSql
impl From<ParameterValue> for rusqlite::ToSql { ... }

// PostgreSQL runner - converts generic to tokio_postgres::types::ToSql
impl From<ParameterValue> for Box<dyn tokio_postgres::types::ToSql + Sync> { ... }
```

### 3. Runner-Specific Adapters
```rust
fn execute_with_sqlite(statement: PreparedParameterStatement, tx: &Transaction) -> Result<()>
fn execute_with_postgresql(statement: PreparedParameterStatement, tx: &Transaction) -> Result<()>
```

## Implementation Steps

### Phase 1: Extract Generic Parameter Logic
- [ ] Create `ParameterValue` enum to represent typed parameter values generically
- [ ] Implement `json_value_to_parameter_value()` function for JSON-to-generic conversion
- [ ] Create `prepare_parameter_statement_generic()` that uses generic types
- [ ] Update table name replacement logic to work with generic parameters
- [ ] Update list parameter expansion to work with generic parameters

### Phase 2: Update SQLite Runner
- [ ] Implement `From<ParameterValue> for rusqlite::ToSql` trait
- [ ] Update SQLite runner to use generic preparation + database-specific conversion
- [ ] Ensure all SQLite tests pass

### Phase 3: Update PostgreSQL Runner
- [ ] Implement `From<ParameterValue> for Box<dyn tokio_postgres::types::ToSql>` trait
- [ ] Update PostgreSQL runner to use generic preparation + database-specific conversion
- [ ] Ensure all PostgreSQL tests pass

### Phase 4: Code Cleanup
- [ ] Remove duplicate parameter conversion logic
- [ ] Simplify runner-specific code
- [ ] Update documentation

## Benefits

### Better Modularity
- **Separation of Concerns**: SQL analysis is separate from database binding
- **Reusable Logic**: Common parameter validation works across all databases
- **Easier Testing**: Can test parameter analysis without database dependencies

### Reduced Duplication
- **Single Source**: Parameter validation logic exists in one place
- **Consistent Behavior**: Same validation rules across databases
- **Easier Maintenance**: Bug fixes apply to all database backends

### Future Extensibility
- **New Databases**: Adding new database support only requires implementing `From<ParameterValue>`
- **Type Safety**: Generic enum ensures type consistency across conversion layers
- **Performance**: Type conversions happen at execution time, not preparation time

## Current Status
- [x] Task defined
- [x] Phase 1: Extract Generic Parameter Logic - COMPLETED
  - [x] Create ParameterValue enum
  - [x] Implement json_value_to_parameter_value function
  - [x] Create prepare_parameter_statement_generic function
  - [x] Update table name replacement logic
  - [x] Update list parameter expansion logic

- [x] Phase 2: Update SQLite Runner - COMPLETED ✓
  - [x] Implement `From<ParameterValue> for rusqlite::ToSql` trait
  - [x] Create `prepare_single_statement_generic()` function using decoupled approach
  - [x] Proven table names ARE fully generic (independent of database)
  - [x] Switch existing SQLite functions to use generic preparation instead of old approach
  - [x] Ensure all SQLite tests pass (backward compatibility verified)

- [x] Phase 3: Update PostgreSQL Runner - COMPLETED ✓
  - [x] Implement `From<ParameterValue> for Box<dyn tokio_postgres::types::ToSql + Sync>` trait
  - [x] Create `prepare_single_statement_postgresql()` function using decoupled approach
  - [x] Switch existing PostgreSQL functions to use generic preparation instead of old approach
  - [x] Ensure all PostgreSQL tests pass (backward compatibility verified)

## Verification Criteria
- All existing tests pass (SQLite + PostgreSQL)
- Code coverage maintained or improved
- No performance regressions
- Cleaner, more maintainable codebase
