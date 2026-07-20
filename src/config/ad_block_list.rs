#[derive(Debug, Clone, Default)]
pub struct AdBlockList {
    domains: Vec<String>,
}

impl AdBlockList {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_list(domains: Vec<String>) -> Self {
        Self { domains }
    }

    pub fn add_domain(&mut self, domain: impl Into<String>) {
        self.domains.push(domain.into());
    }

    pub fn is_blocked(&self, url: &str) -> bool {
        match extract_host(url) {
            Some(host) => self.domains.iter().any(|d| matches_domain(host, d)),
            None => self.domains.iter().any(|d| url.contains(d)),
        }
    }

    pub fn len(&self) -> usize {
        self.domains.len()
    }

    pub fn is_empty(&self) -> bool {
        self.domains.is_empty()
    }
}

fn extract_host(url: &str) -> Option<&str> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let end = rest.find('/').unwrap_or(rest.len());
    let host = &rest[..end];
    Some(host.split(':').next().unwrap_or(host))
}

fn matches_domain(host: &str, blocked: &str) -> bool {
    host == blocked || host.ends_with(&format!(".{}", blocked))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_exact_domain() {
        let list = AdBlockList::from_list(vec!["ads.example.com".to_string()]);
        assert!(list.is_blocked("https://ads.example.com/banner.png"));
        assert!(!list.is_blocked("https://example.com/"));
    }

    #[test]
    fn blocks_subdomain() {
        let list = AdBlockList::from_list(vec!["doubleclick.net".to_string()]);
        assert!(list.is_blocked("https://googleads.doubleclick.net/track"));
        assert!(!list.is_blocked("https://safe-site.com/"));
    }

    #[test]
    fn add_domain_and_len() {
        let mut list = AdBlockList::new();
        list.add_domain("tracker.io");
        assert_eq!(list.len(), 1);
        assert!(list.is_blocked("http://tracker.io/pixel.gif"));
    }

    #[test]
    fn empty_list_allows_everything() {
        let list = AdBlockList::new();
        assert!(!list.is_blocked("https://any-site.com/"));
    }
}
