use crate::api::base_case::BaseCase;
use crate::error::SeleniumBaseError;
use std::fs;
use std::path::Path;

#[derive(Clone, Debug, Default)]
pub struct TourStep {
    pub message: String,
    pub target: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum TourTheme {
    #[default]
    SeleniumBase,
    Shepherd,
    IntroJs,
    DriverJs,
    Bootstrap,
    Hopscotch,
}

impl TourTheme {
    pub fn as_str(&self) -> &'static str {
        match self {
            TourTheme::SeleniumBase => "seleniumbase",
            TourTheme::Shepherd => "shepherd",
            TourTheme::IntroJs => "introjs",
            TourTheme::DriverJs => "driverjs",
            TourTheme::Bootstrap => "bootstrap",
            TourTheme::Hopscotch => "hopscotch",
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Tour {
    pub name: String,
    pub steps: Vec<TourStep>,
    pub theme: TourTheme,
}

impl Tour {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            steps: Vec::new(),
            theme: TourTheme::SeleniumBase,
        }
    }

    pub fn with_theme(mut self, theme: TourTheme) -> Self {
        self.theme = theme;
        self
    }

    pub fn add_step(&mut self, message: &str, target: Option<&str>) {
        self.steps.push(TourStep {
            message: message.to_owned(),
            target: target.map(ToOwned::to_owned),
        });
    }

    /// Injects a self-contained JavaScript tour into the current page.
    pub async fn play(&self, sb: &BaseCase) -> Result<(), SeleniumBaseError> {
        match self.theme {
            TourTheme::Shepherd => self.play_shepherd(sb).await,
            TourTheme::IntroJs => self.play_introjs(sb).await,
            TourTheme::DriverJs => self.play_driverjs(sb).await,
            TourTheme::Bootstrap | TourTheme::Hopscotch | TourTheme::SeleniumBase => {
                self.play_default(sb).await
            }
        }
    }

    async fn play_default(&self, sb: &BaseCase) -> Result<(), SeleniumBaseError> {
        let steps_json = serde_json::to_string(
            &self
                .steps
                .iter()
                .map(|s| {
                    serde_json::json!({
                        "message": s.message,
                        "target": s.target
                    })
                })
                .collect::<Vec<_>>(),
        )
        .map_err(|e| SeleniumBaseError::InvalidConfig(format!("Failed to serialize tour: {e}")))?;

        let script = format!(
            r#"
            (function() {{
                const steps = {steps_json};
                let current = 0;
                function removeOverlay() {{
                    const old = document.getElementById('sb-rs-tour-overlay');
                    if (old) old.remove();
                    const oldBox = document.getElementById('sb-rs-tour-box');
                    if (oldBox) oldBox.remove();
                }}
                function showStep() {{
                    removeOverlay();
                    if (current >= steps.length) return;
                    const step = steps[current];
                    let target = null;
                    if (step.target) {{
                        try {{ target = document.querySelector(step.target); }} catch (e) {{}}
                    }}
                    if (target) {{
                        const rect = target.getBoundingClientRect();
                        const overlay = document.createElement('div');
                        overlay.id = 'sb-rs-tour-overlay';
                        overlay.style.position = 'fixed';
                        overlay.style.left = rect.left + 'px';
                        overlay.style.top = rect.top + 'px';
                        overlay.style.width = rect.width + 'px';
                        overlay.style.height = rect.height + 'px';
                        overlay.style.boxShadow = '0 0 0 9999px rgba(0,0,0,0.5)';
                        overlay.style.zIndex = '2147483646';
                        overlay.style.borderRadius = '4px';
                        document.body.appendChild(overlay);
                    }}
                    const box = document.createElement('div');
                    box.id = 'sb-rs-tour-box';
                    box.style.position = 'fixed';
                    box.style.bottom = '24px';
                    box.style.left = '50%';
                    box.style.transform = 'translateX(-50%)';
                    box.style.background = 'rgba(20,20,20,0.95)';
                    box.style.color = '#fff';
                    box.style.padding = '16px 20px';
                    box.style.borderRadius = '8px';
                    box.style.maxWidth = '480px';
                    box.style.zIndex = '2147483647';
                    box.style.fontFamily = 'Arial,sans-serif';
                    box.innerHTML = '<p style="margin:0 0 10px 0;">' + (current + 1) + '/' + steps.length + ': ' + step.message + '</p>' +
                        '<button id="sb-rs-tour-next" style="margin-right:8px;padding:6px 12px;">Next</button>' +
                        '<button id="sb-rs-tour-end" style="padding:6px 12px;">End</button>';
                    document.body.appendChild(box);
                    document.getElementById('sb-rs-tour-next').onclick = function() {{ current++; showStep(); }};
                    document.getElementById('sb-rs-tour-end').onclick = function() {{ removeOverlay(); box.remove(); }};
                }}
                showStep();
            }})();
            "#
        );
        sb.execute_script(&script).await?;
        Ok(())
    }

