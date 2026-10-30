// Tests of `switch_to_frame`, `frame_switch`, `switch_to_window` and
// `switch_to_alert` against a small fake browser behind the mock WebDriver in
// `webdriver_support`: frames (with an id, a name, a display state and frames
// inside), windows, and an alert. The tests that a real Chrome does the same are
// in `tests/frames_windows_alerts_chrome.rs`.
// Included (for tests only) at the end of `base_case.rs` like the files beside it.

mod tests_switching {
    use super::webdriver_support::{element, MockWebDriver, Reply, Seen};
    use super::*;
    use serde_json::{json, Value};
    use std::sync::{Arc, Mutex};

    /// One `<iframe>` of the fake page.
    struct FakeFrame {
        id: &'static str,
        name: &'static str,
        displayed: bool,
        inner: Vec<FakeFrame>,
    }

    impl FakeFrame {
        fn new(id: &'static str, name: &'static str) -> Self {
            Self {
                id,
                name,
                displayed: true,
                inner: Vec::new(),
            }
        }

        fn hidden(mut self) -> Self {
            self.displayed = false;
            self
        }

        fn holding(mut self, inner: Vec<FakeFrame>) -> Self {
            self.inner = inner;
            self
        }
    }

    /// A browser whose page holds `frames`, with the windows `w1`..., and an
    /// alert that may be open.
    struct Fake {
        frames: Vec<FakeFrame>,
        /// The frames the driver has entered, outermost first, as indexes into
        /// the frames of the document it was in at that moment.
        path: Vec<usize>,
        windows: Vec<String>,
        current: String,
        alert: Option<String>,
        /// What was done to the alert: `accept`, `dismiss` or `keys: ...`.
        alert_log: Vec<String>,
    }

    impl Fake {
        fn new(frames: Vec<FakeFrame>) -> Self {
            Self {
                frames,
                path: Vec::new(),
                windows: vec!["w1".to_owned()],
                current: "w1".to_owned(),
                alert: None,
                alert_log: Vec::new(),
            }
        }

        /// The frames of the document the driver is in.
        fn here(&self) -> &[FakeFrame] {
            let mut frames = self.frames.as_slice();
            for &index in &self.path {
                frames = frames[index].inner.as_slice();
            }
            frames
        }

        /// The frames that `selector` selects in this document: the lookups by
        /// `id` and by `name` the driver is expected to send, `iframe#id`, and
        /// the bare tag `iframe`.
        fn matching(&self, selector: &str) -> Vec<usize> {
            let attribute = |name: &str| -> Option<String> {
                let start = format!("iframe[{name}=\"");
                let rest = selector.strip_prefix(start.as_str())?;
                Some(rest.split('"').next()?.to_owned())
            };
            let by_id = attribute("id");
            let by_name = attribute("name");
            self.here()
                .iter()
                .enumerate()
                .filter(|(_, frame)| {
                    if let Some(id) = &by_id {
                        frame.id == id
                    } else if let Some(name) = &by_name {
                        frame.name == name
                    } else if let Some(id) = selector.strip_prefix("iframe#") {
                        frame.id == id
                    } else {
                        selector == "iframe"
                    }
                })
                .map(|(index, _)| index)
                .collect()
        }

