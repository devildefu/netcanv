use netcanv_renderer::paws;

/// The Canvas API has functions for saving and restoring states.
/// However, there is no function to take the current state and set it.
/// That's what this structure is for, which has all the necessary data we want to save and access,
/// which is necessary to have a global state for netcanv.
#[derive(Debug)]
pub(crate) struct State {
   pub(crate) translation: paws::Vector,
   pub(crate) scaling: paws::Vector,
}

impl Default for State {
   fn default() -> Self {
      Self {
         translation: Default::default(),
         scaling: paws::Vector::new(1.0, 1.0),
      }
   }
}
