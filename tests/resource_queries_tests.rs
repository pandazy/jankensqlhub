use jankensqlhub::{DatabaseConnection, QueryDefinitions, QueryRunner};
use rusqlite::Connection;

fn setup_resource_schema() -> Connection {
    let conn = Connection::open_in_memory().unwrap();

    // Create the four tables based on the schema provided
    conn.execute(
        "CREATE TABLE shows (
        id INTEGER PRIMARY KEY,
        name TEXT,
        context TEXT,
        created_at INTEGER,
        updated_at INTEGER,
        status TEXT
    )",
        [],
    )
    .unwrap();

    conn.execute(
        "CREATE TABLE songs (
        id INTEGER PRIMARY KEY,
        name TEXT,
        artist_id TEXT,
        created_at INTEGER,
        updated_at INTEGER,
        status TEXT
    )",
        [],
    )
    .unwrap();

    conn.execute(
        "CREATE TABLE artists (
        id INTEGER PRIMARY KEY,
        name TEXT,
        created_at INTEGER,
        updated_at INTEGER,
        status TEXT
    )",
        [],
    )
    .unwrap();

    conn.execute(
        "CREATE TABLE rel_show_song (
        id INTEGER PRIMARY KEY,
        show_id INTEGER,
        song_id INTEGER,
        created_at INTEGER
    )",
        [],
    )
    .unwrap();

    // Insert test data
    conn.execute(
        "INSERT INTO shows VALUES (1, 'Rock Concert', 'Stadium', 1234567890, 1234567890, 'active')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO shows VALUES (2, 'Jazz Night', 'Club', 1234567891, 1234567891, 'active')",
        [],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO artists VALUES (1, 'The Rockers', 1234567890, 1234567890, 'active')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO artists VALUES (2, 'Jazz Masters', 1234567891, 1234567891, 'active')",
        [],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO songs VALUES (1, 'Thunder', 1, 1234567890, 1234567890, 'active')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO songs VALUES (2, 'Smooth Jazz', 2, 1234567891, 1234567891, 'active')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO songs VALUES (3, 'Bass Line', 1, 1234567892, 1234567892, 'active')",
        [],
    )
    .unwrap();

    conn.execute("INSERT INTO rel_show_song VALUES (1, 1, 1, 1234567890)", [])
        .unwrap(); // Rock Concert has Thunder
    conn.execute("INSERT INTO rel_show_song VALUES (2, 1, 3, 1234567890)", [])
        .unwrap(); // Rock Concert has Bass Line
    conn.execute("INSERT INTO rel_show_song VALUES (3, 2, 2, 1234567891)", [])
        .unwrap(); // Jazz Night has Smooth Jazz

    conn
}

#[test]
fn test_resource_queries_json_loading() {
    // Test that the new resource_queries.json can be loaded successfully
    let queries = QueryDefinitions::from_file("test_json/resource_queries.json").unwrap();

    // Test that all expected queries are present
    assert!(queries.definitions.contains_key("select_by_ids"));
    assert!(queries.definitions.contains_key("select_children"));
    assert!(
        queries
            .definitions
            .contains_key("select_related_shows_songs")
    );
    assert!(queries.definitions.contains_key("select_shows_via_songs"));
    assert!(queries.definitions.contains_key("select_songs_via_shows"));
    assert!(queries.definitions.contains_key("select_songs_via_artists"));
}

#[test]
fn test_select_by_ids() {
    let queries = QueryDefinitions::from_file("test_json/resource_queries.json").unwrap();
    let conn = setup_resource_schema();
    let mut db_conn = DatabaseConnection::SQLite(conn);

    // Test selecting shows by IDs
    let params = serde_json::json!({"resource": "shows", "ids": [1, 2]});
    let result = db_conn
        .query_run(&queries, "select_by_ids", &params)
        .unwrap();

    assert_eq!(result.data.len(), 2);
    let names: Vec<String> = result
        .data
        .iter()
        .map(|row| row.get("name").unwrap().as_str().unwrap().to_string())
        .collect();
    assert!(names.contains(&"Rock Concert".to_string()));
    assert!(names.contains(&"Jazz Night".to_string()));
}