        fn answer(&mut self, request: &Seen) -> Reply {
            let text = |value: &Value| value.as_str().unwrap_or_default().to_owned();
            let method_and_path = (request.method.as_str(), request.path.as_str());
            match method_and_path {
                ("POST", path @ ("/element" | "/elements")) => {
                    let hits = self.matching(&text(&request.body["value"]));
                    match (path, hits.first()) {
                        ("/element", Some(index)) => Reply::Value(element(&format!("f{index}"))),
                        ("/element", None) => Reply::Error(404, "no such element"),
                        _ => Reply::Value(json!(hits
                            .iter()
                            .map(|index| element(&format!("f{index}")))
                            .collect::<Vec<_>>())),
                    }
                }
                ("GET", path) if path.starts_with("/element/f") && path.ends_with("/displayed") => {
                    let index = path["/element/f".len()..path.len() - "/displayed".len()]
                        .parse::<usize>()
                        .unwrap_or(usize::MAX);
                    match self.here().get(index) {
                        Some(frame) => Reply::Value(json!(frame.displayed)),
                        None => Reply::Error(404, "stale element reference"),
                    }
                }
                ("POST", "/frame") => match &request.body["id"] {
                    Value::Null => {
                        self.path.clear();
                        Reply::Value(Value::Null)
                    }
                    Value::Number(number) => {
                        let index = number.as_u64().and_then(|n| usize::try_from(n).ok());
                        self.enter(index)
                    }
                    Value::Object(reference) => {
                        let index = reference
                            .values()
                            .filter_map(Value::as_str)
                            .find_map(|id| id.strip_prefix('f')?.parse::<usize>().ok());
                        self.enter(index)
                    }
                    _ => Reply::Error(400, "invalid argument"),
                },
                ("POST", "/frame/parent") => {
                    self.path.pop();
                    Reply::Value(Value::Null)
                }
                ("GET", "/window/handles") => Reply::Value(json!(self.windows)),
                ("GET", "/window") => Reply::Value(json!(self.current)),
                ("POST", "/window") => {
                    let handle = text(&request.body["handle"]);
                    if self.windows.contains(&handle) {
                        self.current = handle;
                        Reply::Value(Value::Null)
                    } else {
                        Reply::Error(404, "no such window")
                    }
                }
                ("GET", "/alert/text") => match &self.alert {
                    Some(alert) => Reply::Value(json!(alert)),
                    None => Reply::Error(404, "no such alert"),
                },
                ("POST", "/alert/accept") => self.close_alert("accept"),
                ("POST", "/alert/dismiss") => self.close_alert("dismiss"),
                ("POST", "/alert/text") => {
                    let keys = format!("keys: {}", text(&request.body["text"]));
                    self.alert_log.push(keys);
                    Reply::Value(Value::Null)
                }
                _ => Reply::Value(Value::Null),
            }
        }

        /// Enters the frame at `index` of this document.
        fn enter(&mut self, index: Option<usize>) -> Reply {
            match index {
                Some(index) if index < self.here().len() => {
                    self.path.push(index);
                    Reply::Value(Value::Null)
                }
                _ => Reply::Error(404, "no such frame"),
            }
        }

        fn close_alert(&mut self, what: &str) -> Reply {
            if self.alert.take().is_none() {
                return Reply::Error(404, "no such alert");
            }
            self.alert_log.push(what.to_owned());
            Reply::Value(Value::Null)
        }
    }

    /// A driver backed by a [`Fake`] whose page holds `frames`, and the fake
    /// itself, so a test can look at the browser and change it.
    fn start(frames: Vec<FakeFrame>) -> (MockWebDriver, Arc<Mutex<Fake>>) {
        let fake = Arc::new(Mutex::new(Fake::new(frames)));
        let shared = Arc::clone(&fake);
        let driver = MockWebDriver::start(json!({}), move |request| {
            shared.lock().expect("the fake browser").answer(request)
        });
        (driver, fake)
    }

    /// The frames the driver is in, as the path of indexes.
    fn path(fake: &Mutex<Fake>) -> Vec<usize> {
        fake.lock().expect("the fake browser").path.clone()
    }

    #[tokio::test]
    async fn a_frame_is_entered_by_its_id() {
        let (driver, fake) = start(vec![
            FakeFrame::new("first", ""),
            FakeFrame::new("payment", ""),
        ]);
        let mut sb = driver.base_case().await;

        sb.switch_to_frame("payment")
            .await
            .expect("a frame with that id is there");

        assert_eq!(path(&fake), [1]);
    }

    #[tokio::test]
    async fn a_frame_is_entered_by_its_name() {
        let (driver, fake) = start(vec![
            FakeFrame::new("", "first"),
            FakeFrame::new("", "iframeResult"),
        ]);
        let mut sb = driver.base_case().await;

        sb.switch_to_frame("iframeResult")
            .await
            .expect("a frame with that name is there");

        assert_eq!(path(&fake), [1]);
    }

    #[tokio::test]
    async fn a_frame_is_entered_by_a_selector() {
        let (driver, fake) = start(vec![FakeFrame::new("a", ""), FakeFrame::new("pay", "")]);
        let mut sb = driver.base_case().await;

        sb.switch_to_frame("iframe#pay")
            .await
            .expect("the selector selects a frame");

        assert_eq!(path(&fake), [1]);
    }

    #[tokio::test]
    async fn a_frame_is_entered_by_its_position() {
        let (driver, fake) = start(vec![FakeFrame::new("a", ""), FakeFrame::new("b", "")]);
        let mut sb = driver.base_case().await;

        sb.switch_to_frame(1)
            .await
            .expect("the second frame is there");

        assert_eq!(path(&fake), [1]);
    }

