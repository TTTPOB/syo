use std::net::TcpListener;

/// Ask the OS for an unused TCP port on 127.0.0.1, then immediately release it.
///
/// There is a tiny race window between releasing and the caller binding, but it is
/// acceptable for test orchestration.
pub fn allocate_loopback_port() -> std::io::Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    drop(listener);
    Ok(port)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocates_unique_ports() {
        let a = allocate_loopback_port().unwrap();
        let b = allocate_loopback_port().unwrap();
        assert_ne!(a, b, "two consecutive allocations should not collide");
        assert!(a >= 1024, "should be in the unprivileged range, got {a}");
    }

    #[test]
    fn allocated_port_is_actually_bindable() {
        let port = allocate_loopback_port().unwrap();
        let _bound = TcpListener::bind(("127.0.0.1", port))
            .expect("port returned by allocate_loopback_port should be bindable");
    }
}
