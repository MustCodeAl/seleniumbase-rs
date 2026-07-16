pub fn get_proxy_auth_extension(username: &str, password: &str) -> String {
    format!("Proxy auth extension for user: {}", username)
}
