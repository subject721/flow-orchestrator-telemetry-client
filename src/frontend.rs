use std::error::Error;
use crate::backend::Backend;
use crate::common::metric::Metric;



pub trait MetricFrontend {

    fn run(self) -> Result<(), Box<dyn Error>>;
}
