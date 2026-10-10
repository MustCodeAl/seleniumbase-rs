// Tour constructors and playback aliases.

impl BaseCase {
    /// Creates a tour using the Shepherd theme.
    pub async fn create_shepherd_tour(&mut self, name: &str) -> Result<(), SeleniumBaseError> {
        self.tour = Some(Tour::new(name).with_theme(TourTheme::Shepherd));
        Ok(())
    }

    /// Creates a tour using the IntroJS theme.
    pub async fn create_introjs_tour(&mut self, name: &str) -> Result<(), SeleniumBaseError> {
        self.tour = Some(Tour::new(name).with_theme(TourTheme::IntroJs));
        Ok(())
    }

    /// Creates a tour using the DriverJS theme.
    pub async fn create_driverjs_tour(&mut self, name: &str) -> Result<(), SeleniumBaseError> {
        self.tour = Some(Tour::new(name).with_theme(TourTheme::DriverJs));
        Ok(())
    }

    /// Creates a tour using the Bootstrap theme.
    pub async fn create_bootstrap_tour(&mut self, name: &str) -> Result<(), SeleniumBaseError> {
        self.tour = Some(Tour::new(name).with_theme(TourTheme::Bootstrap));
        Ok(())
    }

    /// Creates a tour using the Hopscotch theme.
    pub async fn create_hopscotch_tour(&mut self, name: &str) -> Result<(), SeleniumBaseError> {
        self.tour = Some(Tour::new(name).with_theme(TourTheme::Hopscotch));
        Ok(())
    }

    /// Alias for `play_tour`.
    pub async fn start_tour(&mut self) -> Result<(), SeleniumBaseError> {
        self.play_tour().await
    }
}

#[cfg(test)]
mod tests_tours {
    use super::*;

    #[tokio::test]
    async fn each_constructor_makes_a_tour_with_its_own_theme() {
        let mut sb = BaseCase::with_session(BrowserConfig::default(), BrowserSession::disconnected());

        sb.create_shepherd_tour("a").await.unwrap();
        assert_eq!(sb.tour.as_ref().map(|t| (&t.name[..], &t.theme)), Some(("a", &TourTheme::Shepherd)));
        sb.create_introjs_tour("b").await.unwrap();
        assert_eq!(sb.tour.as_ref().map(|t| (&t.name[..], &t.theme)), Some(("b", &TourTheme::IntroJs)));
        sb.create_driverjs_tour("c").await.unwrap();
        assert_eq!(sb.tour.as_ref().map(|t| (&t.name[..], &t.theme)), Some(("c", &TourTheme::DriverJs)));
        sb.create_bootstrap_tour("d").await.unwrap();
        assert_eq!(sb.tour.as_ref().map(|t| (&t.name[..], &t.theme)), Some(("d", &TourTheme::Bootstrap)));
        sb.create_hopscotch_tour("e").await.unwrap();
        assert_eq!(sb.tour.as_ref().map(|t| (&t.name[..], &t.theme)), Some(("e", &TourTheme::Hopscotch)));
    }

    #[tokio::test]
    async fn a_new_tour_replaces_the_one_before_it() {
        let mut sb = BaseCase::with_session(BrowserConfig::default(), BrowserSession::disconnected());
        sb.create_shepherd_tour("first").await.unwrap();
        sb.create_introjs_tour("second").await.unwrap();
        let tour = sb.tour.as_ref().unwrap();
        assert_eq!((tour.name.as_str(), &tour.theme), ("second", &TourTheme::IntroJs));
        assert!(tour.steps.is_empty());
    }
}
