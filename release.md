# Release Notes v1.3.1

## 📚 **Documentation**

### Added Claude Code Skill

Added `.claude/skills/using-jankensqlhub/SKILL.md` — a Claude Code agent skill that provides JankenSQLHub usage guidance. This skill is automatically discovered by Claude Code when working in this repository and covers:

- Parameter syntax (`@param`, `#[param]`, `:[param]`, `~[param]`)
- Query definition structure and type system
- Constraint configuration (`enum`, `enumif`, `range`, `pattern`)
- Dynamic returns with `~[fields]`
- SQLite and PostgreSQL execution examples
- Structured error handling with `JankenError`

---

**Version 1.3.1** - Added Claude Code agent skill for JankenSQLHub usage guidance