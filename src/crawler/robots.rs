//! Zero-copy `robots.txt` parser and policy matcher.
//!
//! Provides [`RobotsPolicy`] to parse `robots.txt` files without unnecessary allocations
//! and evaluate URL crawling permissions, crawl delays, and sitemap directives
//! per RFC 9309.

use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;
use std::time::Duration;

use crate::error::SeleniumBaseError;

/// Type of robots directive rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuleType {
    /// Explicit permission to crawl matching paths.
    Allow,
    /// Restriction disallowing crawling matching paths.
    Disallow,
}

impl fmt::Display for RuleType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Allow => write!(f, "Allow"),
            Self::Disallow => write!(f, "Disallow"),
        }
    }
}

/// An individual path pattern rule within a `robots.txt` record group.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RobotsRule<'a> {
    rule_type: RuleType,
    path: Cow<'a, str>,
}

impl<'a> RobotsRule<'a> {
    /// Creates a new rule.
    #[must_use]
    pub fn new(rule_type: RuleType, path: Cow<'a, str>) -> Self {
        Self { rule_type, path }
    }

    /// The directive type (`Allow` or `Disallow`).
    #[must_use]
    pub const fn rule_type(&self) -> RuleType {
        self.rule_type
    }

    /// The pattern path borrowed from the original `robots.txt`.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Converts this rule into an owned variant with `'static` lifetime.
    #[must_use]
    pub fn into_owned(self) -> RobotsRule<'static> {
        RobotsRule {
            rule_type: self.rule_type,
            path: Cow::Owned(self.path.into_owned()),
        }
    }
}

/// A grouped record of user-agents, directive rules, and optional crawl delay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RobotsGroup<'a> {
    user_agents: Vec<Cow<'a, str>>,
    rules: Vec<RobotsRule<'a>>,
    crawl_delay: Option<Duration>,
}

impl<'a> RobotsGroup<'a> {
    /// User agents this record applies to.
    #[must_use]
    pub fn user_agents(&self) -> &[Cow<'a, str>] {
        &self.user_agents
    }

    /// Rules contained in this group.
    #[must_use]
    pub fn rules(&self) -> &[RobotsRule<'a>] {
        &self.rules
    }

    /// Crawl delay specified for this group, if any.
    #[must_use]
    pub const fn crawl_delay(&self) -> Option<Duration> {
        self.crawl_delay
    }

    /// Converts this group into an owned variant with `'static` lifetime.
    #[must_use]
    pub fn into_owned(self) -> RobotsGroup<'static> {
        RobotsGroup {
            user_agents: self
                .user_agents
                .into_iter()
                .map(|ua| Cow::Owned(ua.into_owned()))
                .collect(),
            rules: self.rules.into_iter().map(RobotsRule::into_owned).collect(),
            crawl_delay: self.crawl_delay,
        }
    }
}

/// Zero-copy `robots.txt` parser and policy matcher.
///
/// Implements RFC 9309 path matching, user-agent selection, crawl delay extraction,
/// and sitemap discovery.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::crawler::RobotsPolicy;
///
/// let robots = r#"
///     User-agent: Googlebot
///     Disallow: /admin/
///     Allow: /admin/public/
///
///     User-agent: *
///     Disallow: /private/
///     Crawl-delay: 2.5
///     Sitemap: https://example.com/sitemap.xml
/// "#;
///
/// let policy = RobotsPolicy::parse(robots);
/// assert!(policy.is_allowed("Googlebot", "/admin/public/index.html"));
/// assert!(!policy.is_allowed("Googlebot", "/admin/dashboard"));
/// assert!(policy.is_allowed("MyBot", "/admin/dashboard"));
/// assert!(!policy.is_allowed("MyBot", "/private/data"));
/// assert_eq!(policy.sitemaps(), &["https://example.com/sitemap.xml"]);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RobotsPolicy<'a> {
    groups: Vec<RobotsGroup<'a>>,
    sitemaps: Vec<Cow<'a, str>>,
}

