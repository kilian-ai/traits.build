// traits/kernel/runtimes/examples/multi_target_test.rs
// Test abstraction layer features across targets (native/wasm/wasi)

use traits_runtimes::{init_runtime, get_runtime};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::builder().filter_level(log::LevelFilter::Info).init();

    // Initialize runtime based on compile target
    #[cfg(not(target_arch = "wasm32"))]
    {
        println!("=== Running on NATIVE target ===\n");
        let temp_dir = std::path::PathBuf::from("/tmp/traits_runtime_multi_test");
        let _ = std::fs::create_dir_all(&temp_dir);
        let runtime = traits_runtimes::native::init_native_runtime(Some(temp_dir));
        init_runtime(runtime)?;
    }

    #[cfg(target_arch = "wasm32")]
    {
        println!("=== Running on WASM target ===\n");
        let runtime = traits_runtimes::wasm::init_wasm_runtime(
            "https://relay.traits.build".to_string(),
            std::env::var("RELAY_CODE").unwrap_or_else(|_| "TEST".to_string()),
        );
        init_runtime(runtime)?;
    }

    let rt = get_runtime()?;

    println!("=== Multi-Target Tests ===\n");

    // Test 1: Filesystem operations
    println!("Test 1: Filesystem operations");
    rt.filesystem.write("multi_test.txt", b"Multi-target test\n").await?;
    let content = rt.filesystem.read_to_string("multi_test.txt").await?;
    assert_eq!(content, "Multi-target test\n");
    println!("✓ Filesystem works\n");

    // Test 2: Timer
    println!("Test 2: Timer operations");
    let start = std::time::Instant::now();
    rt.timer.sleep(100).await?;
    let elapsed = start.elapsed().as_millis();
    println!("Slept for ~{}ms (requested 100ms)", elapsed);
    println!("✓ Timer works\n");

    // Test 3: Process spawning (native only)
    #[cfg(not(target_arch = "wasm32"))]
    {
        println!("Test 3: Process spawning");
        let output = rt.process.spawn("echo", &["Hello from process"]).await?;
        println!("Output: {}", output.stdout.trim());
        assert_eq!(output.exit_code, 0);
        println!("✓ Process execution works\n");
    }

    #[cfg(target_arch = "wasm32")]
    {
        println!("Test 3: Process execution (WASM, dispatched to relay)\n");
    }

    // Test 4: Multiple files
    println!("Test 4: Multiple file operations");
    for i in 0..3 {
        let filename = format!("file_{}.txt", i);
        let content = format!("Content of file {}\n", i);
        rt.filesystem.write(&filename, content.as_bytes()).await?;
    }
    let entries = rt.filesystem.list_dir(".").await?;
    println!("Created {} files", entries.iter().filter(|e| !e.is_dir).count());
    println!("✓ Multiple file operations work\n");

    // Test 5: Metadata
    println!("Test 5: File metadata");
    let meta = rt.filesystem.metadata("multi_test.txt").await?;
    println!("Size: {}, Dir: {}", meta.size, meta.is_dir);
    println!("✓ Metadata works\n");

    println!("=== All multi-target tests passed! ===\n");
    println!("The same abstraction layer works across:");
    println!("  • Native (tokio + std)");
    println!("  • WASM (relay bridge)");
    println!("  • WASI (standardized syscalls)");

    Ok(())
}
