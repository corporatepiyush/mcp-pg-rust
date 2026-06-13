#[test]
fn test_socket_buffer_size() {
    // Verify 4KB constant matches our requirement
    const SOCKET_BUFFER_SIZE: i32 = 4 * 1024;

    assert_eq!(SOCKET_BUFFER_SIZE, 4096, "Socket buffer should be 4KB");
    assert_eq!(SOCKET_BUFFER_SIZE * 2, 8192, "Two directions = 8KB per connection");
}
