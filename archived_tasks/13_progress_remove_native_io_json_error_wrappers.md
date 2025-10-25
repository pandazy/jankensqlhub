# Remove Native IO/JSON/Regex Error Wrappers - Progress

## Task Description
Remove `JankenError::Io`, `JankenError::Json`, and `JankenError::Regex` variants and stop wrapping native I/O, JSON parsing, and regex compilation errors. Instead, allow these native errors to bubble up directly through `anyhow::Result` without custom wrapping.

## Goals
- Simplify error handling by not wrapping native external library errors in custom types
- Allow users to handle these errors directly with their native APIs
- Reduce the complexity of the error enum
- Follow the same pattern as removing Sqlite/Postgres error wrapping

## Todo List
- [x] Analyze current usage of Io/Json/Regex errors throughout codebase
- [x] Create progress file and update .clinerules
- [x] Remove JankenError::Io, JankenError::Json, and JankenError::Regex variants from result.rs
- [x] Remove ERR_CODE_IO, ERR_CODE_JSON, and ERR_CODE_REGEX constants
- [x] Remove new_io/new_json/new_regex constructor methods
- [x] Remove From<std::io::Error>, From<serde_json::Error>, and From<regex::Error> implementations
- [x] Remove Io/Json/Regex entries from ERROR_MAPPINGS
- [x] Remove Io/Json/Regex arms from get_error_data() function
- [x] Change QueryDefinitions::from_file() to return anyhow::Result<Self> for native file I/O errors
- [x] Update integration tests to handle native I/O/JSON errors via anyhow and downcast patterns
- [x] Update unit tests to remove Io/Json/Regex error testing
- [x] Remove regex-related constants and mappings that are no longer used
- [x] Update lib.rs to not re-export removed error constants
- [x] Run cargo check, clippy, and fmt to ensure code quality
- [x] Update progress documentation

## Summary

The JankenError enum has been significantly simplified by removing variants that wrapped external library errors:

**Before:**
```rust
pub enum JankenError {
    Io { source: std::io::Error, data: ErrorData },
    Json { source: serde_json::Error, data: ErrorData },
    Regex { source: regex::Error, data: ErrorData },
    QueryNotFound { data: ErrorData },
    // ... parameter validation errors
}
```

**After:**
```rust
pub enum JankenError {
    QueryNotFound { data: ErrorData },
    ParameterNotProvided { data: ErrorData },
    ParameterTypeMismatch { data: ErrorData },
    ParameterNameConflict { data: ErrorData },
}
```

**Changes Made:**
1. **Error Enum Cleanup:** Removed Io, Json, and Regex variants from JankenError
2. **API Changes:** QueryDefinitions::from_file() now returns `anyhow::Result<Self>` instead of `Result<Self>`, allowing native I/O and JSON errors to bubble up directly
3. **Constants and Mappings:** Removed ERR_CODE_IO, ERR_CODE_JSON, ERR_CODE_REGEX constants and their entries in ERROR_MAPPINGS
4. **Constructor Methods:** Removed new_io(), new_json(), new_regex() methods
5. **From Implementations:** Removed From trait implementations for std::io::Error, serde_json::Error, and regex::Error
6. **Library Interface:** Updated lib.rs to not re-export removed error codes
7. **Test Updates:** All tests updated to use downcast patterns for native errors instead of expecting JankenError variants
8. **Compatibility:** Added JankenError -> anyhow::Error conversion for seamless integration

**Benefits:**
- **Cleaner Error Handling:** External library errors are handled directly without intermediate wrapping
- **Better Type Safety:** Users get the actual error types they expect from standard libraries
- **Reduced Complexity:** Smaller, more focused error enum
- **Consistent Pattern:** Follows the same approach as the Sqlite/Postgres error removal

The codebase now compiles successfully and passes `cargo check`, `cargo clippy`, and `cargo fmt`.
