// traits/kernel/runtimes/examples/file_ops.rs
// Test filesystem abstraction incrementally

use traits_runtimes::{init_runtime, get_runtime};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::builder().filter_level(log::LevelFilter::Info).init();

    // Initialize native runtime with a working temp directory
    let temp_dir = PathBuf::from("/tmp/traits_runtime_test");
    // Create it manually since our abstraction doesn't have a pre-init filesystem
    let _ = std::fs::create_dir_all(&temp_dir);

    let runtime = traits_runtimes::native::init_native_runtime(Some(temp_dir));
    init_runtime(runtime)?;

    let rt = get_runtime()?;

    println!("=== Filesystem Tests ===\n");

    // Test 1: Create a file
    println!("Test 1: Create and write file");
    rt.filesystem.write("test.txt", b"Hello, traits-runtimes!\n").await?;
    println!("✓ Created test.txt\n");

    // Test 2: Read the file
    println!("Test 2: Read file");
    let content = rt.filesystem.read_to_string("test.txt").await?;
    println!("Content: {}", content);
    assert_eq!(content, "Hello, traits-runtimes!\n");
    println!("✓ Read succeeded\n");

    // Test 3: Create a directory
    println!("Test 3: Create directory");
    rt.filesystem.mkdir("subdir").await?;
    println!("✓ Created subdir\n");

    // Test 4: Write file in subdirectory
    println!("Test 4: Write file in subdirectory");
    rt.filesystem.write("subdir/nested.txt", b"Nested content\n").await?;
    println!("✓ Created subdir/nested.txt\n");

    // Test 5: List directory
    println!("Test 5: List directory");
    let entries = rt.filesystem.list_dir(".").await?;
    println!("Directory contents:");
    for entry in entries {
        let marker = if entry.is_dir { "/" } else { "" };
        println!("  - {}{} ({}B)", entry.name, marker, entry.size);
    }
    println!("✓ List succeeded\n");

    // Test 6: Get metadata
    println!("Test 6: Get metadata");
    let meta = rt.filesystem.metadata("test.txt").await?;
    println!("test.txt: is_dir={}, size={}B, writable={}", meta.is_dir, meta.size, meta.is_writable);
    assert!(!meta.is_dir);
    println!("✓ Metadata read succeeded\n");

    // Test 7: Read to string (another file)
    println!("Test 7: Read nested file");
    let nested = rt.filesystem.read_to_string("subdir/nested.txt").await?;
    println!("Nested content: {}", nested);
    assert_eq!(nested, "Nested content\n");
    println!("✓ Nested read succeeded\n");

    // Test 8: Remove file
    println!("Test 8: Remove file");
    rt.filesystem.remove_file("test.txt").await?;
    println!("✓ Removed test.txt\n");

    println!("=== All filesystem tests passed! ===");
    Ok(())
}
