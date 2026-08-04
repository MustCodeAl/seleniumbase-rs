/// Build a Selenium Grid hub URL from host, port, and optional path.
pub fn grid_url(host: &str, port: u16, path: Option<&str>) -> String {
    let path = path.unwrap_or("/wd/hub");
    format!("http://{}:{}{}", host, port, path)
}

/// Return a default local Grid URL.
pub fn local_grid_url() -> String {
    grid_url("localhost", 4444, Some("/wd/hub"))
}

/// Parse a Selenium Grid URL into its components.
pub fn parse_grid_url(url: &str) -> Option<(String, u16, String)> {
    // Strip scheme if present.
    let rest = url
        .trim_start_matches("http://")
        .trim_start_matches("https://");
    let (host_port, path) = match rest.find('/') {
        Some(idx) => (&rest[..idx], &rest[idx..]),
        None => (rest, "/"),
    };
    let (host, port_str) = match host_port.find(':') {
        Some(idx) => (&host_port[..idx], &host_port[idx + 1..]),
        None => (host_port, "80"),
    };
    let port = port_str.parse().ok()?;
    Some((host.to_string(), port, path.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grid_url() {
        assert_eq!(grid_url("hub", 4444, None), "http://hub:4444/wd/hub");
    }

    #[test]
    fn test_parse_grid_url() {
        let (host, port, path) = parse_grid_url("http://localhost:4444/wd/hub").unwrap();
        assert_eq!(host, "localhost");
        assert_eq!(port, 4444);
        assert_eq!(path, "/wd/hub");
    }
}