impl<'a> RobotsPolicy<'a> {
    /// Creates an empty policy that allows all URLs.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Parses a `robots.txt` document into a zero-copy [`RobotsPolicy`].
    ///
    /// Borrowed slices point directly to `content` without allocating new strings.
    #[must_use]
    pub fn parse(content: &'a str) -> Self {
        let mut groups = Vec::new();
        let mut sitemaps = Vec::new();

        let mut current_user_agents: Vec<Cow<'a, str>> = Vec::new();
        let mut current_rules: Vec<RobotsRule<'a>> = Vec::new();
        let mut current_delay: Option<Duration> = None;

        for raw_line in content.lines() {
            let line = raw_line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }

            let Some((raw_key, raw_val)) = line.split_once(':') else {
                continue;
            };

            let key = raw_key.trim();
            let val = raw_val.trim();

            if key.eq_ignore_ascii_case("user-agent") {
                if !current_rules.is_empty() || current_delay.is_some() {
                    groups.push(RobotsGroup {
                        user_agents: std::mem::take(&mut current_user_agents),
                        rules: std::mem::take(&mut current_rules),
                        crawl_delay: current_delay.take(),
                    });
                }
                if !val.is_empty() {
                    current_user_agents.push(Cow::Borrowed(val));
                }
            } else if key.eq_ignore_ascii_case("allow") {
                if !current_user_agents.is_empty() && !val.is_empty() {
                    current_rules.push(RobotsRule {
                        rule_type: RuleType::Allow,
                        path: Cow::Borrowed(val),
                    });
                }
            } else if key.eq_ignore_ascii_case("disallow") {
                if !current_user_agents.is_empty() && !val.is_empty() {
                    current_rules.push(RobotsRule {
                        rule_type: RuleType::Disallow,
                        path: Cow::Borrowed(val),
                    });
                }
            } else if key.eq_ignore_ascii_case("crawl-delay") {
                if !current_user_agents.is_empty()
                    && let Ok(secs) = val.parse::<f64>()
                    && secs >= 0.0
                {
                    current_delay = Some(Duration::from_secs_f64(secs));
                }
            } else if key.eq_ignore_ascii_case("sitemap") && !val.is_empty() {
                sitemaps.push(Cow::Borrowed(val));
            }
        }

        if !current_user_agents.is_empty() {
            groups.push(RobotsGroup {
                user_agents: current_user_agents,
                rules: current_rules,
                crawl_delay: current_delay,
            });
        }

        Self { groups, sitemaps }
    }

    /// Evaluates whether a given `user_agent` is allowed to crawl `path_or_url`.
    ///
    /// Implements RFC 9309 rules: longest match wins, and `Allow` breaks ties with `Disallow`.
    #[must_use]
    pub fn is_allowed(&self, user_agent: &str, path_or_url: &str) -> bool {
        let path = extract_path_and_query(path_or_url);
        let Some(group) = self.find_group(user_agent) else {
            return true;
        };

        let mut best_match: Option<(&RobotsRule<'a>, usize)> = None;

        for rule in &group.rules {
            if rule.path.is_empty() {
                continue;
            }
            if matches_pattern(&rule.path, path) {
                let rule_len = rule.path.len();
                match best_match {
                    None => {
                        best_match = Some((rule, rule_len));
                    }
                    Some((prev_rule, prev_len)) => {
                        if rule_len > prev_len
                            || (rule_len == prev_len
                                && rule.rule_type == RuleType::Allow
                                && prev_rule.rule_type == RuleType::Disallow)
                        {
                            best_match = Some((rule, rule_len));
                        }
                    }
                }
            }
        }

        match best_match {
            Some((rule, _)) => rule.rule_type == RuleType::Allow,
            None => true,
        }
    }

    /// Returns the crawl delay specified for the given user agent, if configured.
    #[must_use]
    pub fn crawl_delay(&self, user_agent: &str) -> Option<Duration> {
        self.find_group(user_agent).and_then(|g| g.crawl_delay)
    }

    /// Discovered sitemap URLs referenced in the policy.
    #[must_use]
    pub fn sitemaps(&self) -> Vec<&str> {
        self.sitemaps.iter().map(|s| s.as_ref()).collect()
    }

    /// Converts this policy into an owned variant with `'static` lifetime.
    #[must_use]
    pub fn into_owned(self) -> RobotsPolicy<'static> {
        RobotsPolicy {
            groups: self
                .groups
                .into_iter()
                .map(RobotsGroup::into_owned)
                .collect(),
            sitemaps: self
                .sitemaps
                .into_iter()
                .map(|s| Cow::Owned(s.into_owned()))
                .collect(),
        }
    }

    fn find_group(&self, user_agent: &str) -> Option<&RobotsGroup<'a>> {
        let target_ua = user_agent.to_ascii_lowercase();

        let mut best_group: Option<(&RobotsGroup<'a>, usize)> = None;

        for group in &self.groups {
            for ua in &group.user_agents {
                let ua_lower = ua.to_ascii_lowercase();
                if ua_lower == "*" {
                    continue;
                }
                if target_ua.contains(&ua_lower) {
                    let match_len = ua_lower.len();
                    if best_group.as_ref().is_none_or(|(_, len)| match_len > *len) {
                        best_group = Some((group, match_len));
                    }
                }
            }
        }

        if let Some((group, _)) = best_group {
            return Some(group);
        }

        for group in &self.groups {
            for ua in &group.user_agents {
                if ua.trim() == "*" {
                    return Some(group);
                }
            }
        }

        None
    }
}

