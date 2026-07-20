// Tour constructors and playback aliases.

impl BaseCase {
    /// Creates a tour using the Shepherd theme.
    pub async fn create_shepherd_tour(&mut self, name: &str) -> Result<(), SeleniumBaseError> {
        self.create_tour_with_theme(name, TourTheme::Shepherd).await
    }

    /// Creates a tour using the IntroJS theme.
    pub async fn create_introjs_tour(&mut self, name: &str) -> Result<(), SeleniumBaseError> {
        self.create_tour_with_theme(name, TourTheme::IntroJs).await
    }

    /// Creates a tour using the DriverJS theme.
    pub async fn create_driverjs_tour(&mut self, name: &str) -> Result<(), SeleniumBaseError> {
        self.create_tour_with_theme(name, TourTheme::DriverJs).await
    }

    /// Creates a tour using the Bootstrap theme.
    pub async fn create_bootstrap_tour(&mut self, name: &str) -> Result<(), SeleniumBaseError> {
        self.create_tour_with_theme(name, TourTheme::Bootstrap).await
    }

    /// Creates a tour using the Hopscotch theme.
    pub async fn create_hopscotch_tour(&mut self, name: &str) -> Result<(), SeleniumBaseError> {
        self.create_tour_with_theme(name, TourTheme::Hopscotch).await
    }

    /// Alias for `play_tour`.
    pub async fn start_tour(&mut self) -> Result<(), SeleniumBaseError> {
        self.play_tour().await
    }
}
