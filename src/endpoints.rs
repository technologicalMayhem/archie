#[derive(Debug)]
pub struct Endpoints {
    pub address: String,
    pub port: u16,
    pub https: bool,
}

impl Endpoints {
    #[must_use]
    pub fn artifacts(&self) -> String {
        self.url("artifacts")
    }

    #[must_use]
    pub fn add_packages(&self) -> String {
        self.url("packages/add")
    }

    #[must_use]
    pub fn add_package_url(&self) -> String {
        self.url("packages/add-url")
    }

    #[must_use]
    pub fn remove_packages(&self) -> String {
        self.url("packages/remove")
    }

    #[must_use]
    pub fn rebuilt_packages(&self) -> String {
        self.url("packages/rebuild")
    }

    #[must_use]
    pub fn key(&self) -> String {
        self.url("key")
    }

    #[must_use]
    pub fn status(&self) -> String {
        self.url("status")
    }

    #[must_use]
    pub fn list_logs(&self) -> String {
        self.url("logs")
    }

    #[must_use]
    pub fn get_log(&self, index: usize) -> String {
        self.url(&format!("logs/{index}"))
    }

    fn base(&self) -> String {
        format!("{}{}:{}/", self.protocol(), self.address, self.port)
    }

    fn url(&self, path: &str) -> String {
        let mut base = self.base();
        base.push_str(path);
        base
    }

    fn protocol(&self) -> &'static str {
        if self.https {
            "https://"
        } else {
            "http://"
        }
    }
}

impl Default for Endpoints {
    fn default() -> Self {
        Self {
            port: 3200,
            address: String::new(),
            https: true,
        }
    }
}