impl FromStr for RobotsPolicy<'static> {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(RobotsPolicy::parse(s).into_owned())
    }
}

impl fmt::Display for RobotsPolicy<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for group in &self.groups {
            for ua in &group.user_agents {
                writeln!(f, "User-agent: {ua}")?;
            }
            if let Some(delay) = group.crawl_delay {
                writeln!(f, "Crawl-delay: {}", delay.as_secs_f64())?;
            }
            for rule in &group.rules {
                writeln!(f, "{}: {}", rule.rule_type, rule.path)?;
            }
            writeln!(f)?;
        }
        for sitemap in &self.sitemaps {
            writeln!(f, "Sitemap: {sitemap}")?;
        }
        Ok(())
    }
}

/// Asynchronously fetches a `robots.txt` from a given base or root URL.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::InvalidConfig`] if the network request fails.
pub async fn fetch_robots_txt(
    client: &reqwest::Client,
    base_url: &str,
) -> Result<String, SeleniumBaseError> {
    let robots_url = if let Some(stripped) = base_url.strip_suffix('/') {
        format!("{stripped}/robots.txt")
    } else if base_url.ends_with("/robots.txt") {
        base_url.to_string()
    } else {
        format!("{base_url}/robots.txt")
    };

    let response = client.get(&robots_url).send().await.map_err(|e| {
        SeleniumBaseError::InvalidConfig(format!("failed to fetch robots.txt: {e}"))
    })?;

    let text = response.text().await.map_err(|e| {
        SeleniumBaseError::InvalidConfig(format!("failed to read robots.txt body: {e}"))
    })?;

    Ok(text)
}

fn extract_path_and_query(path_or_url: &str) -> &str {
    if let Some(rest) = path_or_url.strip_prefix("http://") {
        if let Some(idx) = rest.find('/') {
            &rest[idx..]
        } else {
            "/"
        }
    } else if let Some(rest) = path_or_url.strip_prefix("https://") {
        if let Some(idx) = rest.find('/') {
            &rest[idx..]
        } else {
            "/"
        }
    } else {
        path_or_url
    }
}

