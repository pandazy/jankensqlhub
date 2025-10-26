# Release Notes v1.0.1

## 🔧 **Table Name Handling Simplification**

### Removed Table Name Quote Handling
- **Improved string concatenation**: Removed quote wrapper handling to avoid complexity in query building
- **Streamlined processing**: Since table/column names are already filtered (alphanumeric + underscores only), quotes are unnecessary
- **Reduced complexity**: Eliminates need for quote escaping and manipulation in SQL generation

---

**Version 1.0.1** - Simplified table name handling by removing unnecessary quotes