    async fn play_shepherd(&self, sb: &BaseCase) -> Result<(), SeleniumBaseError> {
        let steps_json = serde_json::to_string(
            &self
                .steps
                .iter()
                .map(|s| {
                    serde_json::json!({
                        "title": self.name,
                        "text": s.message,
                        "attachTo": s.target.as_ref().map(|t| serde_json::json!({"element": t, "on": "bottom"}))
                    })
                })
                .collect::<Vec<_>>(),
        )
        .map_err(|e| SeleniumBaseError::InvalidConfig(format!("Failed to serialize tour: {e}")))?;

        let script = format!(
            r#"
            (function() {{
                function loadScript(src) {{
                    return new Promise(function(resolve, reject) {{
                        const s = document.createElement('script');
                        s.src = src;
                        s.onload = resolve;
                        s.onerror = reject;
                        document.head.appendChild(s);
                    }});
                }}
                function loadCss(href) {{
                    const l = document.createElement('link');
                    l.rel = 'stylesheet';
                    l.href = href;
                    document.head.appendChild(l);
                }}
                loadCss('https://cdn.jsdelivr.net/npm/shepherd.js@11.0.1/dist/css/shepherd.css');
                loadScript('https://cdn.jsdelivr.net/npm/shepherd.js@11.0.1/dist/js/shepherd.min.js')
                    .then(function() {{
                        const tour = new Shepherd.Tour({{
                            useModalOverlay: true,
                            defaultStepOptions: {{
                                cancelIcon: {{ enabled: true }},
                                classes: 'shadow-md bg-purple-dark',
                                scrollTo: {{ behavior: 'smooth', block: 'center' }}
                            }}
                        }});
                        const steps = {steps_json};
                        steps.forEach(function(step) {{
                            if (!step.attachTo) delete step.attachTo;
                            tour.addStep(step);
                        }});
                        tour.start();
                    }});
            }})();
            "#
        );
        sb.execute_script(&script).await?;
        Ok(())
    }

    async fn play_introjs(&self, sb: &BaseCase) -> Result<(), SeleniumBaseError> {
        let steps_json = serde_json::to_string(
            &self
                .steps
                .iter()
                .map(|s| {
                    serde_json::json!({
                        "intro": s.message,
                        "element": s.target.as_deref().unwrap_or("body")
                    })
                })
                .collect::<Vec<_>>(),
        )
        .map_err(|e| SeleniumBaseError::InvalidConfig(format!("Failed to serialize tour: {e}")))?;

        let script = format!(
            r#"
            (function() {{
                function loadScript(src) {{
                    return new Promise(function(resolve, reject) {{
                        const s = document.createElement('script');
                        s.src = src;
                        s.onload = resolve;
                        s.onerror = reject;
                        document.head.appendChild(s);
                    }});
                }}
                function loadCss(href) {{
                    const l = document.createElement('link');
                    l.rel = 'stylesheet';
                    l.href = href;
                    document.head.appendChild(l);
                }}
                loadCss('https://unpkg.com/intro.js@7.0.1/minified/introjs.min.css');
                loadScript('https://unpkg.com/intro.js@7.0.1/minified/intro.min.js')
                    .then(function() {{
                        introJs().setOptions({{ steps: {steps_json} }}).start();
                    }});
            }})();
            "#
        );
        sb.execute_script(&script).await?;
        Ok(())
    }

