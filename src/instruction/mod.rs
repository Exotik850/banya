

mod model;
pub use model::*;

mod types {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct Valid;
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct Invalid;
}
