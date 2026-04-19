// traits/kernel/runtimes/examples/socket_echo.rs
// Test socket abstraction incrementally

#[cfg(not(target_arch = "wasm32"))]
use std::net::SocketAddr;
#[cfg(not(target_arch = "wasm32"))]
use tokio::task;
#[cfg(not(target_arch = "wasm32"))]
use traits_runtimes::{get_runtime, init_runtime};

#[cfg(not(target_arch = "wasm32"))]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::builder().filter_level(log::LevelFilter::Info).init();

    // Initialize native runtime
    let runtime = traits_runtimes::native::init_native_runtime(None);
    init_runtime(runtime)?;

    let rt = get_runtime()?;

    println!("=== Socket Tests ===\n");

    // Test 1: Create a listener
    println!("Test 1: Bind to socket");
    let addr: SocketAddr = "127.0.0.1:19999".parse()?;
    let mut listener = rt.socket_factory.bind(addr).await?;
    println!("✓ Listening on {}\n", addr);

    let listener_addr = listener.local_addr()?;
    println!("Local address: {}\n", listener_addr);

    // Test 2: Connect to the listener (in background)
    println!("Test 2: Accept connection");
    let server_handle = task::spawn(async move {
        match listener.accept().await {
            Ok((mut socket, peer_addr)) => {
                println!("✓ Accepted connection from {}", peer_addr);
                
                // Read echo message
                let mut buf = [0u8; 1024];
                match socket.read(&mut buf).await {
                    Ok(n) => {
                        let msg = String::from_utf8_lossy(&buf[..n]);
                        println!("Server received: {}", msg);
                        
                        // Echo back
                        if let Err(e) = socket.write_all(format!("ECHO: {}", msg).as_bytes()).await {
                            println!("Write error: {}", e);
                        }
                    }
                    Err(e) => println!("Read error: {}", e),
                }
            }
            Err(e) => println!("Accept error: {}", e),
        }
    });

    // Give server time to start accepting
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Test 3: Connect as client
    println!("\nTest 3: Connect as client");
    let mut client_socket = rt.socket_factory.connect(addr).await?;
    println!("✓ Connected to {}", addr);

    // Test 4: Send message
    println!("\nTest 4: Send message");
    client_socket.write_all(b"Hello from client!\n").await?;
    println!("✓ Sent message\n");

    // Test 5: Read response
    println!("Test 5: Read response");
    let mut buf = [0u8; 1024];
    let n = client_socket.read(&mut buf).await?;
    let response = String::from_utf8_lossy(&buf[..n]);
    println!("Client received: {}", response);
    println!("✓ Response received\n");

    // Wait for server to finish
    server_handle.await?;

    println!("=== All socket tests passed! ===");
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn main() {
    let _runtime = traits_runtimes::wasm::init_wasm_runtime(
        "https://relay.traits.build".to_string(),
        "TEST".to_string(),
    );
    println!("socket_echo compiles for wasm32 against the relay-backed socket boundary");
}