fn matches_pattern(pattern: &str, target: &str) -> bool {
    if pattern.is_empty() {
        return false;
    }
    let (pattern_no_end, end_anchor) = if let Some(stripped) = pattern.strip_suffix('$') {
        (stripped, true)
    } else {
        (pattern, false)
    };

    let segments: Vec<&str> = pattern_no_end.split('*').collect();
    if segments.is_empty() {
        return true;
    }

    let mut remainder = target;
    if !segments[0].is_empty() {
        if let Some(rest) = remainder.strip_prefix(segments[0]) {
            remainder = rest;
        } else {
            return false;
        }
    }

    let middle_count = if segments.len() > 1 {
        segments.len().saturating_sub(2)
    } else {
        0
    };

    for &segment in &segments[1..=middle_count] {
        if segment.is_empty() {
            continue;
        }
        if let Some(pos) = remainder.find(segment) {
            remainder = &remainder[pos + segment.len()..];
        } else {
            return false;
        }
    }

    if segments.len() > 1 {
        let last_segment = segments[segments.len() - 1];
        if end_anchor {
            remainder.ends_with(last_segment)
        } else if last_segment.is_empty() {
            true
        } else {
            remainder.contains(last_segment)
        }
    } else if end_anchor {
        remainder.is_empty()
    } else {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_evaluates_basic_robots() {
        let txt = r#"
            # robots.txt for test
            User-agent: Googlebot
            Disallow: /admin
            Allow: /admin/public

            User-agent: *
            Disallow: /secret/
            Crawl-delay: 1.5
            Sitemap: https://example.com/sitemap.xml
        "#;

        let policy = RobotsPolicy::parse(txt);
        assert_eq!(policy.sitemaps(), vec!["https://example.com/sitemap.xml"]);

        // Googlebot
        assert!(!policy.is_allowed("Googlebot", "/admin/dashboard"));
        assert!(policy.is_allowed("Googlebot", "/admin/public/profile"));
        assert!(policy.is_allowed("Googlebot", "/secret/data")); // Not blocked for Googlebot
        assert_eq!(policy.crawl_delay("Googlebot"), None);

        // Fallback agent
        assert!(policy.is_allowed("Bingbot", "/admin/dashboard"));
        assert!(!policy.is_allowed("Bingbot", "/secret/data"));
        assert_eq!(
            policy.crawl_delay("Bingbot"),
            Some(Duration::from_millis(1500))
        );
    }

    #[test]
    fn wildcard_and_end_anchor_matching() {
        let txt = r#"
            User-agent: *
            Disallow: /*.gif$
            Disallow: /fish*salmon/
            Allow: /fish/salmon/special
        "#;
        let policy = RobotsPolicy::parse(txt);

        assert!(!policy.is_allowed("bot", "/images/pic.gif"));
        assert!(policy.is_allowed("bot", "/images/pic.gif?size=large"));
        assert!(!policy.is_allowed("bot", "/fish/ocean/salmon/catch"));
        assert!(policy.is_allowed("bot", "/fish/salmon/special"));
    }

    #[test]
    fn empty_disallow_allows_all() {
        let txt = r#"
            User-agent: *
            Disallow:
        "#;
        let policy = RobotsPolicy::parse(txt);
        assert!(policy.is_allowed("bot", "/anything"));
    }

    #[test]
    fn full_url_path_extraction() {
        let txt = r#"
            User-agent: *
            Disallow: /private
        "#;
        let policy = RobotsPolicy::parse(txt);
        assert!(!policy.is_allowed("bot", "https://example.com/private/vault"));
        assert!(policy.is_allowed("bot", "https://example.com/public"));
    }

    #[test]
    fn into_owned_and_display() {
        let txt = "User-agent: *\nDisallow: /test\n";
        let policy = RobotsPolicy::parse(txt).into_owned();
        assert!(!policy.is_allowed("bot", "/test"));
        let rendered = policy.to_string();
        assert!(rendered.contains("User-agent: *"));
        assert!(rendered.contains("Disallow: /test"));
    }

    #[test]
    fn user_agent_specific_precedence_and_grouping() {
        let txt = r#"
            User-agent: SpecificBot
            Disallow: /blocked_for_specific
            Allow: /secret

            User-agent: AnotherBot
            User-agent: ThirdBot
            Disallow: /multi_agent_block

            User-agent: *
            Disallow: /secret
            Crawl-delay: 3.0
        "#;
        let policy = RobotsPolicy::parse(txt);

        // SpecificBot matches its own group, ignores wildcard Disallow: /secret
        assert!(policy.is_allowed("SpecificBot", "/secret"));
        assert!(!policy.is_allowed("SpecificBot", "/blocked_for_specific"));
        // SpecificBot has no Crawl-delay in its group, does not merge from wildcard
        assert_eq!(policy.crawl_delay("SpecificBot"), None);
        // Fallback agent inherits from wildcard group
        assert_eq!(
            policy.crawl_delay("FallbackBot"),
            Some(Duration::from_secs(3))
        );

        // Multi-agent group
        assert!(!policy.is_allowed("AnotherBot", "/multi_agent_block"));
        assert!(!policy.is_allowed("ThirdBot", "/multi_agent_block"));
        assert!(policy.is_allowed("OtherBot", "/multi_agent_block"));
    }

    #[test]
    fn longest_match_precedence_rfc9309() {
        let txt = r#"
            User-agent: *
            Disallow: /a/b/c
            Allow: /a/b
            Disallow: /a/b/c/d/e
            Allow: /a/b/c/d
        "#;
        let policy = RobotsPolicy::parse(txt);

        // /a/b/c -> matches Disallow: /a/b/c (len 6) vs Allow: /a/b (len 4) -> Disallow wins
        assert!(!policy.is_allowed("bot", "/a/b/c"));

        // /a/b/c/d -> matches Allow: /a/b/c/d (len 8) vs Disallow: /a/b/c (len 6) -> Allow wins
        assert!(policy.is_allowed("bot", "/a/b/c/d"));

        // /a/b/c/d/e -> matches Disallow: /a/b/c/d/e (len 10) -> Disallow wins
        assert!(!policy.is_allowed("bot", "/a/b/c/d/e"));

        // /a/b/other -> matches Allow: /a/b (len 4) -> Allow wins
        assert!(policy.is_allowed("bot", "/a/b/other"));
    }

    #[test]
    fn comments_and_whitespace_handling() {
        let txt = r#"
            # Header comment
            User-agent: * # inline comment
            Disallow: /private/ # disallow private
            Allow: /private/public/ # allow public under private
            Sitemap: https://example.com/s1.xml
            Sitemap: https://example.com/s2.xml
        "#;
        let policy = RobotsPolicy::parse(txt);
        assert_eq!(
            policy.sitemaps(),
            vec!["https://example.com/s1.xml", "https://example.com/s2.xml"]
        );
        assert!(!policy.is_allowed("bot", "/private/secret.txt"));
        assert!(policy.is_allowed("bot", "/private/public/index.html"));
    }
}
