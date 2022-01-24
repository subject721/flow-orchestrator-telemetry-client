use std::error::Error;


pub trait MetricFrontend {

    fn run(self) -> Result<(), Box<dyn Error>>;
}