    async fn play_driverjs(&self, sb: &BaseCase) -> Result<(), SeleniumBaseError> {
        let steps_json = serde_json::to_string(
            &self
                .steps
                .iter()
                .map(|s| {
                    serde_json::json!({
                        "element": s.target.as_deref().unwrap_or("body"),
                        "popover": {
                            "title": self.name,
                            "description": s.message,
                            "position": "bottom"
                        }
                    })
                })
                .collect::<Vec<_>>(),
        )
        .map_err(|e| SeleniumBaseError::InvalidConfig(format!("Failed to serialize tour: {e}")))?;

        let script = format!(
            r#"
            (function() {{
                function loadScript(src) {{
                    return new Promise(function(resolve, reject) {{
                        const s = document.createElement('script');
                        s.src = src;
                        s.onload = resolve;
                        s.onerror = reject;
                        document.head.appendChild(s);
                    }});
                }}
                function loadCss(href) {{
                    const l = document.createElement('link');
                    l.rel = 'stylesheet';
                    l.href = href;
                    document.head.appendChild(l);
                }}
                loadCss('https://unpkg.com/driver.js@1.3.1/dist/driver.css');
                loadScript('https://unpkg.com/driver.js@1.3.1/dist/driver.js.iife.js')
                    .then(function() {{
                        const driver = window.driver.js.driver({{ showProgress: true }});
                        const steps = {steps_json};
                        driver.drive({{ steps: steps }});
                    }});
            }})();
            "#
        );
        sb.execute_script(&script).await?;
        Ok(())
    }

    /// Exports the tour as a standalone HTML file that can be opened later.
    pub fn export_html<P: AsRef<Path>>(&self, path: P) -> Result<(), SeleniumBaseError> {
        let mut html = String::from(
            "<!DOCTYPE html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n<title>Tour: ",
        );
        html.push_str(&self.name);
        html.push_str(
            "</title>\n<style>\nbody {{ font-family: Arial,sans-serif; margin: 40px; }}\n.step {{ margin: 12px 0; padding: 12px; border: 1px solid #ccc; border-radius: 6px; }}\n.target {{ color: #555; font-size: 0.9em; }}\n</style>\n</head>\n<body>\n<h1>",
        );
        html.push_str(&self.name);
        html.push_str("</h1>\n<ol>\n");
        for step in &self.steps {
            html.push_str("<li class=\"step\">\n");
            html.push_str(&format!("<p>{}</p>\n", html_escape(&step.message)));
            if let Some(target) = &step.target {
                html.push_str(&format!(
                    "<p class=\"target\">Target: <code>{}</code></p>\n",
                    html_escape(target)
                ));
            }
            html.push_str("</li>\n");
        }
        html.push_str("</ol>\n</body>\n</html>");

        fs::write(path.as_ref(), html).map_err(|e| {
            SeleniumBaseError::InvalidConfig(format!(
                "failed to write tour html '{}': {e}",
                path.as_ref().display()
            ))
        })?;
        Ok(())
    }
}

fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn export_html_contains_steps() {
        let mut tour = Tour::new("Onboarding");
        tour.add_step("Click the logo", Some("#logo"));
        tour.add_step("Fill the form", None);

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tour.html");
        tour.export_html(&path).unwrap();

        let html = fs::read_to_string(&path).unwrap();
        assert!(html.contains("Onboarding"));
        assert!(html.contains("Click the logo"));
        assert!(html.contains("#logo"));
        assert!(html.contains("Fill the form"));
    }
}
