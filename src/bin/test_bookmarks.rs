use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use thor::dx::bookmarks::BookmarkStore;

fn main() {
    println!("Running Bookmark Storage Tests...");
    
    let temp_dir = std::env::temp_dir().join(format!("thor_bm_test_{}", std::time::UNIX_EPOCH.elapsed().unwrap().as_nanos()));
    fs::create_dir_all(&temp_dir).unwrap();
    let json_path = temp_dir.join("bookmarks.json");

    // 1. Malformed Chrome JSON import
    test_malformed_json(&temp_dir);

    // 2. Large Dataset
    test_large_dataset(&temp_dir);

    // 3. Persistence Race Conditions
    test_persistence_race_conditions(&json_path);
    
    println!("Cleaning up...");
    let _ = fs::remove_dir_all(&temp_dir);
    println!("Tests completed.");
}

fn test_malformed_json(dir: &PathBuf) {
    println!("-- Testing Malformed JSON --");
    let mut store = BookmarkStore::open_in(dir).unwrap();
    
    // Invalid JSON format
    let invalid_json = r#"{ "roots": { "bookmark_bar": { "children": [ { "type": "url", "name": "Missing Quotes } ] } } }"#;
    let res = store.import_chrome_json(invalid_json);
    println!("Invalid JSON import result: {:?}", res);
    
    // Missing required fields
    let missing_fields = r#"{
        "roots": {
            "bookmark_bar": {
                "type": "folder",
                "name": "bookmark_bar",
                "children": [
                    { "type": "url", "url": "https://example.com" } 
                ]
            }
        }
    }"#;
    // (Notice: name is missing, but parse_chrome_bookmarks might handle it)
    let res = store.import_chrome_json(missing_fields);
    println!("Missing fields import result: {:?}", res);
}

fn test_large_dataset(dir: &PathBuf) {
    println!("-- Testing Large Dataset --");
    let mut store = BookmarkStore::open_in(dir).unwrap();
    
    println!("Adding 10,000 bookmarks...");
    let start = std::time::Instant::now();
    for i in 0..10000 {
        store.add(format!("Bookmark {}", i), format!("https://example.com/{}", i)).unwrap();
    }
    println!("Added 10,000 bookmarks in {:?}", start.elapsed());
    println!("Store list length: {}", store.list().len());
}

fn test_persistence_race_conditions(path: &PathBuf) {
    println!("-- Testing Persistence Race Conditions --");
    
    // Pre-populate with something
    {
        let mut store = BookmarkStore::open_at(path).unwrap();
        store.add("Initial", "https://initial.com").unwrap();
    }
    
    let path_arc = Arc::new(path.clone());
    let mut handles = vec![];
    
    // Spawn multiple threads trying to add bookmarks concurrently
    for i in 0..10 {
        let p = path_arc.clone();
        handles.push(thread::spawn(move || {
            for j in 0..100 {
                // Read and write concurrently
                let mut store = BookmarkStore::open_at(&*p).unwrap_or_else(|_| BookmarkStore::open_at(&*p).unwrap());
                let _ = store.add(format!("Thread {} Item {}", i, j), format!("https://example.com/{}/{}", i, j));
            }
        }));
    }
    
    for h in handles {
        let _ = h.join();
    }
    
    // Check if the file is still a valid JSON
    let content = fs::read_to_string(path).unwrap();
    println!("File size after race condition: {} bytes", content.len());
    
    let res: Result<serde_json::Value, _> = serde_json::from_str(&content);
    match res {
        Ok(json) => {
            println!("File is still valid JSON! (Unexpected, it should probably be corrupted without locking)");
            if let Some(arr) = json.get("bookmarks").and_then(|v| v.as_array()) {
                println!("Contains {} bookmarks", arr.len());
            }
        }
        Err(e) => {
            println!("BUG FOUND: File corruption due to race condition!");
            println!("JSON Parsing Error: {}", e);
        }
    }
}
