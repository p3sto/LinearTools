pub type Error = Box<dyn std::error::Error>;
pub type Result<T> = std::result::Result<T, Error>;

// #[derive(Error)]
// pub enum Error {

// }
