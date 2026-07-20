use rand::RngExt;

#[derive(Debug, Clone, Default)]
pub struct ProxyList {
    proxies: Vec<String>,
    index: usize,
}

impl ProxyList {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_list(proxies: Vec<String>) -> Self {
        Self { proxies, index: 0 }
    }

    pub fn add(&mut self, proxy: impl Into<String>) {
        self.proxies.push(proxy.into());
    }

    pub fn is_empty(&self) -> bool {
        self.proxies.is_empty()
    }

    pub fn len(&self) -> usize {
        self.proxies.len()
    }

    pub fn next_round_robin(&mut self) -> Option<&str> {
        if self.proxies.is_empty() {
            return None;
        }
        let proxy = &self.proxies[self.index];
        self.index = (self.index + 1) % self.proxies.len();
        Some(proxy)
    }

    pub fn random(&self) -> Option<&str> {
        if self.proxies.is_empty() {
            return None;
        }
        let mut rng = rand::rng();
        let idx = rng.random_range(0..self.proxies.len());
        Some(&self.proxies[idx])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_robin_cycles_through_proxies() {
        let mut list = ProxyList::from_list(vec![
            "http://a:8080".to_string(),
            "http://b:8080".to_string(),
        ]);
        assert_eq!(list.next_round_robin(), Some("http://a:8080"));
        assert_eq!(list.next_round_robin(), Some("http://b:8080"));
        assert_eq!(list.next_round_robin(), Some("http://a:8080"));
    }

    #[test]
    fn empty_list_returns_none() {
        let mut list = ProxyList::new();
        assert!(list.is_empty());
        assert_eq!(list.next_round_robin(), None);
        assert_eq!(list.random(), None);
    }

    #[test]
    fn add_and_len() {
        let mut list = ProxyList::new();
        list.add("http://p:8080");
        assert_eq!(list.len(), 1);
        assert_eq!(list.next_round_robin(), Some("http://p:8080"));
    }

    #[test]
    fn random_returns_member() {
        let proxies = vec![
            "http://a:8080".to_string(),
            "http://b:8080".to_string(),
            "http://c:8080".to_string(),
        ];
        let list = ProxyList::from_list(proxies.clone());
        let choice = list.random().unwrap();
        assert!(proxies.iter().any(|p| p == choice));
    }
}