    #[tokio::test]
    async fn frames_inside_frames_are_entered_one_at_a_time() {
        let (driver, fake) = start(vec![
            FakeFrame::new("outer", "").holding(vec![FakeFrame::new("inner", "")])
        ]);
        let mut sb = driver.base_case().await;

        sb.switch_to_frame("outer").await.expect("the outer frame");
        sb.switch_to_frame("inner").await.expect("the inner frame");
        assert_eq!(path(&fake), [0, 0]);

        sb.switch_to_parent_frame().await.unwrap();
        assert_eq!(path(&fake), [0]);
        sb.switch_to_default_content().await.unwrap();
        assert_eq!(path(&fake), Vec::<usize>::new());
    }

    #[tokio::test]
    async fn a_frame_that_shows_up_late_is_waited_for() {
        let (driver, fake) = start(Vec::new());
        let mut sb = driver.base_case().await;
        let browser = Arc::clone(&fake);
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(350)).await;
            let mut browser = browser.lock().expect("the fake browser");
            browser.frames.push(FakeFrame::new("late", ""));
        });

        sb.switch_to_frame("late")
            .await
            .expect("the call waits for the frame");

        assert_eq!(path(&fake), [0]);
    }

    #[tokio::test]
    async fn a_frame_that_never_shows_up_times_out_and_names_the_frame() {
        let (driver, fake) = start(Vec::new());
        let mut sb = driver.base_case().await;

        let error = sb
            .switch_to_frame("nothing")
            .timeout(Duration::from_secs(1))
            .await
            .expect_err("there is no such frame");

        assert!(
            matches!(&error, SeleniumBaseError::WaitTimeout(message)
                if message.contains("Frame {nothing} was not visible after 1s")),
            "{error:?}"
        );
        assert_eq!(path(&fake), Vec::<usize>::new());
    }

    #[tokio::test]
    async fn a_hidden_frame_needs_invisible_when_it_is_selected_by_a_selector() {
        let (driver, fake) = start(vec![FakeFrame::new("shy", "").hidden()]);
        let mut sb = driver.base_case().await;

        let error = sb
            .switch_to_frame("iframe#shy")
            .timeout(Duration::from_secs(1))
            .await
            .expect_err("a selector waits for a displayed frame");
        assert!(
            matches!(&error, SeleniumBaseError::WaitTimeout(message)
                if message.contains("was not visible")),
            "{error:?}"
        );

        sb.switch_to_frame("iframe#shy")
            .invisible(true)
            .await
            .expect("invisible lets a hidden frame in");
        assert_eq!(path(&fake), [0]);

        // A frame found by its id or name needs no display, as in Python.
        sb.switch_to_default_content().await.unwrap();
        sb.switch_to_frame("shy").await.expect("an id needs no display");
        assert_eq!(path(&fake), [0]);
    }

    #[tokio::test]
    async fn a_frame_entered_by_text_is_recorded_and_by_position_is_not() {
        let (driver, _fake) = start(vec![FakeFrame::new("pay", "")]);
        let mut sb = driver.base_case().await;

        sb.switch_to_frame(0).await.unwrap();
        sb.switch_to_default_content().await.unwrap();
        sb.switch_to_frame("pay").await.unwrap();

        let recorded: Vec<(String, Option<String>)> = sb
            .recorder
            .lock()
            .unwrap()
            .actions
            .iter()
            .map(|action| (action.name.clone(), action.target.clone()))
            .collect();
        assert_eq!(
            recorded,
            [
                ("switch_to_default_content".to_owned(), None),
                ("switch_to_frame".to_owned(), Some("pay".to_owned())),
            ]
        );
    }

    #[tokio::test]
    async fn frame_switch_runs_the_steps_inside_and_leaves_the_frame_after() {
        let (driver, fake) = start(vec![FakeFrame::new("pay", "")]);
        let mut sb = driver.base_case().await;

        let inside = sb
            .frame_switch("pay")
            .run(async |_sb| Ok(path(&fake)))
            .await
            .expect("the frame is there");

        assert_eq!(inside, [0], "the steps run inside the frame");
        assert_eq!(path(&fake), Vec::<usize>::new(), "the frame is left after");
    }

    #[tokio::test]
    async fn frame_switch_leaves_the_frame_when_the_steps_fail() {
        let (driver, fake) = start(vec![FakeFrame::new("pay", "")]);
        let mut sb = driver.base_case().await;

        let error = sb
            .frame_switch("pay")
            .run(async |_sb| Err::<(), _>(SeleniumBaseError::AssertionFailed("boom".to_owned())))
            .await
            .expect_err("the steps fail");

        assert!(
            matches!(&error, SeleniumBaseError::AssertionFailed(message) if message == "boom"),
            "the steps' own error comes back: {error:?}"
        );
        assert_eq!(path(&fake), Vec::<usize>::new(), "the frame is left anyway");
    }

    #[tokio::test]
    async fn frame_switch_blocks_nest_and_each_leaves_one_frame() {
        let (driver, fake) = start(vec![
            FakeFrame::new("outer", "").holding(vec![FakeFrame::new("inner", "")])
        ]);
        let mut sb = driver.base_case().await;

        sb.frame_switch("outer")
            .run(async |sb| {
                let deepest = sb
                    .frame_switch("inner")
                    .run(async |_sb| Ok(path(&fake)))
                    .await?;
                assert_eq!(deepest, [0, 0]);
                assert_eq!(path(&fake), [0], "the inner block left only the inner frame");
                Ok(())
            })
            .await
            .expect("both frames are there");

        assert_eq!(path(&fake), Vec::<usize>::new());
    }

    #[tokio::test]
    async fn frame_switch_does_not_run_the_steps_without_the_frame() {
        let (driver, _fake) = start(Vec::new());
        let mut sb = driver.base_case().await;
        let ran = std::sync::atomic::AtomicBool::new(false);

        let error = sb
            .frame_switch("nothing")
            .timeout(Duration::from_secs(1))
            .run(async |_sb| {
                ran.store(true, std::sync::atomic::Ordering::SeqCst);
                Ok(())
            })
            .await
            .expect_err("there is no such frame");

        assert!(matches!(error, SeleniumBaseError::WaitTimeout(_)), "{error:?}");
        assert!(!ran.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[tokio::test]
    async fn frame_switch_is_recorded_as_a_frame_and_a_return_to_the_parent() {
        let (driver, _fake) = start(vec![FakeFrame::new("pay", "")]);
        let mut sb = driver.base_case().await;

        sb.frame_switch("pay").run(async |_sb| Ok(())).await.unwrap();

        let names: Vec<String> = sb
            .recorder
            .lock()
            .unwrap()
            .actions
            .iter()
            .map(|action| action.name.clone())
            .collect();
        assert_eq!(names, ["switch_to_frame", "switch_to_parent_frame"]);
    }

    /// A fake browser with the windows `w1`, `w2` and `w3`.
    fn three_windows() -> (MockWebDriver, Arc<Mutex<Fake>>) {
        let (driver, fake) = start(Vec::new());
        fake.lock().expect("the fake browser").windows =
            ["w1", "w2", "w3"].map(str::to_owned).to_vec();
        (driver, fake)
    }

    fn current_window(fake: &Mutex<Fake>) -> String {
        fake.lock().expect("the fake browser").current.clone()
    }

    #[tokio::test]
    async fn a_window_is_entered_by_its_position() {
        let (driver, fake) = three_windows();
        let sb = driver.base_case().await;

        sb.switch_to_window(1).await.expect("the second window");
        assert_eq!(current_window(&fake), "w2");
        sb.switch_to_window(-1).await.expect("the newest window");
        assert_eq!(current_window(&fake), "w3");
        sb.switch_to_window(0).await.expect("the first window");
        assert_eq!(current_window(&fake), "w1");
        sb.switch_to_window(-2).await.expect("the one before the newest");
        assert_eq!(current_window(&fake), "w2");
        let position: usize = 2;
        sb.switch_to_window(position).await.expect("a usize works too");
        assert_eq!(current_window(&fake), "w3");
    }

    #[tokio::test]
    async fn a_window_is_entered_by_its_handle() {
        let (driver, fake) = three_windows();
        let sb = driver.base_case().await;

        sb.switch_to_window("w2").await.expect("a &str handle");
        assert_eq!(current_window(&fake), "w2");
        let handles = sb.get_all_window_handles().await.unwrap();
        sb.switch_to_window(&handles[2]).await.expect("a &String handle");
        assert_eq!(current_window(&fake), "w3");
        sb.switch_to_window(handles[0].clone()).await.expect("a String handle");
        assert_eq!(current_window(&fake), "w1");
    }

    #[tokio::test]
    async fn a_window_that_opens_late_is_waited_for() {
        let (driver, fake) = start(Vec::new());
        let sb = driver.base_case().await;
        let browser = Arc::clone(&fake);
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(350)).await;
            browser
                .lock()
                .expect("the fake browser")
                .windows
                .push("w2".to_owned());
        });

        sb.switch_to_window(1)
            .await
            .expect("the call waits for the window");

        assert_eq!(current_window(&fake), "w2");
    }

    #[tokio::test]
    async fn a_window_that_is_not_there_times_out_and_names_the_window() {
        let (driver, fake) = three_windows();
        let sb = driver.base_case().await;

        for window in [WindowRef::Index(5), WindowRef::Handle("gone".to_owned())] {
            let error = sb
                .switch_to_window(window.clone())
                .timeout(Duration::from_secs(1))
                .await
                .expect_err("there is no such window");
            assert!(
                matches!(&error, SeleniumBaseError::WaitTimeout(message)
                    if message.contains(&format!("Window {{{window}}} was not present after 1s"))),
                "{error:?}"
            );
        }
        assert_eq!(current_window(&fake), "w1", "the browser stayed where it was");
    }

    #[tokio::test]
    async fn a_switch_to_a_window_is_recorded_with_its_handle() {
        let (driver, _fake) = three_windows();
        let sb = driver.base_case().await;

        sb.switch_to_window(-1).await.unwrap();

        let recorded: Vec<(String, Option<String>)> = sb
            .recorder
            .lock()
            .unwrap()
            .actions
            .iter()
            .map(|action| (action.name.clone(), action.target.clone()))
            .collect();
        assert_eq!(
            recorded,
            [("switch_to_window".to_owned(), Some("w3".to_owned()))]
        );
    }

    #[tokio::test]
    async fn switch_to_alert_gives_a_handle_on_the_alert() {
        let (driver, fake) = start(Vec::new());
        fake.lock().expect("the fake browser").alert = Some("My Prompt".to_owned());
        let sb = driver.base_case().await;

        let alert = sb.switch_to_alert().await.expect("an alert is open");

        assert_eq!(alert.text().await.unwrap(), "My Prompt");
        alert.send_keys("typed").await.unwrap();
        alert.dismiss().await.unwrap();
        assert_eq!(
            fake.lock().expect("the fake browser").alert_log,
            ["keys: typed", "dismiss"]
        );
        let error = alert.accept().await.expect_err("the alert is closed");
        assert!(matches!(error, SeleniumBaseError::WebDriver(_)), "{error:?}");
    }

    #[tokio::test]
    async fn switch_to_alert_waits_for_the_alert_and_times_out_without_one() {
        let (driver, fake) = start(Vec::new());
        let sb = driver.base_case().await;

        let error = sb
            .switch_to_alert()
            .timeout(Duration::from_secs(1))
            .await
            .expect_err("no alert is open");
        assert!(
            matches!(&error, SeleniumBaseError::WaitTimeout(message)
                if message.contains("Alert was not present after 1s")),
            "{error:?}"
        );

        let browser = Arc::clone(&fake);
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(350)).await;
            browser.lock().expect("the fake browser").alert = Some("late".to_owned());
        });
        let alert = sb.switch_to_alert().await.expect("the call waits");
        assert_eq!(alert.text().await.unwrap(), "late");
    }

    #[tokio::test]
    async fn the_alert_calls_of_the_case_work_beside_the_handle() {
        let (driver, fake) = start(Vec::new());
        fake.lock().expect("the fake browser").alert = Some("Sure?".to_owned());
        let sb = driver.base_case().await;

        assert!(sb.is_alert_present().await.unwrap());
        assert_eq!(sb.get_alert_text().await.unwrap(), "Sure?");
        let alert = sb.wait_for_and_switch_to_alert(1).await.unwrap();
        assert_eq!(alert.text().await.unwrap(), "Sure?");
        sb.accept_alert().await.unwrap();
        assert!(!sb.is_alert_present().await.unwrap());
    }

    #[tokio::test]
    async fn the_alert_handle_records_what_is_done_with_the_alert() {
        let (driver, fake) = start(Vec::new());
        fake.lock().expect("the fake browser").alert = Some("Name?".to_owned());
        let sb = driver.base_case().await;

        let alert = sb.switch_to_alert().await.unwrap();
        alert.send_keys("Ada").await.unwrap();
        alert.accept().await.unwrap();

        let recorded: Vec<(String, Option<String>)> = sb
            .recorder
            .lock()
            .unwrap()
            .actions
            .iter()
            .map(|action| (action.name.clone(), action.target.clone()))
            .collect();
        assert_eq!(
            recorded,
            [
                ("type_alert_text".to_owned(), Some("Ada".to_owned())),
                ("accept_alert".to_owned(), None),
            ]
        );
    }

}