#[test]
fn test_select_children() {
    let queries = QueryDefinitions::from_file("test_json/resource_queries.json").unwrap();
    let conn = setup_resource_schema();
    let mut db_conn = DatabaseConnection::SQLite(conn);

    // Test selecting songs by artist_id (children of artists)
    let params =
        serde_json::json!({"resource": "songs", "foreign_key": "artist_id", "f_ids": ["1", "2"]});
    let result = db_conn
        .query_run(&queries, "select_children", &params)
        .unwrap();

    assert_eq!(result.data.len(), 3); // Thunder and Bass Line for artist 1, Smooth Jazz for artist 2

    let song_names: Vec<String> = result
        .data
        .iter()
        .map(|row| row.get("name").unwrap().as_str().unwrap().to_string())
        .collect();
    assert!(song_names.contains(&"Thunder".to_string()));
    assert!(song_names.contains(&"Smooth Jazz".to_string()));
    assert!(song_names.contains(&"Bass Line".to_string()));
}

#[test]
fn test_select_related_shows_songs() {
    let queries = QueryDefinitions::from_file("test_json/resource_queries.json").unwrap();
    let conn = setup_resource_schema();
    let mut db_conn = DatabaseConnection::SQLite(conn);

    // Test selecting related shows and songs through rel_show_song
    let params = serde_json::json!({});
    let result = db_conn
        .query_run(&queries, "select_related_shows_songs", &params)
        .unwrap();

    assert_eq!(result.data.len(), 3); // 3 relationships total

    // First relationship: Rock Concert (show) - Thunder (song)
    let first_rel = &result.data[0];
    assert_eq!(
        first_rel.get("show_name").unwrap().as_str().unwrap(),
        "Rock Concert"
    );
    assert_eq!(
        first_rel.get("song_name").unwrap().as_str().unwrap(),
        "Thunder"
    );
    assert_eq!(first_rel.get("rel_id").unwrap().as_i64().unwrap(), 1);
    assert_eq!(first_rel.get("show_id").unwrap().as_i64().unwrap(), 1);
    assert_eq!(first_rel.get("song_id").unwrap().as_i64().unwrap(), 1);
}

#[test]
fn test_select_shows_via_songs() {
    let queries = QueryDefinitions::from_file("test_json/resource_queries.json").unwrap();
    let conn = setup_resource_schema();
    let mut db_conn = DatabaseConnection::SQLite(conn);

    // Test selecting shows that have specific songs
    let params = serde_json::json!({"song_ids": [1, 2]});
    let result = db_conn
        .query_run(&queries, "select_shows_via_songs", &params)
        .unwrap();

    assert_eq!(result.data.len(), 2); // Show 1 (Thunder), Show 2 (Smooth Jazz)

    let show_names: Vec<String> = result
        .data
        .iter()
        .map(|row| row.get("name").unwrap().as_str().unwrap().to_string())
        .collect();
    assert!(show_names.contains(&"Rock Concert".to_string()));
    assert!(show_names.contains(&"Jazz Night".to_string()));
}

#[test]
fn test_select_songs_via_shows() {
    let queries = QueryDefinitions::from_file("test_json/resource_queries.json").unwrap();
    let conn = setup_resource_schema();
    let mut db_conn = DatabaseConnection::SQLite(conn);

    // Test selecting songs that belong to specific shows
    let params = serde_json::json!({"show_ids": [1]});
    let result = db_conn
        .query_run(&queries, "select_songs_via_shows", &params)
        .unwrap();

    assert_eq!(result.data.len(), 2); // Show 1 has Thunder and Bass Line

    let song_names: Vec<String> = result
        .data
        .iter()
        .map(|row| row.get("name").unwrap().as_str().unwrap().to_string())
        .collect();
    assert!(song_names.contains(&"Thunder".to_string()));
    assert!(song_names.contains(&"Bass Line".to_string()));
}

#[test]
fn test_select_songs_via_artists() {
    let queries = QueryDefinitions::from_file("test_json/resource_queries.json").unwrap();
    let conn = setup_resource_schema();
    let mut db_conn = DatabaseConnection::SQLite(conn);

    // Test selecting songs that belong to specific artists
    let params = serde_json::json!({"artist_ids": ["1"]});
    let result = db_conn
        .query_run(&queries, "select_songs_via_artists", &params)
        .unwrap();

    assert_eq!(result.data.len(), 2); // Artist 1 has Thunder and Bass Line

    let song_names: Vec<String> = result
        .data
        .iter()
        .map(|row| row.get("name").unwrap().as_str().unwrap().to_string())
        .collect();
    assert!(song_names.contains(&"Thunder".to_string()));
    assert!(song_names.contains(&"Bass Line".to_string()));
}
