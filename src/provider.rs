pub mod twitch;

pub trait Provider {
  fn openChannel(&mut self);
}